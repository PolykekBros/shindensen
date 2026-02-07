use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub active_connections: Arc<DashMap<String, broadcast::Sender<String>>>,
    pub jwt_secret: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Message {
    pub id: i64,
    pub sender_id: i64,
    pub receiver_id: i64,
    pub content: String,
    pub timestamp: String, // Storing as TEXT in SQLite is simplest
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUser {
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessage {
    pub receiver_username: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // using username as subject or we can add user_id
    pub user_id: i64,
    pub username: String,
    pub exp: usize,
}
