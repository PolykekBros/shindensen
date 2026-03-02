use axum::{
    extract::{FromRef, FromRequestParts, State},
    http::request::Parts,
    Json, RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::errors::AppError;
use crate::models::{AppState, AuthResponse, Claims, CreateUser, User, UserId};

const JWT_EXPIRATION: usize = 3600 * 24; // 24 hours

#[derive(Clone)]
pub struct AuthenticatedUser {
    pub user_id: UserId,
    pub username: String,
}

impl AuthenticatedUser {
    pub fn validate_token(token: &str, secret: &str) -> Result<Self, AppError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::AuthError("Invalid token".to_string()))?;
        Ok(AuthenticatedUser {
            user_id: token_data.claims.user_id,
            username: token_data.claims.username,
        })
    }
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
        Self::validate_token(bearer.token(), &app_state.jwt_secret)
    }
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
