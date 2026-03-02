use axum::{
    extract::{
        ws::{CloseFrame, Message as WsMessage, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::borrow::Cow;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time;

use crate::errors::AppError;
use crate::handlers::auth::AuthenticatedUser;
use crate::models::{AppState, Message, UserId, WsMessageIn};

#[derive(serde::Deserialize)]
#[serde(tag = "op", content = "d")]
enum WsOpIn {
    #[serde(rename = "IDENTIFY")]
    Identify { token: String },
}

#[derive(serde::Serialize)]
struct WsOpOut {
    op: &'static str,
    d: ReadyData,
}

#[derive(serde::Serialize)]
struct ReadyData {
    user_id: UserId,
}

pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // 1. Authentication Phase: Identify within 10s
    let auth_res = time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Ok(WsOpIn::Identify { token }) = serde_json::from_str::<WsOpIn>(&text) {
                        match AuthenticatedUser::validate_token(&token, &state.jwt_secret) {
                            Ok(auth) => return Ok(auth),
                            Err(_) => return Err(4004),
                        }
                    }
                }
                Ok(WsMessage::Close(_)) => return Err(0),
                Err(_) => return Err(0),
                _ => {} // Ignore Ping/Pong/Binary during identification
            }
        }
        Err(0)
    })
    .await;

    let auth = match auth_res {
        Ok(Ok(auth)) => auth,
        Ok(Err(4004)) => {
            let _ = sender
                .send(WsMessage::Close(Some(CloseFrame {
                    code: 4004,
                    reason: Cow::Borrowed("Authentication Failed"),
                })))
                .await;
            return;
        }
        _ => return, // Timeout or connection drop
    };

    // 2. Promotion: Send READY and set up broadcast
    let ready = serde_json::to_string(&WsOpOut {
        op: "READY",
        d: ReadyData {
            user_id: auth.user_id,
        },
    })
    .unwrap();
    let _ = sender.send(WsMessage::Text(ready)).await;

    let tx = state
        .active_connections
        .entry(auth.username.clone())
        .or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(100);
            tx
        })
        .clone();
    let mut rx = tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Err(_) = sender.send(WsMessage::Text(msg)).await {
                break;
            }
        }
    });

    let state_clone = state.clone();
    let auth_clone = auth.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                WsMessage::Text(text) => {
                    if let Ok(payload) = serde_json::from_str::<WsMessageIn>(&text) {
                        if let Err(e) = process_message(&state_clone, &auth_clone, payload).await {
                            tracing::error!("Failed to process WS message: {:?}", e);
                        }
                    }
                }
                WsMessage::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

async fn process_message(
    state: &AppState,
    auth: &AuthenticatedUser,
    payload: WsMessageIn,
) -> Result<(), AppError> {
    let has_content = payload
        .content
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let files_in = payload.files.unwrap_or_default();
    let has_files = !files_in.is_empty();
    if !has_content && !has_files {
        return Err(AppError::BadRequest(
            "Message must have text or at least one file".to_string(),
        ));
    }
    if files_in.len() > 10 {
        return Err(AppError::BadRequest(
            "Maximum 10 files allowed per message".to_string(),
        ));
    }
    for file in &files_in {
        if file.size_bytes > 10 * 1024 * 1024 {
            return Err(AppError::BadRequest(format!(
                "File {} exceeds 10MB limit",
                file.filename
            )));
        }
    }
    let is_participant = sqlx::query_scalar!(
        "SELECT 1 FROM chat_participants WHERE chat_id = ? AND user_id = ?",
        payload.chat_id,
        auth.user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .is_some();
    if !is_participant {
        return Err(AppError::AuthError(
            "Not authorized to send to this chat".to_string(),
        ));
    }
    let timestamp = chrono::Utc::now().to_rfc3339();
    let mut tx = state.pool.begin().await?;
    let message_id = sqlx::query_scalar!(
        "INSERT INTO messages (chat_id, sender_id, content, timestamp) VALUES (?, ?, ?, ?) RETURNING id",
        payload.chat_id,
        auth.user_id,
        payload.content,
        timestamp
    )
    .fetch_one(&mut *tx)
    .await?;
    let mut db_files = Vec::new();
    for file_in in files_in {
        let file_id = sqlx::query_scalar!(
            r#"
            INSERT INTO files (type, url, filename, mime_type, size_bytes)
            VALUES (?, ?, ?, ?, ?) RETURNING id
            "#,
            file_in.r#type,
            file_in.url,
            file_in.filename,
            file_in.mime_type,
            file_in.size_bytes
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO message_files (message_id, file_id) VALUES (?, ?)",
            message_id,
            file_id
        )
        .execute(&mut *tx)
        .await?;
        db_files.push(crate::models::MediaAsset {
            id: file_id,
            r#type: file_in.r#type,
            url: file_in.url,
            filename: file_in.filename,
            mime_type: file_in.mime_type,
            size_bytes: file_in.size_bytes,
            created_at: timestamp.clone(),
        });
    }
    tx.commit().await?;
    struct Participant {
        username: String,
    }
    let participants = sqlx::query_as!(
        Participant,
        r#"
        SELECT u.username as "username!"
        FROM chat_participants cp
        JOIN users u ON cp.user_id = u.id
        WHERE cp.chat_id = ?
        "#,
        payload.chat_id
    )
    .fetch_all(&state.pool)
    .await?;
    let msg = Message {
        id: message_id,
        chat_id: payload.chat_id,
        sender_id: auth.user_id,
        content: payload.content,
        timestamp,
        files: db_files,
    };
    let msg_json = serde_json::to_string(&msg).unwrap();
    for p in participants {
        if let Some(sender_tx) = state.active_connections.get(&p.username) {
            let _ = sender_tx.send(msg_json.clone());
        }
    }
    Ok(())
}
