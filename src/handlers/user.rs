use axum::{
    extract::{Multipart, Path, Query, State},
    Json,
};

use crate::errors::AppError;
use crate::handlers::auth::AuthenticatedUser;
use crate::models::{AppState, FileUploadResponse, User, UserId, UserSearchQuery};

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
        let filename = field.file_name().unwrap_or("unknown").to_string();
        let mime_type = field.content_type().map(|m| m.to_string());
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

        tokio::fs::write(&save_path, data)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to save file: {}", e)))?;

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
