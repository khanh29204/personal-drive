use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub key: String,
    pub size: i64,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "externalUrl")]
    pub external_url: Option<String>,
    #[serde(rename = "folderId")]
    pub folder_id: Option<ObjectId>,
    #[serde(rename = "ownerId")]
    pub owner_id: String,
    #[serde(rename = "isPublic", default)]
    pub is_public: bool,
    pub status: FileStatus,
    /// `uploadId` của multipart upload đang dở. Server tự lưu thay vì nhận từ
    /// client để client không ghép part vào một upload của người khác. `None`
    /// với file upload bằng PUT thường và với mọi bản ghi tạo trước khi có
    /// multipart, nên phải `default`.
    #[serde(rename = "multipartUploadId", default, skip_serializing_if = "Option::is_none")]
    pub multipart_upload_id: Option<String>,
    #[serde(default)]
    pub views: i64,
    #[serde(default)]
    pub downloads: i64,
    #[serde(rename = "createdAt", with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "updatedAt", with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
