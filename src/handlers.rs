use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket},
        FromRequestParts, Path, State, WebSocketUpgrade,
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

use crate::errors::AppError;
use crate::models::{AppState, AuthResponse, Claims, CreateUser, Message, SendMessage, User};

const JWT_EXPIRATION: usize = 3600 * 24; // 24 hours

// --- Authentication Extractor ---
#[derive(Clone)]
pub struct AuthenticatedUser {
    pub user_id: i64,
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
        // Extract the token from the Authorization header
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::AuthError("Missing Authorization header".to_string()))?;

        let app_state = AppState::from_ref(state);
        let jwt_secret = &app_state.jwt_secret;

        // Decode the token
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

// Need to define FromRef for AppState -> AppState (Identity)
use axum::extract::FromRef;

// --- Handlers ---

pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateUser>,
) -> Result<Json<AuthResponse>, AppError> {
    // Check if user exists
    let user_opt: Option<User> = sqlx::query_as!(
        User,
        r#"SELECT id as "id!", username as "username!" FROM users WHERE username = ?"#,
        payload.username
    )
    .fetch_optional(&state.pool)
    .await?;

    let user = match user_opt {
        Some(u) => u,
        None => {
            // Create user
            let id = sqlx::query!("INSERT INTO users (username) VALUES (?)", payload.username)
                .execute(&state.pool)
                .await?
                .last_insert_rowid();

            User {
                id,
                username: payload.username.clone(),
            }
        }
    };

    // Generate JWT
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

pub async fn send_message_handler(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<SendMessage>,
) -> Result<Json<serde_json::Value>, AppError> {
    process_message(&state, &auth, payload).await?;
    Ok(Json(serde_json::json!({"status": "sent"})))
}

async fn process_message(
    state: &AppState,
    auth: &AuthenticatedUser,
    payload: SendMessage,
) -> Result<(), AppError> {
    // Check receiver exists
    let receiver_opt: Option<User> = sqlx::query_as!(
        User,
        r#"SELECT id as "id!", username as "username!" FROM users WHERE username = ?"#,
        payload.receiver_username
    )
    .fetch_optional(&state.pool)
    .await?;

    let receiver = receiver_opt.ok_or(AppError::BadRequest("Receiver not found".to_string()))?;

    // Create message with timestamp
    // ISO 8601 format
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Insert into DB
    sqlx::query!(
        "INSERT INTO messages (sender_id, receiver_id, content, timestamp) VALUES (?, ?, ?, ?)",
        auth.user_id,
        receiver.id,
        payload.content,
        timestamp
    )
    .execute(&state.pool)
    .await?;

    // Check if receiver is connected via WebSocket
    if let Some(sender_tx) = state.active_connections.get(&receiver.username) {
        // Send message to receiver
        // Format the message as JSON string
        let msg_out = serde_json::json!({
            "sender": auth.username,
            "content": payload.content,
            "timestamp": timestamp
        });

        let _ = sender_tx.send(msg_out.to_string());
    }

    Ok(())
}

pub async fn get_history_handler(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(target_username): Path<String>,
) -> Result<Json<Vec<Message>>, AppError> {
    // Find target user
    let target_opt: Option<User> = sqlx::query_as!(
        User,
        r#"SELECT id as "id!", username as "username!" FROM users WHERE username = ?"#,
        target_username
    )
    .fetch_optional(&state.pool)
    .await?;

    let target = target_opt.ok_or(AppError::BadRequest("Target user not found".to_string()))?;

    // Retrieve messages where (sender=me AND receiver=them) OR (sender=them AND receiver=me)

    let messages = sqlx::query_as!(
        Message,
        r#"
        SELECT id as "id!", sender_id as "sender_id!", receiver_id as "receiver_id!", content as "content!", CAST(timestamp AS TEXT) as "timestamp!"
        FROM messages
        WHERE (sender_id = ? AND receiver_id = ?)
           OR (sender_id = ? AND receiver_id = ?)
        ORDER BY timestamp ASC
        "#,
        auth.user_id,
        target.id,
        target.id,
        auth.user_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(messages))
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

    // Check if user already has a broadcast channel
    let tx = state
        .active_connections
        .entry(auth.username.clone())
        .or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(100);
            tx
        })
        .clone();

    let mut rx = tx.subscribe();

    // Spawn task to forward broadcast messages to websocket
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Err(_e) = sender.send(WsMessage::Text(msg)).await {
                // Client disconnected
                break;
            }
        }
    });

    // Handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                WsMessage::Text(text) => {
                    if let Ok(payload) = serde_json::from_str::<SendMessage>(&text) {
                        if let Err(e) = process_message(&state, &auth, payload).await {
                            tracing::error!("Failed to process WS message: {:?}", e);
                        }
                    }
                }
                WsMessage::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}
