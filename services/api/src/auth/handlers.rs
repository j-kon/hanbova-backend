use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts, State},
    http::{header, request::Parts, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::{
    auth::{
        jwt::validate_access_token,
        models::{
            AuthResponse, ForgotPasswordRequest, ForgotPasswordResponse, LoginRequest,
            RefreshTokenRequest, RegisterRequest, ResetPasswordRequest, UserProfileResponse,
        },
    },
    error::{ApiError, Result},
    state::AppState,
};

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::BadRequest("Missing Authorization header".to_string()))?;

        if !auth_header.starts_with("Bearer ") {
            return Err(ApiError::BadRequest(
                "Invalid Authorization format. Expected 'Bearer <token>'".to_string(),
            ));
        }

        let token = &auth_header[7..];
        let claims = validate_access_token(token, &app_state.config.jwt_secret)
            .map_err(|_| ApiError::BadRequest("Invalid or expired access token".to_string()))?;

        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| ApiError::BadRequest("Invalid subject in token".to_string()))?;

        Ok(AuthUser {
            user_id,
            username: claims.username,
        })
    }
}

pub async fn register_handler(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>)> {
    let response = state.auth_service.register(payload).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let response = state.auth_service.login(payload).await?;
    Ok(Json(response))
}

pub async fn refresh_handler(
    State(state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Result<Json<AuthResponse>> {
    let response = state.auth_service.refresh(payload).await?;
    Ok(Json(response))
}

pub async fn logout_handler(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<StatusCode> {
    state.auth_service.logout(auth_user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn forgot_password_handler(
    State(state): State<AppState>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<Json<ForgotPasswordResponse>> {
    let response = state.auth_service.forgot_password(payload).await?;
    Ok(Json(response))
}

pub async fn reset_password_handler(
    State(state): State<AppState>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<StatusCode> {
    state.auth_service.reset_password(payload).await?;
    Ok(StatusCode::OK)
}

pub async fn get_me_handler(
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<UserProfileResponse>> {
    let profile = state.auth_service.get_profile(auth_user.user_id).await?;
    Ok(Json(profile))
}
