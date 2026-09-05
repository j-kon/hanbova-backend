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

// Keep every PostgreSQL read aligned with `ProtectedMessageRow`.  Omitting one
// of these columns makes sqlx fail at runtime when decoding the row, which is
// particularly easy to miss because the in-memory repository does not use SQL
// decoding.
const PROTECTED_MESSAGE_COLUMNS: &str = "id, payment_intent_id, sender_user_id, recipient_user_id, sender_username, recipient_username, encrypted_payload, payload_version, status, recipient_transport_key_fingerprint, recipient_p2pk_key_fingerprint, wallet_environment, created_at, acknowledged_at";

// These are delivery/coordination reports, not independent proof of settlement.
// Retries are idempotent; terminal reports must never overwrite one another.
fn allowed_message_sources(target: &str) -> Result<&'static [&'static str]> {
    match target {
        "delivered" => Ok(&["delivered"]),
        "acknowledged" => Ok(&["delivered", "acknowledged"]),
        "claimed" => Ok(&["delivered", "acknowledged", "claimed"]),
        "refunded" => Ok(&["delivered", "acknowledged", "refunded"]),
        _ => Err(ApiError::BadRequest(
            "Unknown protected message status".into(),
        )),
    }
}

#[async_trait]
pub trait ProtectedMessageRepository: Send + Sync {
    async fn upsert_user_payment_keys(
        &self,
        user_id: Uuid,
        wallet_environment: &str,
        protected_pubkey: &str,
        transport_pubkey: &str,
    ) -> Result<()>;

    async fn find_payment_profile_by_username(
        &self,
        username: &str,
        wallet_environment: &str,
    ) -> Result<Option<UserPaymentProfileResponse>>;

    async fn save_message(&self, message: &ProtectedMessageRow) -> Result<()>;

    async fn find_message_by_id(&self, id: Uuid) -> Result<Option<ProtectedMessageRow>>;

    async fn find_inbox_by_user_id(
        &self,
        recipient_user_id: Uuid,
    ) -> Result<Vec<ProtectedMessageRow>>;

    async fn find_outbox_by_user_id(
        &self,
        sender_user_id: Uuid,
    ) -> Result<Vec<ProtectedMessageRow>>;

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
        wallet_environment: &str,
        protected_pubkey: &str,
        transport_pubkey: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO user_payment_keys (user_id, wallet_environment, protected_payment_pubkey, transport_encryption_pubkey, updated_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (user_id, wallet_environment) DO UPDATE SET
                protected_payment_pubkey = EXCLUDED.protected_payment_pubkey,
                transport_encryption_pubkey = EXCLUDED.transport_encryption_pubkey,
                updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(wallet_environment)
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
        wallet_environment: &str,
    ) -> Result<Option<UserPaymentProfileResponse>> {
        let clean_username = username.strip_prefix('@').unwrap_or(username);

        let row = sqlx::query(
            r#"
            SELECT u.username, k.wallet_environment, k.protected_payment_pubkey, k.transport_encryption_pubkey
            FROM users u
            JOIN user_payment_keys k ON u.id = k.user_id
            WHERE (LOWER(u.username) = LOWER($1) OR LOWER(u.email) = LOWER($1)) AND k.wallet_environment = $2
            "#,
        )
        .bind(clean_username)
        .bind(wallet_environment)
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
                wallet_environment: r.get("wallet_environment"),
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
                status, recipient_transport_key_fingerprint, recipient_p2pk_key_fingerprint,
                wallet_environment, created_at, acknowledged_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
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
        .bind(&message.recipient_transport_key_fingerprint)
        .bind(&message.recipient_p2pk_key_fingerprint)
        .bind(&message.wallet_environment)
        .bind(message.created_at)
        .bind(message.acknowledged_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to save protected message: {e}")))?;

        Ok(())
    }

    async fn find_message_by_id(&self, id: Uuid) -> Result<Option<ProtectedMessageRow>> {
        let row = sqlx::query_as::<_, ProtectedMessageRow>(&format!(
            "SELECT {PROTECTED_MESSAGE_COLUMNS} FROM protected_messages WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to find protected message: {e}")))?;

        Ok(row)
    }

    async fn find_inbox_by_user_id(
        &self,
        recipient_user_id: Uuid,
    ) -> Result<Vec<ProtectedMessageRow>> {
        let rows = sqlx::query_as::<_, ProtectedMessageRow>(
            &format!(
                "SELECT {PROTECTED_MESSAGE_COLUMNS} FROM protected_messages WHERE recipient_user_id = $1 ORDER BY created_at DESC"
            ),
        )
        .bind(recipient_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch inbox: {e}")))?;

        Ok(rows)
    }

    async fn find_outbox_by_user_id(
        &self,
        sender_user_id: Uuid,
    ) -> Result<Vec<ProtectedMessageRow>> {
        let rows = sqlx::query_as::<_, ProtectedMessageRow>(
            &format!(
                "SELECT {PROTECTED_MESSAGE_COLUMNS} FROM protected_messages WHERE sender_user_id = $1 ORDER BY created_at DESC"
            ),
        )
        .bind(sender_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch outbox: {e}")))?;

        Ok(rows)
    }

    async fn update_message_status(&self, id: Uuid, status: &str) -> Result<()> {
        let allowed = allowed_message_sources(status)?;
        let result = sqlx::query(
            r#"
            UPDATE protected_messages
            SET status = $1, acknowledged_at = CASE WHEN acknowledged_at IS NULL THEN NOW() ELSE acknowledged_at END
            WHERE id = $2 AND status = ANY($3)
            "#,
        )
        .bind(status)
        .bind(id)
        .bind(allowed)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update message status: {e}")))?;

        if result.rows_affected() == 0 {
            if self.find_message_by_id(id).await?.is_none() {
                return Err(ApiError::NotFound(format!(
                    "Protected message {id} not found"
                )));
            }
            return Err(ApiError::Conflict(
                "Protected message status cannot move backwards or replace a terminal report"
                    .into(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn check_message_status_race(
        repo: &dyn ProtectedMessageRepository,
        sender: Uuid,
        recipient: Uuid,
    ) {
        let row = ProtectedMessageRow {
            id: Uuid::new_v4(),
            payment_intent_id: None,
            sender_user_id: sender,
            recipient_user_id: recipient,
            sender_username: "alice".into(),
            recipient_username: "bob".into(),
            encrypted_payload: "opaque-test-ciphertext".into(),
            payload_version: 1,
            status: "delivered".into(),
            recipient_transport_key_fingerprint: None,
            recipient_p2pk_key_fingerprint: None,
            wallet_environment: None,
            created_at: Utc::now(),
            acknowledged_at: None,
        };
        repo.save_message(&row).await.unwrap();
        repo.update_message_status(row.id, "acknowledged")
            .await
            .unwrap();
        let acknowledged = repo
            .find_message_by_id(row.id)
            .await
            .unwrap()
            .unwrap()
            .acknowledged_at;
        assert!(repo
            .update_message_status(row.id, "delivered")
            .await
            .is_err());
        let (claim, refund) = tokio::join!(
            repo.update_message_status(row.id, "claimed"),
            repo.update_message_status(row.id, "refunded")
        );
        assert_ne!(
            claim.is_ok(),
            refund.is_ok(),
            "only one terminal report can win"
        );
        let winner = repo.find_message_by_id(row.id).await.unwrap().unwrap();
        for status in ["delivered", "acknowledged", "unknown"] {
            assert!(repo.update_message_status(row.id, status).await.is_err());
        }
        repo.update_message_status(row.id, &winner.status)
            .await
            .unwrap();
        let retried = repo.find_message_by_id(row.id).await.unwrap().unwrap();
        assert_eq!(retried.status, winner.status);
        assert_eq!(retried.acknowledged_at, acknowledged);
        assert!(repo
            .update_message_status(Uuid::new_v4(), "claimed")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn memory_message_status_is_monotonic() {
        check_message_status_race(
            &InMemoryProtectedMessageRepository::new(None),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
        .await;
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires an isolated PostgreSQL test server; run explicitly in CI"]
    async fn postgres_message_status_is_monotonic(pool: PgPool) {
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        for (id, username) in [(sender, "alice"), (recipient, "bob")] {
            sqlx::query("INSERT INTO users (id, username, email) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(username)
                .bind(format!("{username}@example.test"))
                .execute(&pool)
                .await
                .unwrap();
        }
        check_message_status_race(&PgProtectedMessageRepository::new(pool), sender, recipient)
            .await;
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires an isolated PostgreSQL test server; run explicitly in CI"]
    async fn postgres_message_metadata_round_trips(pool: sqlx::PgPool) {
        use super::{PgProtectedMessageRepository, ProtectedMessageRepository};
        use crate::models::ProtectedMessageRow;
        use chrono::Utc;
        use uuid::Uuid;
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        for (id, username) in [(sender, "alice"), (recipient, "bob")] {
            sqlx::query("INSERT INTO users (id, username, email) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(username)
                .bind(format!("{username}@example.test"))
                .execute(&pool)
                .await
                .unwrap();
        }
        let repo = PgProtectedMessageRepository::new(pool);
        for populated in [false, true] {
            let row = ProtectedMessageRow {
                id: Uuid::new_v4(),
                payment_intent_id: None,
                sender_user_id: sender,
                recipient_user_id: recipient,
                sender_username: "alice".into(),
                recipient_username: "bob".into(),
                encrypted_payload: "opaque-encrypted-test-envelope".into(),
                payload_version: 1,
                status: "delivered".into(),
                recipient_transport_key_fingerprint: populated
                    .then(|| "transport-fingerprint".into()),
                recipient_p2pk_key_fingerprint: populated.then(|| "payment-fingerprint".into()),
                wallet_environment: populated.then(|| "wallet_local".into()),
                created_at: Utc::now(),
                acknowledged_at: None,
            };
            repo.save_message(&row).await.unwrap();
            let by_id = repo.find_message_by_id(row.id).await.unwrap().unwrap();
            let inbox = repo.find_inbox_by_user_id(recipient).await.unwrap();
            let outbox = repo.find_outbox_by_user_id(sender).await.unwrap();
            for read in [
                &by_id,
                inbox.iter().find(|item| item.id == row.id).unwrap(),
                outbox.iter().find(|item| item.id == row.id).unwrap(),
            ] {
                assert_eq!(read.encrypted_payload, row.encrypted_payload);
                assert_eq!(read.wallet_environment, row.wallet_environment);
                assert_eq!(
                    read.recipient_transport_key_fingerprint,
                    row.recipient_transport_key_fingerprint
                );
                assert_eq!(
                    read.recipient_p2pk_key_fingerprint,
                    row.recipient_p2pk_key_fingerprint
                );
            }
            assert!(repo.find_inbox_by_user_id(sender).await.unwrap().is_empty());
            assert!(repo
                .find_outbox_by_user_id(recipient)
                .await
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn protected_message_projection_covers_every_row_field() {
        for column in [
            "id",
            "payment_intent_id",
            "sender_user_id",
            "recipient_user_id",
            "sender_username",
            "recipient_username",
            "encrypted_payload",
            "payload_version",
            "status",
            "recipient_transport_key_fingerprint",
            "recipient_p2pk_key_fingerprint",
            "wallet_environment",
            "created_at",
            "acknowledged_at",
        ] {
            assert!(
                PROTECTED_MESSAGE_COLUMNS
                    .split(", ")
                    .any(|selected| selected == column),
                "projection is missing {column}"
            );
        }
    }
}

use crate::auth::repository::UserRepository;
use std::sync::Arc;

type UserEnvKey = (Uuid, String);
type KeyPairPubkeys = (String, String);
type PaymentKeyStore = Arc<RwLock<HashMap<UserEnvKey, KeyPairPubkeys>>>;

/// In-memory implementation of ProtectedMessageRepository for testing
#[derive(Default, Clone)]
pub struct InMemoryProtectedMessageRepository {
    keys: PaymentKeyStore,
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
        wallet_environment: &str,
        protected_pubkey: &str,
        transport_pubkey: &str,
    ) -> Result<()> {
        let mut k = self.keys.write().await;
        k.insert(
            (user_id, wallet_environment.to_string()),
            (protected_pubkey.to_string(), transport_pubkey.to_string()),
        );
        Ok(())
    }

    async fn find_payment_profile_by_username(
        &self,
        username: &str,
        wallet_environment: &str,
    ) -> Result<Option<UserPaymentProfileResponse>> {
        let clean = username
            .strip_prefix('@')
            .unwrap_or(username)
            .to_lowercase();
        let (user_id, canonical_username) = if let Some(ref ur) = self.user_repo {
            let u_opt = ur
                .find_by_username_or_email(&clean)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            match u_opt {
                Some(u) => (u.id, u.username),
                None => return Ok(None),
            }
        } else {
            let u = self.usernames.read().await;
            match u.get(&clean) {
                Some(id) => (*id, clean.clone()),
                None => return Ok(None),
            }
        };

        let k = self.keys.read().await;
        let Some((protected_pubkey, transport_pubkey)) =
            k.get(&(user_id, wallet_environment.to_string()))
        else {
            return Ok(None);
        };

        let handle = if canonical_username.starts_with('@') {
            canonical_username.clone()
        } else {
            format!("@{canonical_username}")
        };

        Ok(Some(UserPaymentProfileResponse {
            username: canonical_username,
            handle,
            wallet_environment: wallet_environment.to_string(),
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

    async fn find_inbox_by_user_id(
        &self,
        recipient_user_id: Uuid,
    ) -> Result<Vec<ProtectedMessageRow>> {
        let m = self.messages.read().await;
        let mut list: Vec<_> = m
            .values()
            .filter(|v| v.recipient_user_id == recipient_user_id)
            .cloned()
            .collect();
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(list)
    }

    async fn find_outbox_by_user_id(
        &self,
        sender_user_id: Uuid,
    ) -> Result<Vec<ProtectedMessageRow>> {
        let m = self.messages.read().await;
        let mut list: Vec<_> = m
            .values()
            .filter(|v| v.sender_user_id == sender_user_id)
            .cloned()
            .collect();
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(list)
    }

    async fn update_message_status(&self, id: Uuid, status: &str) -> Result<()> {
        let allowed = allowed_message_sources(status)?;
        let mut m = self.messages.write().await;
        let msg = m
            .get_mut(&id)
            .ok_or_else(|| ApiError::NotFound(format!("Protected message {id} not found")))?;
        if !allowed.contains(&msg.status.as_str()) {
            return Err(ApiError::Conflict(
                "Protected message status cannot move backwards or replace a terminal report"
                    .into(),
            ));
        }
        msg.status = status.to_string();
        if msg.acknowledged_at.is_none() {
            msg.acknowledged_at = Some(Utc::now());
        }
        Ok(())
    }
}
