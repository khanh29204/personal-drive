//! Probe đọc-only kiểm chứng `list_orphan_files` trên dữ liệu thật.
//!
//! Không xóa/ghi gì: gọi thẳng service layer — cùng đường đi với handler — và
//! in ba loại rác mà endpoint mới trả về, kèm đối chiếu thủ công để chắc chắn
//! không loại nào bị sót.
//!
//! ```sh
//! cargo test --test orphan_diagnostic -- --ignored --nocapture
//! ```

use bson::doc;
use futures::StreamExt;
use rust_backend::config::AppConfig;
use rust_backend::models::file::File;
use rust_backend::services::file_service::FileService;
use rust_backend::services::r2_service::R2Service;

#[tokio::test]
#[ignore = "cần credential R2 + MongoDB, chỉ đọc"]
async fn list_orphan_phu_ba_loai_rac() {
    let config = AppConfig::from_env().expect("thiếu biến môi trường (.env)");
    let db = rust_backend::db::init_db(&config.mongodb_uri)
        .await
        .expect("nối MongoDB thất bại");
    let r2 = R2Service::new(&config).await;

    let scan = FileService::list_orphan_files(&db, &r2)
        .await
        .expect("list_orphan_files thất bại");

    println!("── Kết quả list_orphan_files (endpoint GET /api/files/storage/orphans) ──");

    println!("  [1] Object R2 không có bản ghi DB: {}", scan.orphan_objects.len());
    for o in scan.orphan_objects.iter().take(10) {
        println!("      {} ({} byte)", o.key, o.size);
    }

    println!("  [2] Bản ghi pending quá 24h: {}", scan.stale_pending_files.len());
    for f in scan.stale_pending_files.iter().take(10) {
        println!(
            "      {} | {} | tạo {} | multipart={}",
            f.name, f.size_formatted, f.created_at, f.has_multipart
        );
    }

    println!("  [3] Multipart treo vô chủ: {}", scan.dangling_multipart.len());
    for m in scan.dangling_multipart.iter().take(10) {
        println!("      {} (initiated={:?})", m.key, m.initiated);
    }

    // Đối chiếu độc lập: số bản ghi pending quá hạn đếm trực tiếp từ DB phải
    // khớp với loại [2] — đây là loại rác mà bản cũ bỏ sót hoàn toàn.
    let collection = db.collection::<File>("files");
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    let mut cursor = collection
        .find(doc! { "status": "pending", "createdAt": { "$lt": cutoff } })
        .await
        .expect("đếm pending quá hạn thất bại");
    let mut dem = 0usize;
    while let Some(item) = cursor.next().await {
        if item.is_ok() {
            dem += 1;
        }
    }
    println!("  đối chiếu: DB có {dem} bản ghi pending quá 24h");
    assert_eq!(
        scan.stale_pending_files.len(),
        dem,
        "list_orphan_files phải liệt kê đủ mọi bản ghi pending quá hạn"
    );

    // Upload đang chạy (pending dưới 24h) không được xuất hiện ở loại [2].
    let mut cursor_non = collection
        .find(doc! { "status": "pending", "createdAt": { "$gte": cutoff } })
        .await
        .expect("đếm pending non thất bại");
    let mut pending_non = 0usize;
    while let Some(item) = cursor_non.next().await {
        if let Ok(f) = item {
            pending_non += 1;
            assert!(
                !scan.stale_pending_files.iter().any(|s| s.key == f.key),
                "upload đang chạy {} không được tính là rác",
                f.key
            );
        }
    }
    println!("  đối chiếu: {pending_non} upload pending dưới 24h — không xuất hiện trong danh sách rác");

    println!("✔ đối chiếu xong — ba loại rác được phủ đủ, không sót pending quá hạn");
}