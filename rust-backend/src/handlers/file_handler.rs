use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use bson::oid::ObjectId;
use serde::Deserialize;

use crate::errors::AppError;
use crate::extractors::auth::{AuthUser, OptionalAuthUser};
use crate::models::file::File;
use crate::services::file_service::{
    CleanOrphanResult, CompleteMultipartBody, DeleteOrphansBody, DeleteOrphansResult, FileService,
    OrphanScanResult, PartUrlsResult, StorageQuotaResult, UploadUrlResult,
};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadUrlBody {
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub size: i64,
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,
    #[serde(rename = "isPublic", default)]
    pub is_public: bool,
}

#[derive(Debug, Deserialize)]
pub struct PartUrlsBody {
    #[serde(rename = "partNumbers")]
    pub part_numbers: Vec<i32>,
}

#[derive(Debug, Deserialize)]
pub struct LinkFileBody {
    pub name: String,
    pub url: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFileBody {
    pub name: Option<String>,
    #[serde(rename = "isPublic")]
    pub is_public: Option<bool>,
    #[serde(
        rename = "folderId",
        default,
        deserialize_with = "crate::utils::double_option"
    )]
    pub folder_id: Option<Option<String>>,
    pub url: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

pub async fn list_files(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Query(query): Query<ListFilesQuery>,
) -> Result<Json<Vec<File>>, AppError> {
    let folder_oid = match query.folder_id {
        Some(s) if !s.trim().is_empty() => Some(
            ObjectId::parse_str(&s)
                .map_err(|_| AppError::bad_request("folderId không hợp lệ"))?,
        ),
        _ => None,
    };

    let viewer_id = user.as_ref().map(|u| u.id.as_str());
    let files = FileService::list_files(&state.db, folder_oid, viewer_id).await?;
    Ok(Json(files))
}

pub async fn request_upload_url(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<UploadUrlBody>,
) -> Result<(StatusCode, Json<UploadUrlResult>), AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::bad_request("Tên file không được để trống"));
    }
    if body.mime_type.trim().is_empty() {
        return Err(AppError::bad_request("mimeType không được để trống"));
    }
    if body.size < 0 {
        return Err(AppError::bad_request("size không hợp lệ"));
    }

    let folder_oid = match body.folder_id {
        Some(s) if !s.trim().is_empty() => Some(
            ObjectId::parse_str(&s)
                .map_err(|_| AppError::bad_request("folderId không hợp lệ"))?,
        ),
        _ => None,
    };

    let result = FileService::request_upload_url(
        &state.db,
        &state.r2,
        body.name.trim().to_string(),
        body.mime_type.trim().to_string(),
        body.size,
        folder_oid,
        body.is_public,
        user.id,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(result)))
}

pub async fn complete_upload(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<File>, AppError> {
    let file_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    let file =
        FileService::complete_upload(&state.db, &state.r2, &state.file_cache, &file_oid, &user.id)
            .await?;

    Ok(Json(file))
}

pub async fn create_part_urls(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PartUrlsBody>,
) -> Result<Json<PartUrlsResult>, AppError> {
    let file_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    let result = FileService::create_part_urls(
        &state.db,
        &state.r2,
        &file_oid,
        &user.id,
        body.part_numbers,
    )
    .await?;

    Ok(Json(result))
}

pub async fn complete_multipart_upload(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<CompleteMultipartBody>,
) -> Result<Json<File>, AppError> {
    let file_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    let file = FileService::complete_multipart_upload(
        &state.db,
        &state.r2,
        &state.file_cache,
        &file_oid,
        &user.id,
        body.parts,
    )
    .await?;

    Ok(Json(file))
}

pub async fn abort_upload(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let file_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    FileService::abort_upload(
        &state.db,
        &state.r2,
        &state.file_cache,
        &file_oid,
        &user.id,
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_download_url(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let file_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    let viewer_id = user.as_ref().map(|u| u.id.as_str());
    let url = FileService::get_download_url(
        &state.db,
        &state.r2,
        &state.file_cache,
        &file_oid,
        viewer_id,
        false,
    )
    .await?;

    Ok(Json(serde_json::json!({ "downloadUrl": url })))
}

pub async fn delete_file(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let file_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    FileService::delete_file(&state.db, &state.r2, &state.file_cache, &file_oid, &user.id).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_linked_file(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<LinkFileBody>,
) -> Result<(StatusCode, Json<File>), AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::bad_request("Tên file không được để trống"));
    }
    if body.url.trim().is_empty() {
        return Err(AppError::bad_request("URL không được để trống"));
    }

    let folder_oid = match body.folder_id {
        Some(s) if !s.trim().is_empty() => Some(
            ObjectId::parse_str(&s)
                .map_err(|_| AppError::bad_request("folderId không hợp lệ"))?,
        ),
        _ => None,
    };

    let file = FileService::create_linked_file(
        &state.db,
        body.name.trim().to_string(),
        body.url.trim().to_string(),
        body.mime_type.trim().to_string(),
        folder_oid,
        user.id,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(file)))
}

pub async fn update_file(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateFileBody>,
) -> Result<Json<File>, AppError> {
    let file_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    let folder_oid_opt = match body.folder_id {
        Some(Some(s)) if !s.trim().is_empty() => Some(Some(
            ObjectId::parse_str(&s)
                .map_err(|_| AppError::bad_request("folderId không hợp lệ"))?,
        )),
        Some(_) => Some(None),
        None => None,
    };

    let name_opt = body.name.filter(|n| !n.trim().is_empty());
    let url_opt = body.url.filter(|u| !u.trim().is_empty());
    let mime_opt = body.mime_type.filter(|m| !m.trim().is_empty());

    let file = FileService::update_file(
        &state.db,
        &state.file_cache,
        &file_oid,
        &user.id,
        name_opt,
        body.is_public,
        folder_oid_opt,
        url_opt,
        mime_opt,
    )
    .await?;

    Ok(Json(file))
}

pub async fn get_storage_quota(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<StorageQuotaResult>, AppError> {
    let quota = FileService::get_storage_quota(&state.db, Some(&user.id)).await?;
    Ok(Json(quota))
}

pub async fn clean_orphan_files(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> Result<Json<CleanOrphanResult>, AppError> {
    let result = FileService::clean_orphan_files(&state.db, &state.r2).await?;
    Ok(Json(result))
}

pub async fn list_orphan_files(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> Result<Json<OrphanScanResult>, AppError> {
    let list = FileService::list_orphan_files(&state.db, &state.r2).await?;
    Ok(Json(list))
}

pub async fn delete_specific_orphans(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Json(body): Json<DeleteOrphansBody>,
) -> Result<Json<DeleteOrphansResult>, AppError> {
    let result = FileService::delete_specific_orphans(&state.db, &state.r2, body).await?;
    Ok(Json(result))
}
