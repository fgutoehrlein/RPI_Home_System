use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Room {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_dm: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: Uuid,
    pub room_id: Uuid,
    pub author_id: u32,
    pub text_md: String,
    pub created_at: i64,
    pub edited_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attachment {
    pub id: Uuid,
    pub message_id: Uuid,
    pub file_id: String,
    pub file_name: String,
    pub mime: Option<String>,
    pub size_bytes: i64,
}
