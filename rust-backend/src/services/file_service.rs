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

/// Bản ghi `pending` hoặc phiên multipart vượt quá ngưỡng này thì coi là rác:
/// upload thật không thể im lặng quá 24h mà client không abort hay complete.
/// `list_orphan_files` và `clean_orphan_files` phải dùng chung một ngưỡng để
/// danh sách liệt kê luôn khớp với những gì nút dọn dẹp sẽ xóa.
pub const STALE_PENDING_CUTOFF_HOURS: i64 = 24;

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

/// Bản ghi `pending` đã quá hạn — upload chết giữa chừng. Object trên R2 có thể
/// tồn tại hoặc không; thứ chắc chắn rác là chính bản ghi này.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StalePendingInfo {
    pub file_id: String,
    pub name: String,
    pub key: String,
    pub size: i64,
    pub size_formatted: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub has_multipart: bool,
}

/// Phiên multipart treo trên R2 không còn bản ghi DB nào nhận (bản ghi nhận nó
/// đã bị xóa hoặc chưa từng tồn tại). Part dở dang vẫn bị tính dung lượng lưu
/// trữ nhưng không hiện trong danh sách object.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DanglingMultipartInfo {
    pub key: String,
    pub upload_id: String,
    pub initiated: Option<chrono::DateTime<chrono::Utc>>,
}

/// Đủ ba loại rác mà `clean_orphan_files` dọn. Bản cũ chỉ trả loại đầu nên hai
/// loại sau — trong đó có các upload thất bại chiếm hàng GB — không bao giờ
/// xuất hiện trong modal.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanScanResult {
    pub orphan_objects: Vec<OrphanFileInfo>,
    pub stale_pending_files: Vec<StalePendingInfo>,
    pub dangling_multipart: Vec<DanglingMultipartInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOrphansResult {
    pub deleted_count: usize,
    pub deleted_stale_pending: usize,
    pub aborted_multipart: usize,
    pub freed_bytes: i64,
    pub freed_formatted: String,
}

/// Tham chiếu một phiên multipart cần abort, nhận từ client.
#[derive(Debug, Deserialize)]
pub struct MultipartRef {
    pub key: String,
    #[serde(rename = "uploadId")]
    pub upload_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteOrphansBody {
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default, rename = "fileIds")]
    pub file_ids: Vec<String>,
    #[serde(default, rename = "multipartRefs")]
    pub multipart_refs: Vec<MultipartRef>,
}

/// Phân loại rác thành ba loại khớp đúng những gì `clean_orphan_files` dọn:
///
/// - `orphan_objects`: object trên R2 không bản ghi DB nào biết tới;
/// - `stale_pending_files`: bản ghi `pending` quá `cutoff` — upload chết giữa
///   chừng, object tương ứng có thể còn hoặc mất;
/// - `dangling_multipart`: phiên multipart quá `cutoff` mà không bản ghi nào
///   nhận qua `multipartUploadId` (phiên đã có bản ghi nhận được loại thứ hai
///   phủ rồi, không liệt kê trùng).
///
/// Hàm thuần để unit test không cần nối R2/MongoDB.
pub fn classify_orphans(
    files: &[File],
    r2_objects: &[(String, i64)],
    multipart_uploads: &[(String, String, Option<chrono::DateTime<chrono::Utc>>)],
    cutoff: chrono::DateTime<chrono::Utc>,
) -> OrphanScanResult {
    use std::collections::HashSet;

    // Mọi bản ghi đều được tính là "chủ" hợp lệ của key, kể cả `pending`:
    // object của upload đang chạy không phải rác.
    let valid_keys: HashSet<&str> = files.iter().map(|f| f.key.as_str()).collect();
    let claimed_upload_ids: HashSet<&str> = files
        .iter()
        .filter_map(|f| f.multipart_upload_id.as_deref())
        .collect();

    let orphan_objects = r2_objects
        .iter()
        .filter(|(key, _)| !valid_keys.contains(key.as_str()))
        .map(|(key, size)| {
            let name = key
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or(key)
                .to_string();
            OrphanFileInfo {
                name,
                size_formatted: crate::utils::file_display::format_bytes(*size),
                key: key.clone(),
                size: *size,
            }
        })
        .collect();

    let stale_pending_files = files
        .iter()
        .filter(|f| f.status == FileStatus::Pending && f.created_at < cutoff)
        .filter_map(|f| {
            // Bản ghi đọc từ cursor luôn có `_id`; nếu không thì cũng không có
            // cách nào xóa đích danh nó nên bỏ qua.
            let id = f.id?;
            Some(StalePendingInfo {
                file_id: id.to_hex(),
                name: f.name.clone(),
                key: f.key.clone(),
                size: f.size,
                size_formatted: crate::utils::file_display::format_bytes(f.size),
                created_at: f.created_at,
                has_multipart: f.multipart_upload_id.is_some(),
            })
        })
        .collect();

    let dangling_multipart = multipart_uploads
        .iter()
        .filter(|(_, upload_id, initiated)| {
            // Không rõ thời điểm khởi tạo thì để yên, đúng như
            // `clean_orphan_files`: abort nhầm một upload đang chạy còn tệ hơn
            // bỏ sót một phiên treo.
            initiated.is_some_and(|t| t < cutoff) && !claimed_upload_ids.contains(upload_id.as_str())
        })
        .map(|(key, upload_id, initiated)| DanglingMultipartInfo {
            key: key.clone(),
            upload_id: upload_id.clone(),
            initiated: *initiated,
        })
        .collect();

    OrphanScanResult {
        orphan_objects,
        stale_pending_files,
        dangling_multipart,
    }
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

        let stale_cutoff = chrono::Utc::now()
            - chrono::Duration::hours(STALE_PENDING_CUTOFF_HOURS);
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

    /// Quét đủ ba loại rác mà `clean_orphan_files` sẽ dọn, để modal hiện đúng
    /// những gì nút "Xoá tất cả" đụng tới.
    pub async fn list_orphan_files(
        db: &Database,
        r2: &R2Service,
    ) -> Result<OrphanScanResult, AppError> {
        let collection = db.collection::<File>("files");

        let mut cursor = collection
            .find(doc! {})
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        use futures::StreamExt;
        let mut files = Vec::new();
        while let Some(result) = cursor.next().await {
            if let Ok(file) = result {
                files.push(file);
            }
        }

        let r2_objects = r2.list_all_objects().await.map_err(AppError::Internal)?;

        // Liệt kê multipart hỏng thì vẫn trả hai loại còn lại, khớp cách
        // `clean_orphan_files` xử lý cùng lỗi này (ghi log và bỏ qua).
        let multipart_uploads = match r2.list_multipart_uploads().await {
            Ok(uploads) => uploads,
            Err(e) => {
                eprintln!("⚠️ Không liệt kê được multipart upload treo: {}", e);
                Vec::new()
            }
        };

        let stale_cutoff = chrono::Utc::now()
            - chrono::Duration::hours(STALE_PENDING_CUTOFF_HOURS);

        Ok(classify_orphans(&files, &r2_objects, &multipart_uploads, stale_cutoff))
    }

    /// Xóa rác theo lựa chọn của người dùng, phủ cả ba loại với ngữ nghĩa giống
    /// các vòng lặp tương ứng trong `clean_orphan_files`: xóa object, xóa bản ghi
    /// `pending` quá hạn (kèm abort multipart nếu có), và abort phiên multipart
    /// vô chủ.
    pub async fn delete_specific_orphans(
        db: &Database,
        r2: &R2Service,
        body: DeleteOrphansBody,
    ) -> Result<DeleteOrphansResult, AppError> {
        let r2_objects = r2.list_all_objects().await.map_err(AppError::Internal)?;
        use std::collections::HashMap;
        let size_map: HashMap<String, i64> = r2_objects.into_iter().collect();

        let mut deleted_count = 0;
        let mut freed_bytes: i64 = 0;

        for key in body.keys {
            let size = size_map.get(&key).copied().unwrap_or(0);
            if r2.delete_object(&key).await.is_ok() {
                deleted_count += 1;
                freed_bytes += size;
            }
        }

        let collection = db.collection::<File>("files");
        let stale_cutoff = chrono::Utc::now()
            - chrono::Duration::hours(STALE_PENDING_CUTOFF_HOURS);
        let mut deleted_stale_pending = 0;

        for id_str in body.file_ids {
            let Ok(file_oid) = ObjectId::parse_str(&id_str) else {
                continue;
            };
            let file = match collection.find_one(doc! { "_id": file_oid }).await {
                Ok(Some(f)) => f,
                _ => continue,
            };
            // Chỉ đụng bản ghi pending quá hạn: lời gọi đến muộn không được phá
            // một upload thật đang chạy hoặc file đã hoàn tất.
            if file.status != FileStatus::Pending || file.created_at >= stale_cutoff {
                continue;
            }
            if let Some(upload_id) = &file.multipart_upload_id {
                let _ = r2.abort_multipart_upload(&file.key, upload_id).await;
            }
            let _ = r2.delete_object(&file.key).await;
            match collection
                .delete_one(doc! {
                    "_id": file_oid,
                    "status": "pending",
                    "createdAt": { "$lt": stale_cutoff },
                })
                .await
            {
                Ok(res) if res.deleted_count > 0 => deleted_stale_pending += 1,
                _ => {}
            }
        }

        // Phiên multipart vô chủ chỉ abort được; part của chúng không nằm trong
        // danh sách object nên không tính được dung lượng giải phóng ở đây.
        let mut aborted_multipart = 0;
        for mp in body.multipart_refs {
            if r2.abort_multipart_upload(&mp.key, &mp.upload_id).await.is_ok() {
                aborted_multipart += 1;
            }
        }

        Ok(DeleteOrphansResult {
            deleted_count,
            deleted_stale_pending,
            aborted_multipart,
            freed_bytes,
            freed_formatted: crate::utils::file_display::format_bytes(freed_bytes),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn file(status: FileStatus, key: &str, created_at: chrono::DateTime<Utc>) -> File {
        File {
            id: Some(ObjectId::new()),
            name: format!("ten-{key}"),
            key: key.to_string(),
            size: 1024,
            mime_type: "application/octet-stream".to_string(),
            external_url: None,
            folder_id: None,
            owner_id: "u1".to_string(),
            is_public: false,
            status,
            multipart_upload_id: None,
            views: 0,
            downloads: 0,
            created_at,
            updated_at: created_at,
        }
    }

    /// `now` phải là mốc mà dữ liệu test xây dựng theo, để cutoff không bị
    /// trôi giữa lúc tạo dữ liệu và lúc phân loại.
    fn scan(
        files: Vec<File>,
        objects: Vec<(String, i64)>,
        uploads: Vec<(String, String, Option<chrono::DateTime<Utc>>)>,
        now: chrono::DateTime<Utc>,
    ) -> OrphanScanResult {
        classify_orphans(&files, &objects, &uploads, now - Duration::hours(STALE_PENDING_CUTOFF_HOURS))
    }

    #[test]
    fn object_co_ban_ghi_khong_phai_mo_coi_ke_ca_pending() {
        // Object của upload đang chạy (pending non) không được tính là rác.
        let now = Utc::now();
        let res = scan(
            vec![file(FileStatus::Pending, "u1/a.bin", now)],
            vec![("u1/a.bin".to_string(), 100)],
            vec![],
            now,
        );
        assert!(res.orphan_objects.is_empty());
        assert!(res.stale_pending_files.is_empty());
    }

    #[test]
    fn object_khong_co_ban_ghi_bi_phat_hien() {
        let res = scan(vec![], vec![("file-lac-loai.bin".to_string(), 42)], vec![], Utc::now());
        assert_eq!(res.orphan_objects.len(), 1);
        assert_eq!(res.orphan_objects[0].key, "file-lac-loai.bin");
        assert_eq!(res.orphan_objects[0].name, "file-lac-loai.bin");
        assert_eq!(res.orphan_objects[0].size, 42);
    }

    #[test]
    fn pending_qua_han_moi_boc_liet_ke() {
        let now = Utc::now();
        let cutoff_nguon = now - Duration::hours(STALE_PENDING_CUTOFF_HOURS);
        let muoi_phut_truoc = now - Duration::minutes(10);
        let ba_ngay_truoc = now - Duration::days(3);

        let res = scan(
            vec![
                file(FileStatus::Pending, "u1/non.bin", muoi_phut_truoc),
                file(FileStatus::Pending, "u1/stale.bin", ba_ngay_truoc),
                file(FileStatus::Completed, "u1/done.bin", ba_ngay_truoc),
                // pending đúng ngưỡng: không tính rác (phải quá, không phải bằng)
                file(FileStatus::Pending, "u1/exact.bin", cutoff_nguon),
            ],
            vec![],
            vec![],
            now,
        );

        let keys: Vec<&str> = res
            .stale_pending_files
            .iter()
            .map(|f| f.key.as_str())
            .collect();
        assert_eq!(keys, vec!["u1/stale.bin"], "chỉ pending quá 24h mới liệt kê");
        assert!(res.stale_pending_files[0].file_id.len() == 24);
    }

    #[test]
    fn multipart_co_ban_ghi_nhan_khong_vao_dangling() {
        let upload_id = "upload-1".to_string();
        let now = Utc::now();
        let stale_time = now - Duration::days(2);

        let mut f = file(FileStatus::Pending, "u1/key.bin", stale_time);
        f.multipart_upload_id = Some(upload_id.clone());

        let res = scan(
            vec![f],
            vec![],
            vec![("u1/key.bin".to_string(), upload_id, Some(stale_time))],
            now,
        );

        // Đã phủ bởi stale_pending_files; không liệt kê trùng ở dangling.
        assert!(res.dangling_multipart.is_empty());
        assert_eq!(res.stale_pending_files.len(), 1);
        assert!(res.stale_pending_files[0].has_multipart);
    }

    #[test]
    fn multipart_khong_ban_ghi_nhan_va_qua_han_vao_dangling() {
        let now = Utc::now();
        let stale_time = now - Duration::days(2);
        let res = scan(
            vec![],
            vec![],
            vec![
                ("u1/orphan-mp.bin".to_string(), "upload-x".to_string(), Some(stale_time)),
                // Không rõ thời điểm khởi tạo: bỏ qua, không abort nhầm.
                ("u1/unknown.bin".to_string(), "upload-y".to_string(), None),
                // Còn non: bỏ qua.
                (
                    "u1/fresh.bin".to_string(),
                    "upload-z".to_string(),
                    Some(now),
                ),
            ],
            now,
        );

        assert_eq!(res.dangling_multipart.len(), 1);
        assert_eq!(res.dangling_multipart[0].upload_id, "upload-x");
    }
}
