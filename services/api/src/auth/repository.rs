use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::auth::models::User;

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PasswordResetTokenRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create_user(&self, user: User) -> Result<User, sqlx::Error>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, sqlx::Error>;
    async fn find_by_username_or_email(&self, login: &str) -> Result<Option<User>, sqlx::Error>;
    async fn update_password_hash(&self, user_id: Uuid, new_hash: &str) -> Result<(), sqlx::Error>;

    async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error>;
    async fn validate_and_revoke_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<Uuid>, sqlx::Error>;
    async fn revoke_all_user_refresh_tokens(&self, user_id: Uuid) -> Result<(), sqlx::Error>;

    async fn store_password_reset_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error>;
    async fn validate_and_consume_reset_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<Uuid>, sqlx::Error>;
}

// -----------------------------------------------------------------------------
// PostgreSQL Implementation
// -----------------------------------------------------------------------------
pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn create_user(&self, user: User) -> Result<User, sqlx::Error> {
        let row = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, username, email, first_name, last_name, phone, password_hash, email_verified_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, username, email, first_name, last_name, phone, password_hash, email_verified_at, created_at, updated_at
            "#
        )
        .bind(user.id)
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(&user.phone)
        .bind(&user.password_hash)
        .bind(user.email_verified_at)
        .bind(user.created_at)
        .bind(user.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query_as::<_, User>(
            r#"
            SELECT id, username, email, first_name, last_name, phone, password_hash, email_verified_at, created_at, updated_at
            FROM users
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn find_by_username_or_email(&self, login: &str) -> Result<Option<User>, sqlx::Error> {
        let clean_login = login.trim().trim_start_matches('@').to_lowercase();

        let row = sqlx::query_as::<_, User>(
            r#"
            SELECT id, username, email, first_name, last_name, phone, password_hash, email_verified_at, created_at, updated_at
            FROM users
            WHERE LOWER(username) = $1 OR LOWER(email) = $1
            LIMIT 1
            "#
        )
        .bind(clean_login)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn update_password_hash(&self, user_id: Uuid, new_hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE users
            SET password_hash = $1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(new_hash)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let token_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(token_id)
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn validate_and_revoke_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let now = Utc::now();
        let row = sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = $1
            WHERE token_hash = $2 AND revoked_at IS NULL AND expires_at > $1
            RETURNING user_id
            "#,
        )
        .bind(now)
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get::<Uuid, _>("user_id")))
    }

    async fn revoke_all_user_refresh_tokens(&self, user_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = NOW()
            WHERE user_id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn store_password_reset_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let token_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO password_reset_tokens (id, user_id, token_hash, expires_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(token_id)
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn validate_and_consume_reset_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let now = Utc::now();
        let row = sqlx::query(
            r#"
            UPDATE password_reset_tokens
            SET used_at = $1
            WHERE token_hash = $2 AND used_at IS NULL AND expires_at > $1
            RETURNING user_id
            "#,
        )
        .bind(now)
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get::<Uuid, _>("user_id")))
    }
}

// -----------------------------------------------------------------------------
// In-Memory Implementation
// -----------------------------------------------------------------------------
#[derive(Default)]
pub struct InMemoryUserRepository {
    users: Arc<RwLock<HashMap<Uuid, User>>>,
    refresh_tokens: Arc<RwLock<HashMap<String, RefreshTokenRecord>>>,
    reset_tokens: Arc<RwLock<HashMap<String, PasswordResetTokenRecord>>>,
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn create_user(&self, user: User) -> Result<User, sqlx::Error> {
        let mut store = self.users.write().await;
        for existing in store.values() {
            if existing.username.eq_ignore_ascii_case(&user.username) {
                return Err(sqlx::Error::RowNotFound); // indicates unique constraint simulation
            }
            if existing.email.eq_ignore_ascii_case(&user.email) {
                return Err(sqlx::Error::RowNotFound);
            }
        }
        store.insert(user.id, user.clone());
        Ok(user)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let store = self.users.read().await;
        Ok(store.get(&id).cloned())
    }

    async fn find_by_username_or_email(&self, login: &str) -> Result<Option<User>, sqlx::Error> {
        let clean = login.trim().trim_start_matches('@').to_lowercase();
        let store = self.users.read().await;
        for user in store.values() {
            if user.username.to_lowercase() == clean || user.email.to_lowercase() == clean {
                return Ok(Some(user.clone()));
            }
        }
        Ok(None)
    }

    async fn update_password_hash(&self, user_id: Uuid, new_hash: &str) -> Result<(), sqlx::Error> {
        let mut store = self.users.write().await;
        if let Some(user) = store.get_mut(&user_id) {
            user.password_hash = new_hash.to_string();
            user.updated_at = Utc::now();
        }
        Ok(())
    }

    async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let mut store = self.refresh_tokens.write().await;
        store.insert(
            token_hash.to_string(),
            RefreshTokenRecord {
                id: Uuid::new_v4(),
                user_id,
                token_hash: token_hash.to_string(),
                expires_at,
                revoked_at: None,
                created_at: Utc::now(),
            },
        );
        Ok(())
    }

    async fn validate_and_revoke_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let mut store = self.refresh_tokens.write().await;
        if let Some(record) = store.get_mut(token_hash) {
            let now = Utc::now();
            if record.revoked_at.is_none() && record.expires_at > now {
                record.revoked_at = Some(now);
                return Ok(Some(record.user_id));
            }
        }
        Ok(None)
    }

    async fn revoke_all_user_refresh_tokens(&self, user_id: Uuid) -> Result<(), sqlx::Error> {
        let mut store = self.refresh_tokens.write().await;
        let now = Utc::now();
        for record in store.values_mut() {
            if record.user_id == user_id && record.revoked_at.is_none() {
                record.revoked_at = Some(now);
            }
        }
        Ok(())
    }

    async fn store_password_reset_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let mut store = self.reset_tokens.write().await;
        store.insert(
            token_hash.to_string(),
            PasswordResetTokenRecord {
                id: Uuid::new_v4(),
                user_id,
                token_hash: token_hash.to_string(),
                expires_at,
                used_at: None,
                created_at: Utc::now(),
            },
        );
        Ok(())
    }

    async fn validate_and_consume_reset_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let mut store = self.reset_tokens.write().await;
        if let Some(record) = store.get_mut(token_hash) {
            let now = Utc::now();
            if record.used_at.is_none() && record.expires_at > now {
                record.used_at = Some(now);
                return Ok(Some(record.user_id));
            }
        }
        Ok(None)
    }
}
