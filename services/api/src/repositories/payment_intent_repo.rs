use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hanbova_core::{PaymentIntent, PaymentStatus, SatoshiAmount};
use sqlx::{PgPool, Row};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::ApiError;

pub type Result<T> = std::result::Result<T, ApiError>;

#[async_trait]
pub trait PaymentIntentRepository: Send + Sync {
    async fn save(&self, intent: &PaymentIntent) -> Result<()>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentIntent>>;
    async fn find_by_reference(&self, reference: &str) -> Result<Option<PaymentIntent>>;
    async fn list_all(&self) -> Result<Vec<PaymentIntent>>;
    async fn find_by_user(&self, user_identifier: &str) -> Result<Vec<PaymentIntent>>;
    /// Validate and persist a transition atomically, returning its stored timestamp.
    async fn update_status(&self, id: Uuid, status: PaymentStatus) -> Result<DateTime<Utc>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use hanbova_core::PaymentType;

    async fn check_payment_status_race(repo: &dyn PaymentIntentRepository) {
        let mut intent = PaymentIntent::new(
            PaymentType::Protected,
            SatoshiAmount::new(100).unwrap(),
            "bob",
            Some("alice".into()),
            None,
            None,
        )
        .unwrap();
        intent.status = PaymentStatus::RefundAvailable;
        repo.save(&intent).await.unwrap();
        let (claim, refund) = tokio::join!(
            repo.update_status(intent.id, PaymentStatus::Claimed),
            repo.update_status(intent.id, PaymentStatus::Refunded),
        );
        assert_ne!(
            claim.is_ok(),
            refund.is_ok(),
            "exactly one terminal report must succeed"
        );
        let winner = repo.find_by_id(intent.id).await.unwrap().unwrap();
        assert_eq!(
            claim.or(refund).unwrap(),
            winner.updated_at,
            "response timestamp must match the stored value"
        );
        assert!(repo
            .update_status(intent.id, PaymentStatus::Claimable)
            .await
            .is_err());
        repo.update_status(intent.id, winner.status).await.unwrap();
        let retried = repo.find_by_id(intent.id).await.unwrap().unwrap();
        assert_eq!(retried.status, winner.status);
        assert_eq!(retried.updated_at, winner.updated_at);
        assert!(repo
            .update_status(Uuid::new_v4(), PaymentStatus::Claimed)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn memory_payment_status_preserves_terminal_winner() {
        check_payment_status_race(&InMemoryPaymentIntentRepository::new()).await;
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires an isolated PostgreSQL test server; run explicitly in CI"]
    async fn postgres_payment_status_preserves_terminal_winner(pool: PgPool) {
        // Exercise database precision loss even on hosts whose wall clock
        // already has microsecond precision. This changes only the isolated fixture.
        sqlx::query("ALTER TABLE payment_intents ALTER COLUMN updated_at TYPE TIMESTAMPTZ(3)")
            .execute(&pool)
            .await
            .unwrap();
        check_payment_status_race(&PgPaymentIntentRepository::new(pool)).await;
    }
}

/// PostgreSQL implementation of PaymentIntentRepository.
pub struct PgPaymentIntentRepository {
    pool: PgPool,
}

impl PgPaymentIntentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PaymentIntentRepository for PgPaymentIntentRepository {
    async fn save(&self, intent: &PaymentIntent) -> Result<()> {
        let payment_type = intent.payment_type.to_string();
        let status = intent.status.to_string();
        let amount = intent.amount_sats.as_u64() as i64;

        sqlx::query(
            r#"
            INSERT INTO payment_intents (
                id, payment_type, status, amount_sats, sender_id, recipient_identifier, description, expires_at, claim_reference, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                claim_reference = EXCLUDED.claim_reference,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(intent.id)
        .bind(payment_type)
        .bind(status)
        .bind(amount)
        .bind(&intent.sender_id)
        .bind(&intent.recipient_identifier)
        .bind(&intent.description)
        .bind(intent.expires_at)
        .bind(&intent.claim_reference)
        .bind(intent.created_at)
        .bind(intent.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentIntent>> {
        let row = sqlx::query(
            r#"
            SELECT id, payment_type, status, amount_sats, sender_id, recipient_identifier, description, expires_at, claim_reference, created_at, updated_at
            FROM payment_intents
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id: Uuid = row.get("id");
                let payment_type_str: String = row.get("payment_type");
                let status_str: String = row.get("status");
                let amount_sats: i64 = row.get("amount_sats");
                let sender_id: Option<String> = row.get("sender_id");
                let recipient_identifier: String = row.get("recipient_identifier");
                let description: Option<String> = row.get("description");
                let expires_at: Option<DateTime<Utc>> = row.get("expires_at");
                let claim_reference: Option<String> = row.get("claim_reference");
                let created_at: DateTime<Utc> = row.get("created_at");
                let updated_at: DateTime<Utc> = row.get("updated_at");

                let payment_type = payment_type_str.parse().map_err(ApiError::BadRequest)?;
                let status = status_str.parse().map_err(ApiError::BadRequest)?;

                Ok(Some(PaymentIntent {
                    id,
                    payment_type,
                    status,
                    amount_sats: SatoshiAmount::from_sats(amount_sats as u64),
                    sender_id,
                    recipient_identifier,
                    description,
                    expires_at,
                    claim_reference,
                    created_at,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn find_by_reference(&self, reference: &str) -> Result<Option<PaymentIntent>> {
        let row = sqlx::query(
            r#"
            SELECT id, payment_type, status, amount_sats, sender_id, recipient_identifier, description, expires_at, claim_reference, created_at, updated_at
            FROM payment_intents
            WHERE claim_reference = $1 OR id::text = $1
            "#,
        )
        .bind(reference)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id: Uuid = row.get("id");
                let payment_type_str: String = row.get("payment_type");
                let status_str: String = row.get("status");
                let amount_sats: i64 = row.get("amount_sats");
                let sender_id: Option<String> = row.get("sender_id");
                let recipient_identifier: String = row.get("recipient_identifier");
                let description: Option<String> = row.get("description");
                let expires_at: Option<DateTime<Utc>> = row.get("expires_at");
                let claim_reference: Option<String> = row.get("claim_reference");
                let created_at: DateTime<Utc> = row.get("created_at");
                let updated_at: DateTime<Utc> = row.get("updated_at");

                let payment_type = payment_type_str.parse().map_err(ApiError::BadRequest)?;
                let status = status_str.parse().map_err(ApiError::BadRequest)?;

                Ok(Some(PaymentIntent {
                    id,
                    payment_type,
                    status,
                    amount_sats: SatoshiAmount::from_sats(amount_sats as u64),
                    sender_id,
                    recipient_identifier,
                    description,
                    expires_at,
                    claim_reference,
                    created_at,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_all(&self) -> Result<Vec<PaymentIntent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, payment_type, status, amount_sats, sender_id, recipient_identifier, description, expires_at, claim_reference, created_at, updated_at
            FROM payment_intents
            ORDER BY created_at DESC
            LIMIT 50
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            let id: Uuid = row.get("id");
            let payment_type_str: String = row.get("payment_type");
            let status_str: String = row.get("status");
            let amount_sats: i64 = row.get("amount_sats");
            let sender_id: Option<String> = row.get("sender_id");
            let recipient_identifier: String = row.get("recipient_identifier");
            let description: Option<String> = row.get("description");
            let expires_at: Option<DateTime<Utc>> = row.get("expires_at");
            let claim_reference: Option<String> = row.get("claim_reference");
            let created_at: DateTime<Utc> = row.get("created_at");
            let updated_at: DateTime<Utc> = row.get("updated_at");

            let payment_type = payment_type_str.parse().map_err(ApiError::BadRequest)?;
            let status = status_str.parse().map_err(ApiError::BadRequest)?;

            results.push(PaymentIntent {
                id,
                payment_type,
                status,
                amount_sats: SatoshiAmount::from_sats(amount_sats as u64),
                sender_id,
                recipient_identifier,
                description,
                expires_at,
                claim_reference,
                created_at,
                updated_at,
            });
        }

        Ok(results)
    }

    async fn find_by_user(&self, user_identifier: &str) -> Result<Vec<PaymentIntent>> {
        let clean_id = user_identifier.strip_prefix('@').unwrap_or(user_identifier);
        let rows = sqlx::query(
            r#"
            SELECT id, payment_type, status, amount_sats, sender_id, recipient_identifier, description, expires_at, claim_reference, created_at, updated_at
            FROM payment_intents
            WHERE sender_id = $1 OR recipient_identifier = $1 OR recipient_identifier = $2
            ORDER BY created_at DESC
            LIMIT 50
            "#,
        )
        .bind(user_identifier)
        .bind(clean_id)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            let id: Uuid = row.get("id");
            let payment_type_str: String = row.get("payment_type");
            let status_str: String = row.get("status");
            let amount_sats: i64 = row.get("amount_sats");
            let sender_id: Option<String> = row.get("sender_id");
            let recipient_identifier: String = row.get("recipient_identifier");
            let description: Option<String> = row.get("description");
            let expires_at: Option<DateTime<Utc>> = row.get("expires_at");
            let claim_reference: Option<String> = row.get("claim_reference");
            let created_at: DateTime<Utc> = row.get("created_at");
            let updated_at: DateTime<Utc> = row.get("updated_at");

            let payment_type = payment_type_str.parse().map_err(ApiError::BadRequest)?;
            let status = status_str.parse().map_err(ApiError::BadRequest)?;

            results.push(PaymentIntent {
                id,
                payment_type,
                status,
                amount_sats: SatoshiAmount::from_sats(amount_sats as u64),
                sender_id,
                recipient_identifier,
                description,
                expires_at,
                claim_reference,
                created_at,
                updated_at,
            });
        }

        Ok(results)
    }

    async fn update_status(&self, id: Uuid, status: PaymentStatus) -> Result<DateTime<Utc>> {
        let mut transaction = self.pool.begin().await?;
        let row =
            sqlx::query("SELECT status, updated_at FROM payment_intents WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or_else(|| ApiError::NotFound(format!("Payment intent {id} not found")))?;
        let current: PaymentStatus = row
            .try_get::<String, _>("status")?
            .parse()
            .map_err(ApiError::BadRequest)?;
        current.transition_to(status)?;
        if current == status {
            let updated_at = row.try_get("updated_at")?;
            transaction.commit().await?;
            return Ok(updated_at);
        }
        let status_str = status.to_string();
        let now = Utc::now();
        let updated_at = sqlx::query_scalar(
            r#"
            UPDATE payment_intents
            SET status = $1, updated_at = $2
            WHERE id = $3
            RETURNING updated_at
            "#,
        )
        .bind(status_str)
        .bind(now)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(updated_at)
    }
}

/// In-memory repository for development fallback / unit testing without database.
#[derive(Debug, Clone, Default)]
pub struct InMemoryPaymentIntentRepository {
    storage: Arc<RwLock<HashMap<Uuid, PaymentIntent>>>,
}

impl InMemoryPaymentIntentRepository {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl PaymentIntentRepository for InMemoryPaymentIntentRepository {
    async fn save(&self, intent: &PaymentIntent) -> Result<()> {
        let mut map = self.storage.write().await;
        map.insert(intent.id, intent.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentIntent>> {
        let map = self.storage.read().await;
        Ok(map.get(&id).cloned())
    }

    async fn find_by_reference(&self, reference: &str) -> Result<Option<PaymentIntent>> {
        let map = self.storage.read().await;
        for intent in map.values() {
            if intent.claim_reference.as_deref() == Some(reference)
                || intent.id.to_string() == reference
                || intent.id.simple().to_string() == reference
            {
                return Ok(Some(intent.clone()));
            }
        }
        Ok(None)
    }

    async fn list_all(&self) -> Result<Vec<PaymentIntent>> {
        let map = self.storage.read().await;
        let mut list: Vec<PaymentIntent> = map.values().cloned().collect();
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(list)
    }

    async fn find_by_user(&self, user_identifier: &str) -> Result<Vec<PaymentIntent>> {
        let clean = user_identifier.strip_prefix('@').unwrap_or(user_identifier);
        let map = self.storage.read().await;
        let mut list: Vec<PaymentIntent> = map
            .values()
            .filter(|i| {
                i.sender_id.as_deref() == Some(user_identifier)
                    || i.sender_id.as_deref() == Some(clean)
                    || i.recipient_identifier == user_identifier
                    || i.recipient_identifier == clean
                    || i.recipient_identifier.strip_prefix('@') == Some(clean)
            })
            .cloned()
            .collect();
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(list)
    }

    async fn update_status(&self, id: Uuid, status: PaymentStatus) -> Result<DateTime<Utc>> {
        let mut map = self.storage.write().await;
        let intent = map
            .get_mut(&id)
            .ok_or_else(|| ApiError::NotFound(format!("Payment intent {id} not found")))?;
        intent.status.transition_to(status)?;
        if intent.status != status {
            intent.status = status;
            intent.updated_at = Utc::now();
        }
        Ok(intent.updated_at)
    }
}
