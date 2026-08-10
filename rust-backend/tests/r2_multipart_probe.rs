//! Probe chạy tay đối chiếu luồng multipart với R2 thật.
//!
//! Không chạy trong `cargo test` thường vì cần credential, mạng và có ghi lên
//! bucket. Chạy bằng:
//!
//! ```sh
//! cargo test --test r2_multipart_probe -- --ignored --nocapture
//! ```
//!
//! Probe ghi vài KB vào prefix `_probe/` rồi tự dọn.
//!
//! Điểm cần kiểm chứng: một multipart chỉ có đúng một part được phép nhỏ hơn
//! 5 MiB (ràng buộc kích thước tối thiểu không áp cho part cuối), nên probe
//! xác minh được toàn bộ đường đi mà chỉ tốn vài KB.
//!
//! Lưu ý `R2Service::new()` có gọi `put_bucket_cors`, nhưng API token hiện tại
//! chỉ có quyền object nên lời gọi đó luôn trả `AccessDenied` — CORS của bucket
//! được đặt tay trong Cloudflare Dashboard. Vì vậy có test riêng đo CORS bằng
//! một request thật thay vì đọc config.

use rust_backend::config::AppConfig;
use rust_backend::services::r2_service::R2Service;

const PROBE_BODY: &[u8] = b"multipart probe payload - se bi xoa ngay sau khi kiem tra xong\n";

async fn setup() -> R2Service {
    let config = AppConfig::from_env().expect("thiếu biến môi trường R2 (.env)");
    R2Service::new(&config).await
}

#[tokio::test]
#[ignore = "cần credential R2 và có ghi lên bucket thật"]
async fn multipart_di_tron_vong_tu_tao_toi_ghep() {
    let r2 = setup().await;
    let key = format!("_probe/multipart-{}.bin", uuid::Uuid::new_v4());

    let upload_id = r2
        .create_multipart_upload(&key, "application/octet-stream")
        .await
        .expect("create_multipart_upload thất bại");
    println!("✔ uploadId = {upload_id}");

    let part_url = r2
        .create_part_upload_url(&key, &upload_id, 1)
        .await
        .expect("presign UploadPart thất bại");

    // Gửi part KHÔNG kèm Content-Type, đúng như trình duyệt gửi một Blob cắt từ
    // file. Nếu URL bị ký kèm content-type thì bước này trả SignatureDoesNotMatch.
    let client = reqwest::Client::new();
    let resp = client
        .put(&part_url)
        .body(PROBE_BODY.to_vec())
        .send()
        .await
        .expect("gửi part thất bại");

    let status = resp.status();
    let etag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let _ = r2.abort_multipart_upload(&key, &upload_id).await;
        panic!("PUT part trả {status}, body: {body}");
    }

    let etag = etag.expect("R2 không trả ETag cho part");
    println!("✔ part 1 lên xong, ETag = {etag}");

    r2.complete_multipart_upload(&key, &upload_id, &[(1, etag)])
        .await
        .expect("complete_multipart_upload thất bại");

    let (exists, size) = r2.get_object_meta(&key).await.expect("head_object thất bại");
    assert!(exists, "object không tồn tại sau khi ghép");
    assert_eq!(
        size,
        Some(PROBE_BODY.len() as i64),
        "kích thước object không khớp phần đã gửi"
    );
    println!("✔ object đã ghép, size = {} byte", PROBE_BODY.len());

    r2.delete_object(&key).await.expect("dọn object probe thất bại");
    println!("✔ đã dọn {key}");
}

#[tokio::test]
#[ignore = "cần credential R2 và có ghi lên bucket thật"]
async fn abort_go_multipart_khoi_danh_sach_dang_do() {
    let r2 = setup().await;
    let key = format!("_probe/multipart-abort-{}.bin", uuid::Uuid::new_v4());

    let upload_id = r2
        .create_multipart_upload(&key, "application/octet-stream")
        .await
        .expect("create_multipart_upload thất bại");

    // Không assert rằng multipart vừa tạo có mặt ngay: R2 index danh sách này
    // trễ vài phút. Job dọn rác chỉ đụng tới upload quá 24h nên độ trễ đó vô
    // hại — ở đây chỉ cần biết endpoint gọi được và parse được.
    let uploads = r2
        .list_multipart_uploads()
        .await
        .expect("list_multipart_uploads thất bại");
    println!("✔ list_multipart_uploads trả về {} mục đang dở", uploads.len());

    r2.abort_multipart_upload(&key, &upload_id)
        .await
        .expect("abort_multipart_upload thất bại");

    let (exists, _) = r2.get_object_meta(&key).await.expect("head_object thất bại");
    assert!(!exists, "abort không được để lại object");
    println!("✔ abort chạy sạch, không để lại object");
}

/// Dọn các multipart `_probe/` treo lại từ những lần chạy trước. Danh sách được
/// index trễ nên probe không tự dọn được ngay sau khi chạy; chạy test này sau
/// đó vài phút. Chỉ đụng tới prefix `_probe/`, không chạm dữ liệu thật.
#[tokio::test]
#[ignore = "cần credential R2 và có ghi lên bucket thật"]
async fn don_multipart_probe_con_treo() {
    let r2 = setup().await;

    let uploads = r2
        .list_multipart_uploads()
        .await
        .expect("list_multipart_uploads thất bại");

    let mut aborted = 0;
    for (key, upload_id, _) in uploads {
        if !key.starts_with("_probe/") {
            continue;
        }
        match r2.abort_multipart_upload(&key, &upload_id).await {
            Ok(_) => {
                aborted += 1;
                println!("✔ đã abort {key}");
            }
            Err(e) => println!("✗ không abort được {key}: {e}"),
        }
    }

    println!("✔ dọn xong {aborted} multipart probe treo");
}

/// Kiểm chứng quyết định cho multipart trên trình duyệt: PUT một part kèm
/// header `Origin` và xem R2 có trả `Access-Control-Expose-Headers: ETag` hay
/// không. Thiếu header đó thì `xhr.getResponseHeader('ETag')` trong JS trả null
/// dù request thành công, và không ghép được các part lại.
#[tokio::test]
#[ignore = "cần credential R2 và có ghi lên bucket thật"]
async fn cors_expose_etag_cho_part_upload() {
    let r2 = setup().await;
    let key = format!("_probe/cors-etag-{}.bin", uuid::Uuid::new_v4());
    let origin = std::env::var("CORS_ORIGIN")
        .unwrap_or_else(|_| "https://old.quockhanh020924.id.vn".to_string());

    let upload_id = r2
        .create_multipart_upload(&key, "application/octet-stream")
        .await
        .expect("create_multipart_upload thất bại");

    let part_url = r2
        .create_part_upload_url(&key, &upload_id, 1)
        .await
        .expect("presign UploadPart thất bại");

    let resp = reqwest::Client::new()
        .put(&part_url)
        .header("Origin", &origin)
        .body(PROBE_BODY.to_vec())
        .send()
        .await
        .expect("gửi part thất bại");

    let status = resp.status();
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    };
    let allow_origin = header("access-control-allow-origin");
    let expose = header("access-control-expose-headers");
    let etag = header("etag");

    let _ = r2.abort_multipart_upload(&key, &upload_id).await;

    println!("  status                        = {status}");
    println!("  access-control-allow-origin   = {allow_origin:?}");
    println!("  access-control-expose-headers = {expose:?}");
    println!("  etag                          = {etag:?}");

    assert!(status.is_success(), "PUT part thất bại: {status}");
    assert!(
        allow_origin.is_some(),
        "R2 không trả access-control-allow-origin cho origin {origin} — CORS chưa cho phép origin này"
    );
    let expose = expose.expect(
        "R2 không trả access-control-expose-headers — trình duyệt sẽ không đọc được ETag, \
         phải thêm ExposeHeaders: ETag vào CORS policy của bucket",
    );
    assert!(
        expose.to_ascii_lowercase().contains("etag"),
        "expose-headers không có ETag (hiện là {expose:?})"
    );
    println!("✔ trình duyệt đọc được ETag — multipart chạy được từ web");
}
