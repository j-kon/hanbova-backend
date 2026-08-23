use async_trait::async_trait;
use chrono::Utc;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::{ProtectedMessageRow, UserPaymentProfileResponse},
};

pub type Result<T> = std::result::Result<T, ApiError>;

#[async_trait]
pub trait ProtectedMessageRepository: Send + Sync {
    async fn upsert_user_payment_keys(
        &self,
        user_id: Uuid,
        protected_pubkey: &str,
        transport_pubkey: &str,
    ) -> Result<()>;

    async fn find_payment_profile_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserPaymentProfileResponse>>;

    async fn save_message(&self, message: &ProtectedMessageRow) -> Result<()>;

    async fn find_message_by_id(&self, id: Uuid) -> Result<Option<ProtectedMessageRow>>;

    async fn find_inbox_by_user_id(&self, recipient_user_id: Uuid) -> Result<Vec<ProtectedMessageRow>>;

    async fn find_outbox_by_user_id(&self, sender_user_id: Uuid) -> Result<Vec<ProtectedMessageRow>>;

    async fn update_message_status(&self, id: Uuid, status: &str) -> Result<()>;
}

/// PostgreSQL implementation of ProtectedMessageRepository
pub struct PgProtectedMessageRepository {
    pool: PgPool,
}

impl PgProtectedMessageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProtectedMessageRepository for PgProtectedMessageRepository {
    async fn upsert_user_payment_keys(
        &self,
        user_id: Uuid,
        protected_pubkey: &str,
        transport_pubkey: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO user_payment_keys (user_id, protected_payment_pubkey, transport_encryption_pubkey, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (user_id) DO UPDATE SET
                protected_payment_pubkey = EXCLUDED.protected_payment_pubkey,
                transport_encryption_pubkey = EXCLUDED.transport_encryption_pubkey,
                updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(protected_pubkey)
        .bind(transport_pubkey)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to upsert user payment keys: {e}")))?;

        Ok(())
    }

    async fn find_payment_profile_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserPaymentProfileResponse>> {
        let clean_username = username.strip_prefix('@').unwrap_or(username);

        let row = sqlx::query(
            r#"
            SELECT u.username, k.protected_payment_pubkey, k.transport_encryption_pubkey
            FROM users u
            JOIN user_payment_keys k ON u.id = k.user_id
            WHERE LOWER(u.username) = LOWER($1)
            "#,
        )
        .bind(clean_username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to find payment profile: {e}")))?;

        Ok(row.map(|r| {
            let uname: String = r.get("username");
            let handle = if uname.starts_with('@') {
                uname.clone()
            } else {
                format!("@{}", uname)
            };
            UserPaymentProfileResponse {
                username: uname,
                handle,
                protected_payment_pubkey: r.get("protected_payment_pubkey"),
                transport_encryption_pubkey: r.get("transport_encryption_pubkey"),
            }
        }))
    }

    async fn save_message(&self, message: &ProtectedMessageRow) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO protected_messages (
                id, payment_intent_id, sender_user_id, recipient_user_id,
                sender_username, recipient_username, encrypted_payload, payload_version,
                status, created_at, acknowledged_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(message.id)
        .bind(message.payment_intent_id)
        .bind(message.sender_user_id)
        .bind(message.recipient_user_id)
        .bind(&message.sender_username)
        .bind(&message.recipient_username)
        .bind(&message.encrypted_payload)
        .bind(message.payload_version)
        .bind(&message.status)
        .bind(message.created_at)
        .bind(message.acknowledged_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to save protected message: {e}")))?;

        Ok(())
    }

    async fn find_message_by_id(&self, id: Uuid) -> Result<Option<ProtectedMessageRow>> {
        let row = sqlx::query_as::<_, ProtectedMessageRow>(
            r#"
            SELECT id, payment_intent_id, sender_user_id, recipient_user_id,
                   sender_username, recipient_username, encrypted_payload, payload_version,
                   status, created_at, acknowledged_at
            FROM protected_messages
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to find protected message: {e}")))?;

        Ok(row)
    }

    async fn find_inbox_by_user_id(&self, recipient_user_id: Uuid) -> Result<Vec<ProtectedMessageRow>> {
        let rows = sqlx::query_as::<_, ProtectedMessageRow>(
            r#"
            SELECT id, payment_intent_id, sender_user_id, recipient_user_id,
                   sender_username, recipient_username, encrypted_payload, payload_version,
                   status, created_at, acknowledged_at
            FROM protected_messages
            WHERE recipient_user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(recipient_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch inbox: {e}")))?;

        Ok(rows)
    }

    async fn find_outbox_by_user_id(&self, sender_user_id: Uuid) -> Result<Vec<ProtectedMessageRow>> {
        let rows = sqlx::query_as::<_, ProtectedMessageRow>(
            r#"
            SELECT id, payment_intent_id, sender_user_id, recipient_user_id,
                   sender_username, recipient_username, encrypted_payload, payload_version,
                   status, created_at, acknowledged_at
            FROM protected_messages
            WHERE sender_user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(sender_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch outbox: {e}")))?;

        Ok(rows)
    }

    async fn update_message_status(&self, id: Uuid, status: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE protected_messages
            SET status = $1, acknowledged_at = CASE WHEN acknowledged_at IS NULL THEN NOW() ELSE acknowledged_at END
            WHERE id = $2
            "#,
        )
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update message status: {e}")))?;

        Ok(())
    }
}

use crate::auth::repository::UserRepository;
use std::sync::Arc;

/// In-memory implementation of ProtectedMessageRepository for testing
#[derive(Default, Clone)]
pub struct InMemoryProtectedMessageRepository {
    keys: Arc<RwLock<HashMap<Uuid, (String, String)>>>,
    usernames: Arc<RwLock<HashMap<String, Uuid>>>,
    messages: Arc<RwLock<HashMap<Uuid, ProtectedMessageRow>>>,
    user_repo: Option<Arc<dyn UserRepository>>,
}

impl InMemoryProtectedMessageRepository {
    pub fn new(user_repo: Option<Arc<dyn UserRepository>>) -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            usernames: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            user_repo,
        }
    }
}

#[async_trait]
impl ProtectedMessageRepository for InMemoryProtectedMessageRepository {
    async fn upsert_user_payment_keys(
        &self,
        user_id: Uuid,
        protected_pubkey: &str,
        transport_pubkey: &str,
    ) -> Result<()> {
        let mut k = self.keys.write().await;
        k.insert(user_id, (protected_pubkey.to_string(), transport_pubkey.to_string()));
        Ok(())
    }

    async fn find_payment_profile_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserPaymentProfileResponse>> {
        let clean = username.strip_prefix('@').unwrap_or(username).to_lowercase();
        let user_id = if let Some(ref ur) = self.user_repo {
            let u_opt = ur
                .find_by_username_or_email(&clean)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            match u_opt {
                Some(u) => u.id,
                None => return Ok(None),
            }
        } else {
            let u = self.usernames.read().await;
            match u.get(&clean) {
                Some(id) => *id,
                None => return Ok(None),
            }
        };

        let k = self.keys.read().await;
        let Some((protected_pubkey, transport_pubkey)) = k.get(&user_id) else {
            return Ok(None);
        };

        Ok(Some(UserPaymentProfileResponse {
            username: clean.clone(),
            handle: format!("@{clean}"),
            protected_payment_pubkey: protected_pubkey.clone(),
            transport_encryption_pubkey: transport_pubkey.clone(),
        }))
    }

    async fn save_message(&self, message: &ProtectedMessageRow) -> Result<()> {
        let mut m = self.messages.write().await;
        m.insert(message.id, message.clone());
        Ok(())
    }

    async fn find_message_by_id(&self, id: Uuid) -> Result<Option<ProtectedMessageRow>> {
        let m = self.messages.read().await;
        Ok(m.get(&id).cloned())
    }

    async fn find_inbox_by_user_id(&self, recipient_user_id: Uuid) -> Result<Vec<ProtectedMessageRow>> {
        let m = self.messages.read().await;
        let mut list: Vec<_> = m
            .values()
            .filter(|v| v.recipient_user_id == recipient_user_id)
            .cloned()
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(list)
    }

    async fn find_outbox_by_user_id(&self, sender_user_id: Uuid) -> Result<Vec<ProtectedMessageRow>> {
        let m = self.messages.read().await;
        let mut list: Vec<_> = m
            .values()
            .filter(|v| v.sender_user_id == sender_user_id)
            .cloned()
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(list)
    }

    async fn update_message_status(&self, id: Uuid, status: &str) -> Result<()> {
        let mut m = self.messages.write().await;
        if let Some(msg) = m.get_mut(&id) {
            msg.status = status.to_string();
            if msg.acknowledged_at.is_none() {
                msg.acknowledged_at = Some(Utc::now());
            }
        }
        Ok(())
    }
}
