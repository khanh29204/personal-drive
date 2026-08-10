use axum::http::{HeaderValue, Method};
use axum::routing::{get, patch, post};
use axum::Router;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;

use crate::app_state::AppState;
use crate::handlers::{auth_handler, file_handler, folder_handler, web_handler};

pub fn create_router(state: AppState) -> Router {
    let allowed_methods = vec![
        Method::GET,
        Method::POST,
        Method::PATCH,
        Method::DELETE,
        Method::PUT,
        Method::OPTIONS,
    ];

    let cors = if let Some(origin) = &state.config.cors_origin {
        if let Ok(header_val) = origin.parse::<HeaderValue>() {
            CorsLayer::new()
                .allow_origin(header_val)
                .allow_methods(allowed_methods.clone())
                .allow_headers(AllowHeaders::mirror_request())
                .allow_credentials(true)
        } else {
            CorsLayer::new()
                .allow_origin(AllowOrigin::mirror_request())
                .allow_methods(allowed_methods.clone())
                .allow_headers(AllowHeaders::mirror_request())
                .allow_credentials(true)
        }
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::mirror_request())
            .allow_methods(allowed_methods)
            .allow_headers(AllowHeaders::mirror_request())
            .allow_credentials(true)
    };

    Router::new()
        // Web EJS/HTML routes
        .route("/", get(web_handler::render_home))
        .route("/files/{id}/download", get(web_handler::redirect_to_download))
        .route("/files/{id}/view", get(web_handler::redirect_to_view))
        // Auth API
        .route("/api/auth/login", post(auth_handler::login))
        .route("/api/auth/logout", post(auth_handler::logout))
        // Folder API
        .route(
            "/api/folders",
            get(folder_handler::list_folders).post(folder_handler::create_folder),
        )
        .route(
            "/api/folders/{id}",
            patch(folder_handler::update_folder).delete(folder_handler::delete_folder),
        )
        // File API
        .route("/api/files", get(file_handler::list_files))
        .route("/api/files/upload-url", post(file_handler::request_upload_url))
        .route("/api/files/link", post(file_handler::create_linked_file))
        .route("/api/files/storage/quota", get(file_handler::get_storage_quota))
        .route("/api/files/storage/clean-orphans", post(file_handler::clean_orphan_files))
        .route(
            "/api/files/storage/orphans",
            get(file_handler::list_orphan_files).delete(file_handler::delete_specific_orphans),
        )
        .route("/api/files/{id}/complete", post(file_handler::complete_upload))
        .route(
            "/api/files/{id}/multipart/part-urls",
            post(file_handler::create_part_urls),
        )
        .route(
            "/api/files/{id}/multipart/complete",
            post(file_handler::complete_multipart_upload),
        )
        .route("/api/files/{id}/abort-upload", post(file_handler::abort_upload))
        .route("/api/files/{id}/download-url", get(file_handler::get_download_url))
        .route(
            "/api/files/{id}",
            patch(file_handler::update_file).delete(file_handler::delete_file),
        )
        // Static files fallback (public/style.css, public/app.js)
        .fallback_service(ServeDir::new("public"))
        .layer(cors)
        .with_state(state)
}
