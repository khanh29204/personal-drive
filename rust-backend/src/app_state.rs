use std::sync::Arc;
use moka::future::Cache;
use mongodb::Database;

use crate::config::AppConfig;
use crate::models::file::File;
use crate::services::r2_service::R2Service;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: Database,
    pub r2: R2Service,
    pub file_cache: Cache<String, File>,
    pub jinja: Arc<minijinja::Environment<'static>>,
}
