use chrono::{DateTime, Utc};
use hanbova_core::{PaymentStatus, SatoshiAmount};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Locking conditions for a Cashu NUT-11 P2PK protected payment.
///
/// Implements Cashu NUT-10 Spending Conditions and NUT-11 Pay-To-Public-Key:
/// - `recipient_pubkey`: 33-byte compressed secp256k1 hex public key authorized to claim.
/// - `locktime`: Unix timestamp / DateTime after which the sender refund key becomes valid.
/// - `refund_pubkey`: 33-byte compressed secp256k1 hex public key authorized to refund after locktime.
/// - `sig_flag`: Signature commitment flag (`SIG_INPUTS` default, protecting inputs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockingConditions {
    pub recipient_pubkey: String,
    pub locktime: DateTime<Utc>,
    pub refund_pubkey: Option<String>,
    pub sig_flag: Option<String>,
}

/// Request to create and lock a protected payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProtectedPaymentRequest {
    pub payment_id: Option<Uuid>,
    pub amount_sats: SatoshiAmount,
    pub recipient_identifier: String,
    pub sender_id: Option<String>,
    pub description: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub locking_conditions: Option<LockingConditions>,
}

/// Receipt returned after creating or settling a protected payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedPaymentReceipt {
    pub payment_id: Uuid,
    pub status: PaymentStatus,
    pub amount_sats: SatoshiAmount,
    pub recipient_identifier: String,
    pub expires_at: DateTime<Utc>,
    pub claim_reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cashu_token: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Request submitted by recipient to claim locked funds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimPaymentRequest {
    pub payment_id: Uuid,
    pub claim_proof: String,
    pub claimer_identifier: String,
    pub cashu_token: Option<String>,
}

/// Request submitted by sender to refund expired funds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundPaymentRequest {
    pub payment_id: Uuid,
    pub sender_id: String,
    pub refund_proof: Option<String>,
    pub cashu_token: Option<String>,
}

/// Breakdown of wallet balance across spendable and protected pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WalletBalance {
    pub spendable_sats: u64,
    pub protected_outgoing_sats: u64,
    pub protected_incoming_sats: u64,
}
