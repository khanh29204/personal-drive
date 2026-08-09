use bson::doc;
use bson::oid::ObjectId;
use moka::future::Cache;
use mongodb::Database;
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::models::file::{File, FileStatus};
use crate::services::folder_service::FolderService;
use crate::services::r2_service::{build_object_key, R2Service};

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadUrlResult {
    #[serde(rename = "fileId")]
    pub file_id: String,
    #[serde(rename = "uploadUrl")]
    pub upload_url: String,
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
        if let Some(fid) = folder_id {
            FolderService::assert_folder_ownership(db, &fid, &owner_id).await?;
        }

        let key = build_object_key(&owner_id, &name);
        let now = chrono::Utc::now();

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

        let upload_url = r2
            .create_upload_url(&key, &mime_type)
            .await
            .map_err(AppError::Internal)?;

        Ok(UploadUrlResult {
            file_id: inserted_id.to_hex(),
            upload_url,
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
}
