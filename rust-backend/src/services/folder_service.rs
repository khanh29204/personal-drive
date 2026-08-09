use bson::doc;
use bson::oid::ObjectId;
use mongodb::Database;
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::models::file::File;
use crate::models::folder::Folder;
use crate::services::r2_service::R2Service;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreadcrumbEntry {
    pub id: String,
    pub name: String,
    pub path: String,
}

pub struct FolderService;

impl FolderService {
    pub async fn resolve_path(
        db: &Database,
        path: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<ObjectId>, AppError> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.trim().is_empty()).collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let collection = db.collection::<Folder>("folders");
        let mut current_parent_id: Option<ObjectId> = None;

        for part in parts {
            let mut filter = doc! {
                "name": part,
                "parentId": current_parent_id,
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

            let folder_doc = collection
                .find_one(filter)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            match folder_doc {
                Some(folder) => {
                    current_parent_id = folder.id;
                }
                None => return Ok(None),
            }
        }

        Ok(current_parent_id)
    }

    pub async fn list_folders(
        db: &Database,
        parent_id: Option<ObjectId>,
        viewer_id: Option<&str>,
    ) -> Result<Vec<Folder>, AppError> {
        let collection = db.collection::<Folder>("folders");
        let mut filter = doc! { "parentId": parent_id };

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
            .sort(doc! { "name": 1 })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut folders = Vec::new();
        use futures::StreamExt;
        while let Some(result) = cursor.next().await {
            let folder = result.map_err(|e| AppError::Internal(e.to_string()))?;
            folders.push(folder);
        }

        Ok(folders)
    }

    pub async fn list_all_user_folders(
        db: &Database,
        owner_id: &str,
    ) -> Result<Vec<Folder>, AppError> {
        let collection = db.collection::<Folder>("folders");
        let mut cursor = collection
            .find(doc! { "ownerId": owner_id })
            .sort(doc! { "name": 1 })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut folders = Vec::new();
        use futures::StreamExt;
        while let Some(result) = cursor.next().await {
            let folder = result.map_err(|e| AppError::Internal(e.to_string()))?;
            folders.push(folder);
        }

        Ok(folders)
    }

    pub async fn create_folder(
        db: &Database,
        name: String,
        parent_id: Option<ObjectId>,
        is_public: bool,
        owner_id: String,
    ) -> Result<Folder, AppError> {
        let collection = db.collection::<Folder>("folders");

        let existing = collection
            .find_one(doc! {
                "name": &name,
                "parentId": parent_id,
                "ownerId": &owner_id
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if let Some(folder) = existing {
            return Ok(folder);
        }

        let now = chrono::Utc::now();
        let new_folder = Folder {
            id: None,
            name,
            parent_id,
            owner_id,
            is_public,
            created_at: now,
            updated_at: now,
        };

        let res = collection
            .insert_one(&new_folder)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let inserted_id = res
            .inserted_id
            .as_object_id()
            .ok_or_else(|| AppError::Internal("Không lấy được ObjectId mới tạo".into()))?;

        let mut created = new_folder;
        created.id = Some(inserted_id);

        Ok(created)
    }

    pub async fn get_owned_folder(
        db: &Database,
        folder_id: &ObjectId,
        owner_id: &str,
    ) -> Result<Folder, AppError> {
        let collection = db.collection::<Folder>("folders");
        let folder = collection
            .find_one(doc! { "_id": folder_id })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy thư mục".to_string()))?;

        if folder.owner_id != owner_id {
            return Err(AppError::Forbidden);
        }

        Ok(folder)
    }

    pub async fn update_folder(
        db: &Database,
        folder_id: &ObjectId,
        owner_id: &str,
        name: Option<String>,
        parent_id: Option<Option<ObjectId>>,
        is_public: Option<bool>,
    ) -> Result<Folder, AppError> {
        let mut folder = Self::get_owned_folder(db, folder_id, owner_id).await?;
        let mut update_doc = doc! {};

        if let Some(n) = name {
            folder.name = n.clone();
            update_doc.insert("name", n);
        }
        if let Some(p) = parent_id {
            folder.parent_id = p;
            update_doc.insert("parentId", p);
        }
        if let Some(pub_val) = is_public {
            folder.is_public = pub_val;
            update_doc.insert("isPublic", pub_val);
        }

        let now = chrono::Utc::now();
        folder.updated_at = now;
        update_doc.insert("updatedAt", now);

        if !update_doc.is_empty() {
            let collection = db.collection::<Folder>("folders");
            collection
                .update_one(doc! { "_id": folder_id }, doc! { "$set": update_doc })
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }

        Ok(folder)
    }

    pub async fn delete_folder_recursive(
        db: &Database,
        r2: &R2Service,
        folder_id: &ObjectId,
        owner_id: &str,
    ) -> Result<(), AppError> {
        let _ = Self::get_owned_folder(db, folder_id, owner_id).await?;

        let folder_coll = db.collection::<Folder>("folders");
        let file_coll = db.collection::<File>("files");

        use futures::StreamExt;
        let mut child_cursor = folder_coll
            .find(doc! { "parentId": folder_id, "ownerId": owner_id })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        while let Some(result) = child_cursor.next().await {
            let child = result.map_err(|e| AppError::Internal(e.to_string()))?;
            if let Some(cid) = child.id {
                Box::pin(Self::delete_folder_recursive(db, r2, &cid, owner_id)).await?;
            }
        }

        let mut file_cursor = file_coll
            .find(doc! { "folderId": folder_id, "ownerId": owner_id })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        while let Some(result) = file_cursor.next().await {
            let file = result.map_err(|e| AppError::Internal(e.to_string()))?;
            if file.external_url.is_none() {
                let _ = r2.delete_object(&file.key).await;
            }
            if let Some(fid) = file.id {
                file_coll
                    .delete_one(doc! { "_id": fid })
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
        }

        folder_coll
            .delete_one(doc! { "_id": folder_id, "ownerId": owner_id })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    pub async fn get_breadcrumb(
        db: &Database,
        folder_id: Option<ObjectId>,
        viewer_id: Option<&str>,
    ) -> Result<Vec<BreadcrumbEntry>, AppError> {
        let Some(fid) = folder_id else {
            return Ok(Vec::new());
        };

        let collection = db.collection::<Folder>("folders");
        let current = collection
            .find_one(doc! { "_id": fid })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy thư mục".to_string()))?;

        if !current.is_public && current.owner_id.as_str() != viewer_id.unwrap_or_default() {
            return Err(AppError::Forbidden);
        }

        let mut chain = Vec::new();
        let mut curr_opt = Some(current);

        while let Some(curr) = curr_opt {
            let id_str = curr.id.map(|i| i.to_hex()).unwrap_or_default();
            chain.insert(
                0,
                BreadcrumbEntry {
                    id: id_str,
                    name: curr.name.clone(),
                    path: String::new(),
                },
            );

            if let Some(pid) = curr.parent_id {
                curr_opt = collection
                    .find_one(doc! { "_id": pid })
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            } else {
                curr_opt = None;
            }
        }

        let mut current_path = String::new();
        for entry in &mut chain {
            if current_path.is_empty() {
                current_path = entry.name.clone();
            } else {
                current_path = format!("{}/{}", current_path, entry.name);
            }
            entry.path = current_path.clone();
        }

        Ok(chain)
    }

    pub async fn assert_folder_ownership(
        db: &Database,
        folder_id: &ObjectId,
        owner_id: &str,
    ) -> Result<(), AppError> {
        let collection = db.collection::<Folder>("folders");
        let folder = collection
            .find_one(doc! { "_id": folder_id })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy thư mục".to_string()))?;

        if folder.owner_id != owner_id {
            return Err(AppError::Forbidden);
        }
        Ok(())
    }
}
