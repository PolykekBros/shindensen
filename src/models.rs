use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;

pub type UserId = i64;
pub type ChatId = i64;
pub type MessageId = i64;
pub type FileId = i64;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub active_connections: Arc<DashMap<String, broadcast::Sender<String>>>,
    pub jwt_secret: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub image_id: Option<FileId>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum ChatType {
    Direct,
    Group,
    Server,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Chat {
    pub id: ChatId,
    pub name: Option<String>,
    pub r#type: ChatType, // 'type' is a reserved keyword in Rust
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct ChatParticipant {
    pub chat_id: ChatId,
    pub user_id: UserId,
    pub joined_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum FileType {
    Picture,
    Video,
    Audio,
    File,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct MediaAsset {
    pub id: FileId,
    pub r#type: FileType,
    pub url: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Message {
    pub id: MessageId,
    pub chat_id: ChatId,
    pub sender_id: UserId,
    pub content: Option<String>,
    pub timestamp: String,
    #[sqlx(skip)]
    pub files: Vec<MediaAsset>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUser {
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitiateChat {
    pub target_username: String, // For starting a direct chat
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileAssetIn {
    pub r#type: FileType,
    pub url: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileUploadResponse {
    pub url: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsMessageIn {
    pub chat_id: ChatId,
    pub content: Option<String>,
    pub files: Option<Vec<FileAssetIn>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub user_id: UserId,
    pub username: String,
    pub exp: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ChatStatus {
    Exists,
    Created,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitiateDirectChatResponse {
    pub chat_id: ChatId,
    pub status: ChatStatus,
}
