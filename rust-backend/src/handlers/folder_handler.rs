use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use bson::oid::ObjectId;
use serde::Deserialize;

use crate::errors::AppError;
use crate::extractors::auth::{AuthUser, OptionalAuthUser};
use crate::models::folder::Folder;
use crate::services::folder_service::FolderService;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ListFoldersQuery {
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFolderBody {
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(rename = "isPublic", default)]
    pub is_public: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFolderBody {
    pub name: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<Option<String>>,
    #[serde(rename = "isPublic")]
    pub is_public: Option<bool>,
}

pub async fn list_folders(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Query(query): Query<ListFoldersQuery>,
) -> Result<Json<Vec<Folder>>, AppError> {
    let parent_oid = match query.parent_id {
        Some(s) if !s.trim().is_empty() => Some(
            ObjectId::parse_str(&s)
                .map_err(|_| AppError::bad_request("parentId không hợp lệ"))?,
        ),
        _ => None,
    };

    let viewer_id = user.as_ref().map(|u| u.id.as_str());
    let folders = FolderService::list_folders(&state.db, parent_oid, viewer_id).await?;
    Ok(Json(folders))
}

pub async fn create_folder(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateFolderBody>,
) -> Result<(StatusCode, Json<Folder>), AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::bad_request("Tên thư mục không được để trống"));
    }

    let parent_oid = match body.parent_id {
        Some(s) if !s.trim().is_empty() => Some(
            ObjectId::parse_str(&s)
                .map_err(|_| AppError::bad_request("parentId không hợp lệ"))?,
        ),
        _ => None,
    };

    let folder = FolderService::create_folder(
        &state.db,
        body.name.trim().to_string(),
        parent_oid,
        body.is_public,
        user.id,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(folder)))
}

pub async fn update_folder(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateFolderBody>,
) -> Result<Json<Folder>, AppError> {
    let folder_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    let parent_oid_opt = match body.parent_id {
        Some(Some(s)) if !s.trim().is_empty() => Some(Some(
            ObjectId::parse_str(&s)
                .map_err(|_| AppError::bad_request("parentId không hợp lệ"))?,
        )),
        Some(_) => Some(None),
        None => None,
    };

    let name_opt = body.name.filter(|n| !n.trim().is_empty());

    let folder = FolderService::update_folder(
        &state.db,
        &folder_oid,
        &user.id,
        name_opt,
        parent_oid_opt,
        body.is_public,
    )
    .await?;

    Ok(Json(folder))
}

pub async fn delete_folder(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let folder_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    FolderService::delete_folder_recursive(&state.db, &state.r2, &folder_oid, &user.id).await?;

    Ok(StatusCode::NO_CONTENT)
}
