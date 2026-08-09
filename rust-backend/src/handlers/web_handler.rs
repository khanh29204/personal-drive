use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::extractors::auth::OptionalAuthUser;
use crate::services::file_service::FileService;
use crate::services::folder_service::FolderService;
use crate::utils::file_display::{
    format_bytes, get_file_category_code, get_file_category_label, get_file_icon,
};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct RenderHomeQuery {
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,
    pub dir: Option<String>,
    #[serde(rename = "sortBy")]
    pub sort_by: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DisplayItem {
    pub id: String,
    pub name: String,
    #[serde(rename = "isDirectory")]
    pub is_directory: bool,
    pub icon: String,
    pub category: String,
    #[serde(rename = "categoryCode")]
    pub category_code: String,
    #[serde(rename = "sizeLabel")]
    pub size_label: String,
    #[serde(rename = "sizeRaw")]
    pub size_raw: i64,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub modified_date: String,
    pub href: String,
    #[serde(rename = "downloadHref")]
    pub download_href: Option<String>,
    #[serde(rename = "isPublic")]
    pub is_public: bool,
    #[serde(rename = "isOwner")]
    pub is_owner: bool,
    #[serde(rename = "externalUrl")]
    pub external_url: Option<String>,
}

pub async fn render_home(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Query(query): Query<RenderHomeQuery>,
) -> Result<Html<String>, AppError> {
    let viewer_id = user.as_ref().map(|u| u.id.as_str());

    let mut folder_oid = match query.folder_id {
        Some(s) if !s.trim().is_empty() => ObjectId::parse_str(&s).ok(),
        _ => None,
    };

    if folder_oid.is_none() {
        if let Some(dir_path) = &query.dir {
            if let Ok(resolved) = FolderService::resolve_path(&state.db, dir_path, viewer_id).await {
                folder_oid = resolved;
            }
        }
    }

    let sort_by = match query.sort_by.as_deref() {
        Some("type") => "type",
        Some("size") => "size",
        Some("date") => "date",
        _ => "name",
    };
    let order = if query.order.as_deref() == Some("desc") { "desc" } else { "asc" };

    let breadcrumb = FolderService::get_breadcrumb(&state.db, folder_oid, viewer_id).await?;
    let folders = FolderService::list_folders(&state.db, folder_oid, viewer_id).await?;
    let files = FileService::list_files(&state.db, folder_oid, viewer_id).await?;

    let current_path = if let Some(last) = breadcrumb.last() {
        last.path.clone()
    } else {
        String::new()
    };

    let mut folder_items: Vec<DisplayItem> = folders
        .into_iter()
        .map(|f| {
            let fid = f.id.map(|i| i.to_hex()).unwrap_or_default();
            let item_path = if current_path.is_empty() {
                f.name.clone()
            } else {
                format!("{}/{}", current_path, f.name)
            };
            DisplayItem {
                id: fid,
                name: f.name,
                is_directory: true,
                icon: "fa-folder".to_string(),
                category: "Thư mục".to_string(),
                category_code: "folder".to_string(),
                size_label: "-".to_string(),
                size_raw: -1,
                mime_type: String::new(),
                modified_date: f.updated_at.format("%d/%m/%Y").to_string(),
                href: format!("/?dir={}", urlencoding::encode(&item_path)),
                download_href: None,
                is_public: f.is_public,
                is_owner: viewer_id.map(|v| v == f.owner_id).unwrap_or(false),
                external_url: None,
            }
        })
        .collect();

    let mut file_items: Vec<DisplayItem> = files
        .into_iter()
        .map(|f| {
            let fid = f.id.map(|i| i.to_hex()).unwrap_or_default();
            let is_linked = f.external_url.is_some();
            let icon = get_file_icon(&f.name, &f.mime_type, is_linked);
            let category = get_file_category_label(&f.name, &f.mime_type, is_linked);
            let category_code = get_file_category_code(&f.name, &f.mime_type, is_linked).to_string();

            DisplayItem {
                id: fid.clone(),
                name: f.name.clone(),
                is_directory: false,
                icon,
                category,
                category_code,
                size_label: format_bytes(f.size),
                size_raw: f.size,
                mime_type: f.mime_type,
                modified_date: f.updated_at.format("%d/%m/%Y").to_string(),
                href: format!("/files/{}/view", fid),
                download_href: Some(format!("/files/{}/download", fid)),
                is_public: f.is_public,
                is_owner: viewer_id.map(|v| v == f.owner_id).unwrap_or(false),
                external_url: f.external_url,
            }
        })
        .collect();

    // Sort items
    let cmp = |a: &DisplayItem, b: &DisplayItem| -> std::cmp::Ordering {
        let res = match sort_by {
            "type" => {
                let ta = if a.is_directory { &a.name } else { &a.category };
                let tb = if b.is_directory { &b.name } else { &b.category };
                ta.to_lowercase().cmp(&tb.to_lowercase())
            }
            "size" => a.size_raw.cmp(&b.size_raw),
            "date" => a.modified_date.cmp(&b.modified_date),
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        };
        if order == "desc" {
            res.reverse()
        } else {
            res
        }
    };

    folder_items.sort_by(cmp);
    file_items.sort_by(cmp);

    let parent_href = if breadcrumb.len() >= 2 {
        Some(format!("/?dir={}", urlencoding::encode(&breadcrumb[breadcrumb.len() - 2].path)))
    } else if breadcrumb.len() == 1 {
        Some("/".to_string())
    } else {
        None
    };

    let all_folders = if let Some(vid) = viewer_id {
        FolderService::list_all_user_folders(&state.db, vid).await?
    } else {
        Vec::new()
    };

    let current_folder_id_str = folder_oid.map(|i| i.to_hex());
    let current_title = if let Some(last) = breadcrumb.last() {
        last.name.clone()
    } else {
        "Root".to_string()
    };

    let mut items = folder_items;
    items.extend(file_items);

    let template = state
        .jinja
        .get_template("index.html")
        .map_err(|e| AppError::Internal(format!("Template load error: {e}")))?;

    let ctx = minijinja::context! {
        user => user,
        breadcrumb => breadcrumb,
        parentHref => parent_href,
        items => items,
        sortBy => sort_by,
        order => order,
        currentFolderId => current_folder_id_str,
        allFolders => all_folders,
        currentPath => current_path,
        current_title => current_title,
    };

    let html = template
        .render(ctx)
        .map_err(|e| AppError::Internal(format!("Template render error: {e}")))?;

    Ok(Html(html))
}

pub async fn redirect_to_download(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
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

    Ok(Redirect::temporary(&url))
}

pub async fn redirect_to_view(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let file_oid =
        ObjectId::parse_str(&id).map_err(|_| AppError::bad_request("id không hợp lệ"))?;

    let viewer_id = user.as_ref().map(|u| u.id.as_str());
    let url = FileService::get_download_url(
        &state.db,
        &state.r2,
        &state.file_cache,
        &file_oid,
        viewer_id,
        true,
    )
    .await?;

    Ok(Redirect::temporary(&url))
}
