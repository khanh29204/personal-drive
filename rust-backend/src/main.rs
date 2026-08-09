use std::sync::Arc;
use std::time::Duration;
use moka::future::Cache;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app_state;
mod config;
mod db;
mod errors;
mod extractors;
mod handlers;
mod models;
mod routes;
mod services;
mod utils;

use app_state::AppState;
use config::AppConfig;
use db::init_db;
use routes::create_router;
use services::r2_service::R2Service;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,rust_backend=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env().map_err(|e| format!("Config Error: {e}"))?;

    println!("🚀 Đang khởi tạo kết nối MongoDB...");
    let db = init_db(&config.mongodb_uri).await?;

    println!("☁️ Đang khởi tạo Cloudflare R2 S3 Service...");
    let r2 = R2Service::new(&config).await;

    println!("🎨 Đang khởi tạo MiniJinja Template Engine...");
    let mut jinja = minijinja::Environment::new();
    jinja.set_loader(minijinja::path_loader("templates"));
    jinja.add_filter("tojson", minijinja::filters::tojson);

    let jinja_arc = Arc::new(jinja);

    let file_cache = Cache::builder()
        .max_capacity(500)
        .time_to_live(Duration::from_secs(300))
        .build();

    let state = AppState {
        config: config.clone(),
        db,
        r2,
        file_cache,
        jinja: jinja_arc,
    };

    let app = create_router(state);

    let addr = format!("0.0.0.0:{}", config.port);
    println!("🌐 Server Rust đang lắng nghe tại http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
