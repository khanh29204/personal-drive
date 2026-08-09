use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<ObjectId>,
    #[serde(rename = "ownerId")]
    pub owner_id: String,
    #[serde(rename = "isPublic", default)]
    pub is_public: bool,
    #[serde(rename = "createdAt", with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "updatedAt", with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
