use chrono::{DateTime, Utc};
use hanbova_core::{PaymentIntent, PaymentStatus, PaymentType};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request payload to create a new Payment Intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentIntentRequest {
    pub payment_type: PaymentType,
    pub amount_sats: u64,
    pub recipient_identifier: String,
    pub sender_id: Option<String>,
    pub description: Option<String>,
    pub expires_in_seconds: Option<u64>,
    pub recipient_pubkey: Option<String>,
    pub sender_refund_pubkey: Option<String>,
    pub cashu_mint_url: Option<String>,
}

/// Request payload to claim a protected payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimIntentRequest {
    pub claim_proof: String,
    pub claimer_identifier: String,
    pub cashu_token: Option<String>,
}

/// Request payload to refund an expired protected payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundIntentRequest {
    pub sender_id: String,
    pub refund_proof: Option<String>,
    pub cashu_token: Option<String>,
}

/// Response payload representing a Payment Intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentIntentResponse {
    pub id: Uuid,
    pub payment_type: PaymentType,
    pub status: PaymentStatus,
    pub amount_sats: u64,
    pub sender_id: Option<String>,
    pub recipient_identifier: String,
    pub description: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub claim_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cashu_token: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<PaymentIntent> for PaymentIntentResponse {
    fn from(intent: PaymentIntent) -> Self {
        Self {
            id: intent.id,
            payment_type: intent.payment_type,
            status: intent.status,
            amount_sats: intent.amount_sats.as_u64(),
            sender_id: intent.sender_id,
            recipient_identifier: intent.recipient_identifier,
            description: intent.description,
            expires_at: intent.expires_at,
            claim_reference: intent.claim_reference,
            cashu_token: None,
            created_at: intent.created_at,
            updated_at: intent.updated_at,
        }
    }
}
