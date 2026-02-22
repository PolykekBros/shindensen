use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket},
        FromRef, FromRequestParts, Multipart, Path, Query, State, WebSocketUpgrade,
    },
    http::request::Parts,
    response::IntoResponse,
    Json, RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use futures::{sink::SinkExt, stream::StreamExt};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

use crate::models::{
    AppState, AuthResponse, Chat, ChatId, ChatHistoryResponse, ChatType, Claims, CreateUser,
    FileUploadResponse, InitiateChat, Message, User, UserId, UserSearchQuery, WsMessageIn,
};
use crate::{
    errors::AppError,
    models::{ChatStatus, InitiateDirectChatResponse},
};

const JWT_EXPIRATION: usize = 3600 * 24; // 24 hours

#[derive(Clone)]
pub struct AuthenticatedUser {
    pub user_id: UserId,
    pub username: String,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::AuthError("Missing Authorization header".to_string()))?;
        let app_state = AppState::from_ref(state);
        let jwt_secret = &app_state.jwt_secret;
        let token_data = decode::<Claims>(
            bearer.token(),
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::AuthError("Invalid token".to_string()))?;
        Ok(AuthenticatedUser {
            user_id: token_data.claims.user_id,
            username: token_data.claims.username,
        })
    }
}

pub async fn upload_handler(
    State(_state): State<AppState>,
    _auth: AuthenticatedUser,
    mut multipart: Multipart,
) -> Result<Json<FileUploadResponse>, AppError> {
    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        let filename = field
            .file_name()
            .unwrap_or("unknown")
            .to_string();
        let mime_type = field
            .content_type()
            .map(|m| m.to_string());
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let size_bytes = data.len() as i64;

        let extension = std::path::Path::new(&filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("bin");
        
        let unique_filename = format!("{}.{}", uuid::Uuid::new_v4(), extension);
        let save_path = format!("uploads/{}", unique_filename);

        tokio::fs::create_dir_all("uploads").await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to create uploads directory: {}", e))
        })?;

        tokio::fs::write(&save_path, data).await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to save file: {}", e))
        })?;

        let url = format!("/uploads/{}", unique_filename);

        return Ok(Json(FileUploadResponse {
            url,
            filename,
            mime_type,
            size_bytes,
        }));
    }

    Err(AppError::BadRequest("No file provided".to_string()))
}

pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateUser>,
) -> Result<Json<AuthResponse>, AppError> {
    let user: Option<User> = sqlx::query_as!(
        User,
        r#"SELECT id as "id!", username as "username!", display_name, bio, image_id FROM users WHERE username = ?"#,
        payload.username
    )
    .fetch_optional(&state.pool)
    .await?;
    let user = match user {
        Some(u) => u,
        None => {
            let id = sqlx::query_scalar!(
                "INSERT INTO users (username) VALUES (?) RETURNING id",
                payload.username
            )
            .fetch_one(&state.pool)
            .await?;

            User {
                id,
                username: payload.username.clone(),
                display_name: None,
                bio: None,
                image_id: None,
            }
        }
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: user.username.clone(),
        user_id: user.id,
        username: user.username.clone(),
        exp: now + JWT_EXPIRATION,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|_| AppError::InternalServerError("Token creation failed".to_string()))?;
    Ok(Json(AuthResponse { token }))
}

pub async fn list_chats_handler(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> Result<Json<Vec<Chat>>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT c.id as "id!", c.name, c.chat_type as "chat_type: ChatType", c.created_at as "created_at!"
        FROM chats c
        JOIN chat_participants cp ON c.id = cp.chat_id
        WHERE cp.user_id = ?
        ORDER BY c.created_at DESC
        "#,
        auth.user_id
    )
    .fetch_all(&state.pool)
    .await?;

    let mut chats = Vec::new();
    for row in rows {
        let participants = sqlx::query_scalar!(
            "SELECT user_id FROM chat_participants WHERE chat_id = ?",
            row.id
        )
        .fetch_all(&state.pool)
        .await?;

        chats.push(Chat {
            id: row.id,
            name: row.name,
            chat_type: row.chat_type,
            created_at: row.created_at,
            participants,
        });
    }

    Ok(Json(chats))
}

pub async fn get_user_handler(
    State(state): State<AppState>,
    Path(user_id): Path<UserId>,
) -> Result<Json<User>, AppError> {
    let user = sqlx::query_as!(
        User,
        r#"SELECT id as "id!", username as "username!", display_name, bio, image_id FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("User with ID {} not found", user_id)))?;

    Ok(Json(user))
}

pub async fn search_users_handler(
    State(state): State<AppState>,
    Query(query): Query<UserSearchQuery>,
) -> Result<Json<Vec<User>>, AppError> {
    let users = if let Some(username) = query.username {
        let pattern = format!("%{}%", username);
        sqlx::query_as!(
            User,
            r#"SELECT id as "id!", username as "username!", display_name, bio, image_id FROM users WHERE username LIKE ?"#,
            pattern
        )
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as!(
            User,
            r#"SELECT id as "id!", username as "username!", display_name, bio, image_id FROM users"#
        )
        .fetch_all(&state.pool)
        .await?
    };

    Ok(Json(users))
}

pub async fn initiate_direct_chat_handler(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<InitiateChat>,
) -> Result<Json<InitiateDirectChatResponse>, AppError> {
    let target: User = sqlx::query_as!(
        User,
        r#"SELECT id as "id!", username as "username!", display_name, bio, image_id FROM users WHERE id = ?"#,
        payload.target_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Target user not found".to_string()))?;
    let chat_id = sqlx::query_scalar!(
        r#"
        SELECT c.id
        FROM chats c
        JOIN chat_participants cp1 ON c.id = cp1.chat_id
        JOIN chat_participants cp2 ON c.id = cp2.chat_id
        WHERE c.chat_type = 'direct'
          AND cp1.user_id = ?
          AND cp2.user_id = ?
        LIMIT 1
        "#,
        auth.user_id,
        target.id
    )
    .fetch_optional(&state.pool)
    .await?;
    if let Some(chat_id) = chat_id {
        return Ok(Json(InitiateDirectChatResponse {
            chat_id,
            status: ChatStatus::Exists,
        }));
    }
    let mut tx = state.pool.begin().await?;
    let chat_id = sqlx::query_scalar!("INSERT INTO chats (chat_type) VALUES (?) RETURNING id", "direct")
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query!(
        "INSERT INTO chat_participants (chat_id, user_id) VALUES (?, ?), (?, ?)",
        chat_id,
        auth.user_id,
        chat_id,
        target.id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(InitiateDirectChatResponse {
        chat_id,
        status: ChatStatus::Created,
    }))
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

pub async fn get_chat_handler(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(chat_id): Path<ChatId>,
) -> Result<Json<Chat>, AppError> {
    let is_participant = sqlx::query_scalar!(
        "SELECT 1 FROM chat_participants WHERE chat_id = ? AND user_id = ?",
        chat_id,
        auth.user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .is_some();

    if !is_participant {
        return Err(AppError::AuthError(
            "Not authorized to view this chat info".to_string(),
        ));
    }

    let row = sqlx::query!(
        r#"
        SELECT id as "id!", name, chat_type as "chat_type: ChatType", created_at as "created_at!"
        FROM chats
        WHERE id = ?
        "#,
        chat_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Chat with ID {} not found", chat_id)))?;

    let participants = sqlx::query_scalar!(
        "SELECT user_id FROM chat_participants WHERE chat_id = ?",
        chat_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(Chat {
        id: row.id,
        name: row.name,
        chat_type: row.chat_type,
        created_at: row.created_at,
        participants,
    }))
}

pub async fn get_history_handler(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(chat_id): Path<ChatId>,
) -> Result<Json<ChatHistoryResponse>, AppError> {
    let is_participant = sqlx::query_scalar!(
        "SELECT 1 FROM chat_participants WHERE chat_id = ? AND user_id = ?",
        chat_id,
        auth.user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .is_some();
    if !is_participant {
        return Err(AppError::AuthError(
            "Not authorized to view this chat".to_string(),
        ));
    }
    let mut messages = sqlx::query_as::<_, Message>(
        r#"
        SELECT id, chat_id, sender_id, content, timestamp
        FROM messages
        WHERE chat_id = ?
        ORDER BY timestamp ASC
        "#,
    )
    .bind(chat_id)
    .fetch_all(&state.pool)
    .await?;
    for msg in &mut messages {
        let files = sqlx::query_as!(
            crate::models::MediaAsset,
            r#"
            SELECT f.id as "id!", f.type as "type: crate::models::FileType", f.url as "url!", f.filename as "filename!", f.mime_type, f.size_bytes as "size_bytes!", f.created_at as "created_at!"
            FROM files f
            JOIN message_files mf ON f.id = mf.file_id
            WHERE mf.message_id = ?
            "#,
            msg.id
        )
        .fetch_all(&state.pool)
        .await?;
        msg.files = files;
    }
    Ok(Json(ChatHistoryResponse {
        chat_id,
        messages,
    }))
}

pub async fn ws_handler(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, auth))
}

async fn handle_socket(socket: WebSocket, state: AppState, auth: AuthenticatedUser) {
    let (mut sender, mut receiver) = socket.split();
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
            if let Err(_e) = sender.send(WsMessage::Text(msg)).await {
                // Client disconnected
                break;
            }
        }
    });
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                WsMessage::Text(text) => {
                    if let Ok(payload) = serde_json::from_str::<WsMessageIn>(&text) {
                        if let Err(e) = process_message(&state, &auth, payload).await {
                            tracing::error!("Failed to process WS message: {:?}", e);
                            // Optionally send error back to user via WS?
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
