use bson::oid::ObjectId;
use rust_backend::models::file::{File, FileStatus};
use rust_backend::models::folder::Folder;

#[test]
fn test_folder_serialization() {
    let now = chrono::Utc::now();
    let folder = Folder {
        id: Some(ObjectId::new()),
        name: "Documents".to_string(),
        parent_id: None,
        owner_id: "user_test_1".to_string(),
        is_public: true,
        created_at: now,
        updated_at: now,
    };

    let doc = bson::to_document(&folder).unwrap();
    assert_eq!(doc.get_str("name").unwrap(), "Documents");
    assert_eq!(doc.get_str("ownerId").unwrap(), "user_test_1");
    assert!(doc.get_bool("isPublic").unwrap());
}

#[test]
fn test_file_serialization() {
    let now = chrono::Utc::now();
    let file = File {
        id: Some(ObjectId::new()),
        name: "test.pdf".to_string(),
        key: "user1/uuid-test.pdf".to_string(),
        size: 1024,
        mime_type: "application/pdf".to_string(),
        external_url: None,
        folder_id: None,
        owner_id: "user_test_1".to_string(),
        is_public: false,
        status: FileStatus::Completed,
        multipart_upload_id: None,
        views: 5,
        downloads: 2,
        created_at: now,
        updated_at: now,
    };

    let doc = bson::to_document(&file).unwrap();
    assert_eq!(doc.get_str("name").unwrap(), "test.pdf");
    assert_eq!(doc.get_i64("size").unwrap(), 1024);
    assert_eq!(doc.get_str("status").unwrap(), "completed");
    // File thường không được mang theo trường multipart: bản ghi cũ trong DB
    // không có nó, và job dọn rác coi trường này là dấu hiệu upload còn dở.
    assert!(!doc.contains_key("multipartUploadId"));
}
