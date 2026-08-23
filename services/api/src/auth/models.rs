use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub login: String, // email or username
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserProfileResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgotPasswordResponse {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_reset_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileResponse {
    pub id: Uuid,
    pub username: String,
    pub handle: String, // e.g. "@username"
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub display_name: String,
    pub phone: Option<String>,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserProfileResponse {
    fn from(user: User) -> Self {
        let handle = if user.username.starts_with('@') {
            user.username.clone()
        } else {
            format!("@{}", user.username)
        };
        let display_name = format!("{} {}", user.first_name, user.last_name)
            .trim()
            .to_string();

        Self {
            id: user.id,
            username: user.username,
            handle,
            email: user.email,
            first_name: user.first_name,
            last_name: user.last_name,
            display_name,
            phone: user.phone,
            email_verified: user.email_verified_at.is_some(),
            created_at: user.created_at,
        }
    }
}
