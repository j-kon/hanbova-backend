use chrono::{DateTime, Utc};
use hanbova_core::SatoshiAmount;
use serde::{Deserialize, Serialize};

/// Request to create a Lightning invoice (BOLT11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvoiceRequest {
    pub amount_sats: SatoshiAmount,
    pub description: String,
    pub expiry_seconds: Option<u32>,
}

/// Represents an issued Lightning invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub payment_hash: String,
    pub bolt11: String,
    pub amount_sats: SatoshiAmount,
    pub description: String,
    pub expires_at: DateTime<Utc>,
    pub is_paid: bool,
    pub created_at: DateTime<Utc>,
}

/// Request to pay a Lightning invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayInvoiceRequest {
    pub bolt11: String,
    /// Optional max fee tolerance in satoshis
    pub max_fee_sats: Option<u64>,
}

/// Payment settlement details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentDetails {
    pub payment_hash: String,
    pub preimage: Option<String>,
    pub amount_sats: SatoshiAmount,
    pub fee_sats: SatoshiAmount,
    pub status: LightningPaymentStatus,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Settlement status of a Lightning payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightningPaymentStatus {
    Pending,
    Succeeded,
    Failed,
}

/// Node / Wallet balance representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningBalance {
    pub total_sats: SatoshiAmount,
    pub spendable_sats: SatoshiAmount,
    pub receiving_capacity_sats: SatoshiAmount,
}
