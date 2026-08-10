use bson::doc;
use bson::oid::ObjectId;
use moka::future::Cache;
use mongodb::Database;
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::models::file::{File, FileStatus};
use crate::services::folder_service::FolderService;
use crate::services::r2_service::{
    build_object_key, calculate_part_count, calculate_part_size, needs_multipart, R2Service,
    MAX_OBJECT_BYTES,
};

/// Số URL part tối đa cấp trong một lần gọi. Presign là thao tác cục bộ (chỉ
/// ký HMAC, không gọi mạng) nhưng 10.000 URL trong một response là vài MB JSON,
/// nên client xin theo lô.
pub const MAX_PART_URLS_PER_REQUEST: i64 = 100;

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadUrlResult {
    #[serde(rename = "fileId")]
    pub file_id: String,
    /// Chỉ có với upload single PUT.
    #[serde(rename = "uploadUrl", skip_serializing_if = "Option::is_none")]
    pub upload_url: Option<String>,
    /// `true` khi client phải đi đường multipart. Client cũ không đọc trường
    /// này nhưng cũng không gặp nó: file nhỏ vẫn trả `uploadUrl` như trước.
    #[serde(rename = "isMultipart")]
    pub is_multipart: bool,
    #[serde(rename = "partSize", skip_serializing_if = "Option::is_none")]
    pub part_size: Option<i64>,
    #[serde(rename = "partCount", skip_serializing_if = "Option::is_none")]
    pub part_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PartUrl {
    #[serde(rename = "partNumber")]
    pub part_number: i32,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PartUrlsResult {
    #[serde(rename = "partUrls")]
    pub part_urls: Vec<PartUrl>,
}

#[derive(Debug, Deserialize)]
pub struct CompletedPartBody {
    #[serde(rename = "partNumber")]
    pub part_number: i32,
    pub etag: String,
}

#[derive(Debug, Deserialize)]
pub struct CompleteMultipartBody {
    pub parts: Vec<CompletedPartBody>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageQuotaResult {
    pub used_bytes: i64,
    pub used_formatted: String,
    pub free_tier_limit_bytes: i64,
    pub free_tier_limit_formatted: String,
    pub remaining_bytes: i64,
    pub remaining_formatted: String,
    pub used_percentage: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanOrphanResult {
    pub deleted_orphan_r2_objects: usize,
    pub deleted_stale_pending_records: u64,
    pub aborted_stale_multipart_uploads: usize,
    pub freed_bytes: i64,
    pub freed_formatted: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanFileInfo {
    pub key: String,
    pub name: String,
    pub size: i64,
    pub size_formatted: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOrphansResult {
    pub deleted_count: usize,
    pub freed_bytes: i64,
    pub freed_formatted: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteOrphansBody {
    pub keys: Vec<String>,
}

pub struct FileService;

impl FileService {
    pub async fn list_files(
        db: &Database,
        folder_id: Option<ObjectId>,
        viewer_id: Option<&str>,
    ) -> Result<Vec<File>, AppError> {
        let collection = db.collection::<File>("files");
        let mut filter = doc! {
            "folderId": folder_id,
            "status": "completed",
        };

        if let Some(vid) = viewer_id {
            filter.insert(
                "$or",
                vec![
                    doc! { "isPublic": true },
                    doc! { "ownerId": vid },
                ],
            );
        } else {
            filter.insert("isPublic", true);
        }

        let mut cursor = collection
            .find(filter)
            .sort(doc! { "createdAt": -1 })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut files = Vec::new();
        use futures::StreamExt;
        while let Some(result) = cursor.next().await {
            let file = result.map_err(|e| AppError::Internal(e.to_string()))?;
            files.push(file);
        }

        Ok(files)
    }

    pub async fn request_upload_url(
        db: &Database,
        r2: &R2Service,
        name: String,
        mime_type: String,
        size: i64,
        folder_id: Option<ObjectId>,
        is_public: bool,
        owner_id: String,
    ) -> Result<UploadUrlResult, AppError> {
        if size > MAX_OBJECT_BYTES {
            return Err(AppError::BadRequest(format!(
                "File vượt giới hạn {} của R2",
                crate::utils::file_display::format_bytes(MAX_OBJECT_BYTES)
            )));
        }

        if let Some(fid) = folder_id {
            FolderService::assert_folder_ownership(db, &fid, &owner_id).await?;
        }

        let key = build_object_key(&owner_id, &name);
        let now = chrono::Utc::now();
        let use_multipart = needs_multipart(size);

        // Mở multipart trước khi ghi DB: nếu R2 từ chối thì chưa có bản ghi
        // `pending` nào phải dọn.
        let upload_id = if use_multipart {
            Some(
                r2.create_multipart_upload(&key, &mime_type)
                    .await
                    .map_err(AppError::Internal)?,
            )
        } else {
            None
        };

        let new_file = File {
            id: None,
            name: name.clone(),
            key: key.clone(),
            size,
            mime_type: mime_type.clone(),
            external_url: None,
            folder_id,
            owner_id,
            is_public,
            status: FileStatus::Pending,
            multipart_upload_id: upload_id.clone(),
            views: 0,
            downloads: 0,
            created_at: now,
            updated_at: now,
        };

        let collection = db.collection::<File>("files");
        let insert_res = collection.insert_one(&new_file).await;

        // Ghi DB hỏng thì multipart vừa mở sẽ không ai đóng — abort ngay thay
        // vì đợi job dọn rác 24h sau.
        let res = match insert_res {
            Ok(res) => res,
            Err(e) => {
                if let Some(uid) = &upload_id {
                    let _ = r2.abort_multipart_upload(&key, uid).await;
                }
                return Err(AppError::Internal(e.to_string()));
            }
        };

        let inserted_id = res
            .inserted_id
            .as_object_id()
            .ok_or_else(|| AppError::Internal("Không lấy được ObjectId file".to_string()))?;

        if use_multipart {
            let part_size = calculate_part_size(size);
            return Ok(UploadUrlResult {
                file_id: inserted_id.to_hex(),
                upload_url: None,
                is_multipart: true,
                part_size: Some(part_size),
                part_count: Some(calculate_part_count(size, part_size)),
            });
        }

        let upload_url = r2
            .create_upload_url(&key, &mime_type)
            .await
            .map_err(AppError::Internal)?;

        Ok(UploadUrlResult {
            file_id: inserted_id.to_hex(),
            upload_url: Some(upload_url),
            is_multipart: false,
            part_size: None,
            part_count: None,
        })
    }

    /// Cấp presigned URL cho một lô part. Client gọi ngay trước khi gửi lô đó
    /// nên URL không kịp hết hạn dù cả file mất hàng giờ để lên.
    pub async fn create_part_urls(
        db: &Database,
        r2: &R2Service,
        file_id: &ObjectId,
        owner_id: &str,
        part_numbers: Vec<i32>,
    ) -> Result<PartUrlsResult, AppError> {
        if part_numbers.is_empty() {
            return Err(AppError::bad_request("Danh sách partNumbers rỗng"));
        }
        if part_numbers.len() as i64 > MAX_PART_URLS_PER_REQUEST {
            return Err(AppError::BadRequest(format!(
                "Mỗi lần chỉ xin được tối đa {} URL part",
                MAX_PART_URLS_PER_REQUEST
            )));
        }

        let file = Self::get_owned_file(db, file_id, owner_id).await?;
        let upload_id = Self::require_multipart_id(&file)?;

        let mut part_urls = Vec::with_capacity(part_numbers.len());
        for part_number in part_numbers {
            // R2 đánh số part từ 1 tới 10.000; số ngoài khoảng này bị từ chối ở
            // tận lúc gửi part, khi client đã tốn công cắt file.
            if !(1..=crate::services::r2_service::MAX_PARTS as i32).contains(&part_number) {
                return Err(AppError::BadRequest(format!(
                    "partNumber {} không hợp lệ",
                    part_number
                )));
            }

            let url = r2
                .create_part_upload_url(&file.key, upload_id, part_number)
                .await
                .map_err(AppError::Internal)?;
            part_urls.push(PartUrl { part_number, url });
        }

        Ok(PartUrlsResult { part_urls })
    }

    /// Ghép các part lại rồi đánh dấu file hoàn tất. Gộp hai bước vào một
    /// endpoint để không có khoảng trống mà object đã tồn tại trên R2 nhưng bản
    /// ghi vẫn `pending`.
    pub async fn complete_multipart_upload(
        db: &Database,
        r2: &R2Service,
        cache: &Cache<String, File>,
        file_id: &ObjectId,
        owner_id: &str,
        mut parts: Vec<CompletedPartBody>,
    ) -> Result<File, AppError> {
        let file = Self::get_owned_file(db, file_id, owner_id).await?;

        if file.status == FileStatus::Completed {
            return Ok(file);
        }

        // Client gọi lại endpoint này khi mạng đứt giữa chừng. Lần trước có thể
        // đã ghép xong trên R2 mà response không về được, khi đó `uploadId` đã
        // bị xóa — đi thẳng tới bước đánh dấu hoàn tất thay vì báo lỗi.
        if let Some(upload_id) = file.multipart_upload_id.as_deref() {
            if parts.is_empty() {
                return Err(AppError::bad_request("Danh sách parts rỗng"));
            }

            parts.sort_by_key(|p| p.part_number);
            let pairs: Vec<(i32, String)> =
                parts.into_iter().map(|p| (p.part_number, p.etag)).collect();

            if let Err(e) = r2.complete_multipart_upload(&file.key, upload_id, &pairs).await {
                // Lần gọi trước đã ghép xong thì `uploadId` không còn tồn tại và
                // R2 trả `NoSuchUpload`. Object có mặt trên bucket là bằng chứng
                // đủ để coi như thành công; nếu không có thì đây là lỗi thật.
                let already_done = matches!(r2.get_object_meta(&file.key).await, Ok((true, _)));
                if !already_done {
                    return Err(AppError::Internal(e));
                }
            }

            // uploadId đã dùng xong; giữ lại sẽ khiến job dọn rác tưởng còn upload dở.
            let collection = db.collection::<File>("files");
            collection
                .update_one(
                    doc! { "_id": file_id },
                    doc! { "$unset": { "multipartUploadId": "" } },
                )
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }

        Self::complete_upload(db, r2, cache, file_id, owner_id).await
    }

    /// Hủy một upload đang dở: đóng multipart (nếu có) và xóa bản ghi `pending`.
    /// Dùng chung cho cả hai đường upload nên client chỉ cần một lời gọi khi
    /// người dùng bấm Hủy hoặc khi có lỗi giữa chừng.
    pub async fn abort_upload(
        db: &Database,
        r2: &R2Service,
        cache: &Cache<String, File>,
        file_id: &ObjectId,
        owner_id: &str,
    ) -> Result<(), AppError> {
        let file = Self::get_owned_file(db, file_id, owner_id).await?;

        if let Some(upload_id) = &file.multipart_upload_id {
            // Lỗi abort không nên chặn việc xóa bản ghi: job dọn rác sẽ quét lại
            // các multipart treo.
            let _ = r2.abort_multipart_upload(&file.key, upload_id).await;
        }

        // Chỉ xóa bản ghi còn `pending`. File đã hoàn tất mà bị xóa vì một lời
        // gọi abort đến muộn thì mất dữ liệu thật.
        let collection = db.collection::<File>("files");
        collection
            .delete_one(doc! { "_id": file_id, "status": "pending" })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        cache.invalidate(&file_id.to_hex()).await;

        Ok(())
    }

    fn require_multipart_id(file: &File) -> Result<&str, AppError> {
        file.multipart_upload_id.as_deref().ok_or_else(|| {
            AppError::bad_request("File này không dùng multipart upload")
        })
    }


    pub async fn get_owned_file(
        db: &Database,
        file_id: &ObjectId,
        owner_id: &str,
    ) -> Result<File, AppError> {
        let collection = db.collection::<File>("files");
        let file = collection
            .find_one(doc! { "_id": file_id })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy file".to_string()))?;

        if file.owner_id != owner_id {
            return Err(AppError::Forbidden);
        }

        Ok(file)
    }

    pub async fn complete_upload(
        db: &Database,
        r2: &R2Service,
        cache: &Cache<String, File>,
        file_id: &ObjectId,
        owner_id: &str,
    ) -> Result<File, AppError> {
        let mut file = Self::get_owned_file(db, file_id, owner_id).await?;

        if file.status == FileStatus::Completed {
            return Ok(file);
        }

        // Lỗi ở đây (mạng, credential, 5xx của R2) trả về 500 và giữ nguyên
        // trạng thái `pending` để client gọi lại. Chỉ khi R2 xác nhận object
        // không tồn tại mới đánh dấu `failed`.
        let (exists, size_opt) = r2.get_object_meta(&file.key).await.map_err(AppError::Internal)?;

        let collection = db.collection::<File>("files");
        let now = chrono::Utc::now();

        if !exists {
            file.status = FileStatus::Failed;
            file.updated_at = now;
            collection
                .update_one(
                    doc! { "_id": file_id },
                    doc! { "$set": { "status": "failed", "updatedAt": now } },
                )
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            return Err(AppError::BadRequest(
                "Không tìm thấy file trên R2, upload có thể đã thất bại".to_string(),
            ));
        }

        file.status = FileStatus::Completed;
        if let Some(s) = size_opt {
            file.size = s;
        }
        file.updated_at = now;

        collection
            .update_one(
                doc! { "_id": file_id },
                doc! { "$set": { "status": "completed", "size": file.size, "updatedAt": now } },
            )
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        cache.insert(file_id.to_hex(), file.clone()).await;

        Ok(file)
    }

    pub async fn get_download_url(
        db: &Database,
        r2: &R2Service,
        cache: &Cache<String, File>,
        file_id: &ObjectId,
        viewer_id: Option<&str>,
        inline: bool,
    ) -> Result<String, AppError> {
        let file_key_str = file_id.to_hex();
        let mut cached_file = cache.get(&file_key_str).await;

        if cached_file.is_none() {
            let collection = db.collection::<File>("files");
            if let Ok(Some(f)) = collection.find_one(doc! { "_id": file_id }).await {
                cache.insert(file_key_str.clone(), f.clone()).await;
                cached_file = Some(f);
            }
        }

        let file = cached_file.ok_or_else(|| AppError::NotFound("Không tìm thấy file".to_string()))?;

        if file.status != FileStatus::Completed {
            return Err(AppError::NotFound("Không tìm thấy file".to_string()));
        }

        if !file.is_public && file.owner_id.as_str() != viewer_id.unwrap_or_default() {
            return Err(AppError::Forbidden);
        }

        // Background update view / download count
        let db_clone = db.clone();
        let fid = *file_id;
        tokio::spawn(async move {
            let collection = db_clone.collection::<File>("files");
            let inc_field = if inline { "views" } else { "downloads" };
            let _ = collection
                .update_one(doc! { "_id": fid }, doc! { "$inc": { inc_field: 1 } })
                .await;
        });

        if let Some(ext_url) = file.external_url {
            return Ok(ext_url);
        }

        r2.create_download_url(&file.key, Some(&file.name), inline)
            .await
            .map_err(AppError::Internal)
    }

    pub async fn delete_file(
        db: &Database,
        r2: &R2Service,
        cache: &Cache<String, File>,
        file_id: &ObjectId,
        owner_id: &str,
    ) -> Result<(), AppError> {
        let file = Self::get_owned_file(db, file_id, owner_id).await?;

        if file.external_url.is_none() {
            let _ = r2.delete_object(&file.key).await;
        }

        let collection = db.collection::<File>("files");
        collection
            .delete_one(doc! { "_id": file_id })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        cache.invalidate(&file_id.to_hex()).await;

        Ok(())
    }

    pub async fn create_linked_file(
        db: &Database,
        name: String,
        url: String,
        mime_type: String,
        folder_id: Option<ObjectId>,
        owner_id: String,
    ) -> Result<File, AppError> {
        if let Some(fid) = folder_id {
            FolderService::assert_folder_ownership(db, &fid, &owner_id).await?;
        }

        let key = format!("linked-{}-{}-{}", owner_id, chrono::Utc::now().timestamp_millis(), uuid::Uuid::new_v4());
        let now = chrono::Utc::now();

        let new_file = File {
            id: None,
            name,
            key,
            size: 0,
            mime_type,
            external_url: Some(url),
            folder_id,
            owner_id,
            is_public: false,
            status: FileStatus::Completed,
            multipart_upload_id: None,
            views: 0,
            downloads: 0,
            created_at: now,
            updated_at: now,
        };

        let collection = db.collection::<File>("files");
        let res = collection
            .insert_one(&new_file)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let inserted_id = res
            .inserted_id
            .as_object_id()
            .ok_or_else(|| AppError::Internal("Không lấy được ObjectId file".to_string()))?;

        let mut created = new_file;
        created.id = Some(inserted_id);

        Ok(created)
    }

    pub async fn update_file(
        db: &Database,
        cache: &Cache<String, File>,
        file_id: &ObjectId,
        owner_id: &str,
        name: Option<String>,
        is_public: Option<bool>,
        folder_id: Option<Option<ObjectId>>,
        url: Option<String>,
        mime_type: Option<String>,
    ) -> Result<File, AppError> {
        let mut file = Self::get_owned_file(db, file_id, owner_id).await?;

        if let Some(Some(fid)) = folder_id {
            FolderService::assert_folder_ownership(db, &fid, owner_id).await?;
        }

        let mut update_doc = doc! {};

        if let Some(n) = name {
            file.name = n.clone();
            update_doc.insert("name", n);
        }
        if let Some(pub_val) = is_public {
            file.is_public = pub_val;
            update_doc.insert("isPublic", pub_val);
        }
        if let Some(f_opt) = folder_id {
            file.folder_id = f_opt;
            update_doc.insert("folderId", f_opt);
        }
        if let Some(u) = url {
            file.external_url = Some(u.clone());
            update_doc.insert("externalUrl", u);
        }
        if let Some(m) = mime_type {
            file.mime_type = m.clone();
            update_doc.insert("mimeType", m);
        }

        let now = chrono::Utc::now();
        file.updated_at = now;
        update_doc.insert("updatedAt", now);

        if !update_doc.is_empty() {
            let collection = db.collection::<File>("files");
            collection
                .update_one(doc! { "_id": file_id }, doc! { "$set": update_doc })
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }

        cache.insert(file_id.to_hex(), file.clone()).await;

        Ok(file)
    }

    pub async fn get_storage_quota(
        db: &Database,
        owner_id: Option<&str>,
    ) -> Result<StorageQuotaResult, AppError> {
        let collection = db.collection::<File>("files");
        let filter = if let Some(oid) = owner_id {
            doc! { "ownerId": oid, "status": "completed" }
        } else {
            doc! { "status": "completed" }
        };

        let mut cursor = collection
            .find(filter)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut used_bytes: i64 = 0;
        use futures::StreamExt;
        while let Some(result) = cursor.next().await {
            if let Ok(file) = result {
                if file.external_url.is_none() {
                    used_bytes += file.size;
                }
            }
        }

        let free_tier_limit_bytes: i64 = 10 * 1024 * 1024 * 1024; // 10 GB Free Tier limit
        let remaining_bytes = (free_tier_limit_bytes - used_bytes).max(0);
        let used_percentage = if free_tier_limit_bytes > 0 {
            ((used_bytes as f64) / (free_tier_limit_bytes as f64)) * 100.0
        } else {
            0.0
        };

        Ok(StorageQuotaResult {
            used_bytes,
            used_formatted: crate::utils::file_display::format_bytes(used_bytes),
            free_tier_limit_bytes,
            free_tier_limit_formatted: crate::utils::file_display::format_bytes(free_tier_limit_bytes),
            remaining_bytes,
            remaining_formatted: crate::utils::file_display::format_bytes(remaining_bytes),
            used_percentage: (used_percentage * 100.0).round() / 100.0,
        })
    }

    pub async fn clean_orphan_files(
        db: &Database,
        r2: &R2Service,
    ) -> Result<CleanOrphanResult, AppError> {
        let collection = db.collection::<File>("files");

        let mut cursor = collection
            .find(doc! {})
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        use std::collections::HashSet;
        use futures::StreamExt;
        let mut valid_keys = HashSet::new();

        while let Some(result) = cursor.next().await {
            if let Ok(file) = result {
                valid_keys.insert(file.key);
            }
        }

        let stale_cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
        let stale_pending_filter = doc! {
            "status": "pending",
            "createdAt": { "$lt": stale_cutoff }
        };

        let mut stale_cursor = collection
            .find(stale_pending_filter.clone())
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        while let Some(result) = stale_cursor.next().await {
            if let Ok(stale_file) = result {
                // Bản ghi pending dở dang có thể là multipart chưa complete.
                // Xóa object không đụng tới các part đã upload, phải abort riêng.
                if let Some(upload_id) = &stale_file.multipart_upload_id {
                    let _ = r2.abort_multipart_upload(&stale_file.key, upload_id).await;
                }
                let _ = r2.delete_object(&stale_file.key).await;
            }
        }

        let delete_res = collection
            .delete_many(stale_pending_filter)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let deleted_stale_count = delete_res.deleted_count;

        // Multipart bị bỏ giữa chừng (đóng tab, mất mạng) không để lại object
        // nào trong list_objects nhưng các part vẫn bị tính dung lượng, nên phải
        // quét thẳng từ phía R2 chứ không dựa vào bản ghi DB.
        let mut aborted_stale_multipart_uploads = 0;
        match r2.list_multipart_uploads().await {
            Ok(uploads) => {
                for (key, upload_id, initiated) in uploads {
                    // Không rõ thời điểm khởi tạo thì để yên: có thể là upload
                    // đang chạy, abort nhầm sẽ làm hỏng nó.
                    let Some(started_at) = initiated else { continue };
                    if started_at >= stale_cutoff {
                        continue;
                    }
                    if r2.abort_multipart_upload(&key, &upload_id).await.is_ok() {
                        aborted_stale_multipart_uploads += 1;
                    }
                }
            }
            Err(e) => eprintln!("⚠️ Không liệt kê được multipart upload treo: {}", e),
        }

        let r2_objects = r2.list_all_objects().await.map_err(AppError::Internal)?;
        let mut deleted_orphan_r2_objects = 0;
        let mut freed_bytes: i64 = 0;

        for (key, size) in r2_objects {
            if !valid_keys.contains(&key) {
                if r2.delete_object(&key).await.is_ok() {
                    deleted_orphan_r2_objects += 1;
                    freed_bytes += size;
                }
            }
        }

        Ok(CleanOrphanResult {
            deleted_orphan_r2_objects,
            deleted_stale_pending_records: deleted_stale_count,
            aborted_stale_multipart_uploads,
            freed_bytes,
            freed_formatted: crate::utils::file_display::format_bytes(freed_bytes),
        })
    }

    pub async fn list_orphan_files(
        db: &Database,
        r2: &R2Service,
    ) -> Result<Vec<OrphanFileInfo>, AppError> {
        let collection = db.collection::<File>("files");

        let mut cursor = collection
            .find(doc! {})
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        use std::collections::HashSet;
        use futures::StreamExt;
        let mut valid_keys = HashSet::new();

        while let Some(result) = cursor.next().await {
            if let Ok(file) = result {
                valid_keys.insert(file.key);
            }
        }

        let r2_objects = r2.list_all_objects().await.map_err(AppError::Internal)?;
        let mut orphans = Vec::new();

        for (key, size) in r2_objects {
            if !valid_keys.contains(&key) {
                let name = key
                    .rsplit_once('/')
                    .map(|(_, name)| name)
                    .unwrap_or(&key)
                    .to_string();

                orphans.push(OrphanFileInfo {
                    name,
                    size_formatted: crate::utils::file_display::format_bytes(size),
                    key,
                    size,
                });
            }
        }

        Ok(orphans)
    }

    pub async fn delete_specific_orphans(
        r2: &R2Service,
        keys: Vec<String>,
    ) -> Result<DeleteOrphansResult, AppError> {
        let r2_objects = r2.list_all_objects().await.map_err(AppError::Internal)?;
        use std::collections::HashMap;
        let size_map: HashMap<String, i64> = r2_objects.into_iter().collect();

        let mut deleted_count = 0;
        let mut freed_bytes: i64 = 0;

        for key in keys {
            let size = size_map.get(&key).copied().unwrap_or(0);
            if r2.delete_object(&key).await.is_ok() {
                deleted_count += 1;
                freed_bytes += size;
            }
        }

        Ok(DeleteOrphansResult {
            deleted_count,
            freed_bytes,
            freed_formatted: crate::utils::file_display::format_bytes(freed_bytes),
        })
    }
}
