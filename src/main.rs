use axum::{
    routing::{get, post},
    Router,
};
use dashmap::DashMap;
use dotenvy::dotenv;
use sqlx::sqlite::SqlitePoolOptions;
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

mod errors;
mod handlers;
mod models;

use handlers::{get_history_handler, initiate_direct_chat_handler, login_handler, ws_handler};
use models::AppState;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create pool");
    let state = AppState {
        pool,
        active_connections: Arc::new(DashMap::new()),
        jwt_secret,
    };
    let app = Router::new()
        .route("/login", post(login_handler))
        .route("/chats/initiate", post(initiate_direct_chat_handler))
        .route("/chats/:chat_id/messages", get(get_history_handler))
        .route("/ws", get(ws_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
