use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    auth::{
        jwt::{generate_access_token, JwtError},
        models::{
            AuthResponse, ForgotPasswordRequest, ForgotPasswordResponse, LoginRequest,
            RefreshTokenRequest, RegisterRequest, ResetPasswordRequest, User, UserProfileResponse,
        },
        password::{hash_password, verify_password},
        repository::{hash_token, UserRepository},
    },
    error::{ApiError, Result},
};

#[derive(Clone)]
pub struct AuthService {
    repo: Arc<dyn UserRepository>,
    jwt_secret: String,
    access_token_expiry_minutes: i64,
    refresh_token_expiry_days: i64,
    is_development: bool,
}

impl AuthService {
    pub fn new(repo: Arc<dyn UserRepository>, jwt_secret: String, is_development: bool) -> Self {
        Self {
            repo,
            jwt_secret,
            access_token_expiry_minutes: 15,
            refresh_token_expiry_days: 30,
            is_development,
        }
    }

    pub async fn register(&self, req: RegisterRequest) -> Result<AuthResponse> {
        let clean_username = req.username.trim().trim_start_matches('@').to_lowercase();
        if clean_username.len() < 3 || clean_username.len() > 30 {
            return Err(ApiError::BadRequest(
                "Username must be between 3 and 30 characters".to_string(),
            ));
        }

        let clean_email = req.email.trim().to_lowercase();
        if !clean_email.contains('@') || !clean_email.contains('.') {
            return Err(ApiError::BadRequest("Invalid email address".to_string()));
        }

        if req.password.len() < 8 {
            return Err(ApiError::BadRequest(
                "Password must be at least 8 characters long".to_string(),
            ));
        }

        // Check for existing user
        if self
            .repo
            .find_by_username_or_email(&clean_username)
            .await?
            .is_some()
        {
            return Err(ApiError::BadRequest(
                "Username is already taken".to_string(),
            ));
        }
        if self
            .repo
            .find_by_username_or_email(&clean_email)
            .await?
            .is_some()
        {
            return Err(ApiError::BadRequest(
                "Email is already registered".to_string(),
            ));
        }

        let password_hash = hash_password(&req.password)
            .map_err(|e| ApiError::Internal(format!("Failed to hash password: {}", e)))?;

        let user = User {
            id: Uuid::new_v4(),
            username: clean_username.clone(),
            email: clean_email,
            first_name: req.first_name.trim().to_string(),
            last_name: req.last_name.trim().to_string(),
            phone: req.phone.map(|p| p.trim().to_string()),
            password_hash,
            email_verified_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let created_user = self.repo.create_user(user).await?;
        self.generate_auth_response(created_user).await
    }

    pub async fn login(&self, req: LoginRequest) -> Result<AuthResponse> {
        let clean_login = req.login.trim().to_lowercase();
        let user = self
            .repo
            .find_by_username_or_email(&clean_login)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Invalid login credentials".to_string()))?;

        let is_valid = verify_password(&req.password, &user.password_hash)
            .map_err(|_| ApiError::BadRequest("Invalid login credentials".to_string()))?;

        if !is_valid {
            return Err(ApiError::BadRequest(
                "Invalid login credentials".to_string(),
            ));
        }

        self.generate_auth_response(user).await
    }

    pub async fn refresh(&self, req: RefreshTokenRequest) -> Result<AuthResponse> {
        let token_hash = hash_token(&req.refresh_token);
        let user_id = self
            .repo
            .validate_and_revoke_refresh_token(&token_hash)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Invalid or expired refresh token".to_string()))?;

        let user = self
            .repo
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

        self.generate_auth_response(user).await
    }

    pub async fn logout(&self, user_id: Uuid) -> Result<()> {
        self.repo.revoke_all_user_refresh_tokens(user_id).await?;
        Ok(())
    }

    pub async fn forgot_password(
        &self,
        req: ForgotPasswordRequest,
    ) -> Result<ForgotPasswordResponse> {
        let clean_email = req.email.trim().to_lowercase();
        let user = self.repo.find_by_username_or_email(&clean_email).await?;

        let mut dev_token = None;

        if let Some(u) = user {
            let raw_token: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(32)
                .map(char::from)
                .collect();

            let token_hash = hash_token(&raw_token);
            let expires_at = Utc::now() + Duration::hours(1);

            self.repo
                .store_password_reset_token(u.id, &token_hash, expires_at)
                .await?;

            if self.is_development {
                dev_token = Some(raw_token);
            }
        }

        Ok(ForgotPasswordResponse {
            message: "If an account matches that email, a password reset link has been issued."
                .to_string(),
            dev_reset_token: dev_token,
        })
    }

    pub async fn reset_password(&self, req: ResetPasswordRequest) -> Result<()> {
        if req.new_password.len() < 8 {
            return Err(ApiError::BadRequest(
                "Password must be at least 8 characters long".to_string(),
            ));
        }

        let token_hash = hash_token(&req.token);
        let user_id = self
            .repo
            .validate_and_consume_reset_token(&token_hash)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Invalid or expired reset token".to_string()))?;

        let new_hash = hash_password(&req.new_password)
            .map_err(|e| ApiError::Internal(format!("Failed to hash password: {}", e)))?;

        self.repo.update_password_hash(user_id, &new_hash).await?;
        self.repo.revoke_all_user_refresh_tokens(user_id).await?;

        Ok(())
    }

    pub async fn get_profile(&self, user_id: Uuid) -> Result<UserProfileResponse> {
        let user = self
            .repo
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

        Ok(user.into())
    }

    async fn generate_auth_response(&self, user: User) -> Result<AuthResponse> {
        let access_token = generate_access_token(
            user.id,
            &user.username,
            &self.jwt_secret,
            self.access_token_expiry_minutes,
        )
        .map_err(|e| match e {
            JwtError::Creation(msg) => ApiError::Internal(msg),
            _ => ApiError::Internal("Token error".to_string()),
        })?;

        let raw_refresh_token: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(48)
            .map(char::from)
            .collect();

        let refresh_token_hash = hash_token(&raw_refresh_token);
        let refresh_expires_at = Utc::now() + Duration::days(self.refresh_token_expiry_days);

        self.repo
            .store_refresh_token(user.id, &refresh_token_hash, refresh_expires_at)
            .await?;

        Ok(AuthResponse {
            access_token,
            refresh_token: raw_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.access_token_expiry_minutes * 60,
            user: user.into(),
        })
    }
}
