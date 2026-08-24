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
    async fn list_all(&self) -> Result<Vec<PaymentIntent>>;
    async fn find_by_user(&self, user_identifier: &str) -> Result<Vec<PaymentIntent>>;
    async fn update_status(&self, id: Uuid, status: PaymentStatus) -> Result<()>;
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

    async fn update_status(&self, id: Uuid, status: PaymentStatus) -> Result<()> {
        let status_str = status.to_string();
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE payment_intents
            SET status = $1, updated_at = $2
            WHERE id = $3
            "#,
        )
        .bind(status_str)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
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

    async fn update_status(&self, id: Uuid, status: PaymentStatus) -> Result<()> {
        let mut map = self.storage.write().await;
        if let Some(intent) = map.get_mut(&id) {
            intent.status = status;
            intent.updated_at = Utc::now();
        }
        Ok(())
    }
}
