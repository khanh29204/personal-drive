use rust_backend::utils::file_display::{format_bytes, get_file_icon};
use rust_backend::services::r2_service::build_object_key;

#[test]
fn test_format_bytes_utility() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    assert_eq!(format_bytes(10 * 1024 * 1024 * 1024), "10.0 GB");
}

#[test]
fn test_get_file_icon_utility() {
    assert_eq!(get_file_icon("test.jpg", "image/jpeg", false), "fa-file-image");
    assert_eq!(get_file_icon("test.mp4", "video/mp4", false), "fa-file-video");
    assert_eq!(get_file_icon("test.mp3", "audio/mp3", false), "fa-file-audio");
    assert_eq!(get_file_icon("test.pdf", "application/pdf", false), "fa-file-pdf");
    assert_eq!(get_file_icon("test.zip", "application/zip", false), "fa-file-archive");
    assert_eq!(get_file_icon("test.docx", "application/msword", false), "fa-file-word");
    assert_eq!(get_file_icon("test.txt", "text/plain", false), "fa-file-alt");
    assert_eq!(get_file_icon("unknown", "application/octet-stream", false), "fa-file");
}

#[test]
fn test_build_object_key_utility() {
    let owner_id = "user123";
    let original_name = "test file @ 123.png";
    let key = build_object_key(owner_id, original_name);

    assert!(key.starts_with("user123/"));
    assert!(key.contains("test_file___123.png"));
}
