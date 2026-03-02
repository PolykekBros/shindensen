use axum::{
    extract::{Path, State},
    Json,
};

use crate::errors::AppError;
use crate::handlers::auth::AuthenticatedUser;
use crate::models::{
    AppState, Chat, ChatHistoryResponse, ChatId, ChatStatus, ChatType, InitiateChat,
    InitiateDirectChatResponse, Message,
};

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

pub async fn initiate_direct_chat_handler(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<InitiateChat>,
) -> Result<Json<InitiateDirectChatResponse>, AppError> {
    use crate::models::User;

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
    let chat_id = sqlx::query_scalar!(
        "INSERT INTO chats (chat_type) VALUES (?) RETURNING id",
        "direct"
    )
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

    Ok(Json(ChatHistoryResponse { chat_id, messages }))
}
