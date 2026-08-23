use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPaymentProfileResponse {
    pub username: String,
    pub handle: String,
    pub protected_payment_pubkey: String,
    pub transport_encryption_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePaymentKeysRequest {
    pub protected_payment_pubkey: String,
    pub transport_encryption_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProtectedMessageRequest {
    pub recipient_username: String,
    pub encrypted_payload: String,
    #[serde(default = "default_payload_version")]
    pub payload_version: i32,
    pub payment_intent_id: Option<Uuid>,
}

fn default_payload_version() -> i32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProtectedMessageRow {
    pub id: Uuid,
    pub payment_intent_id: Option<Uuid>,
    pub sender_user_id: Uuid,
    pub recipient_user_id: Uuid,
    pub sender_username: String,
    pub recipient_username: String,
    pub encrypted_payload: String,
    pub payload_version: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedMessageResponse {
    pub id: Uuid,
    pub payment_intent_id: Option<Uuid>,
    pub sender_username: String,
    pub recipient_username: String,
    pub encrypted_payload: String,
    pub payload_version: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
}

impl From<ProtectedMessageRow> for ProtectedMessageResponse {
    fn from(row: ProtectedMessageRow) -> Self {
        Self {
            id: row.id,
            payment_intent_id: row.payment_intent_id,
            sender_username: row.sender_username,
            recipient_username: row.recipient_username,
            encrypted_payload: row.encrypted_payload,
            payload_version: row.payload_version,
            status: row.status,
            created_at: row.created_at,
            acknowledged_at: row.acknowledged_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcknowledgeMessageRequest {
    pub status: Option<String>,
}
