use hanbova_core::CoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtectedPaymentError {
    #[error("Core domain error: {0}")]
    Core(#[from] CoreError),

    #[error("Payment not found: {0}")]
    NotFound(String),

    #[error("Invalid claim proof: {0}")]
    InvalidClaimProof(String),

    #[error("Invalid public key format: {0}")]
    InvalidPublicKey(String),

    #[error("Mint does not support NUT-11 P2PK spending conditions: {0}")]
    Nut11NotSupported(String),

    #[error("Mint offline or unreachable: {0}")]
    MintUnreachable(String),

    #[error("Insufficient wallet funds: requested {requested_sats} sats, available {available_sats} sats")]
    InsufficientFunds {
        requested_sats: u64,
        available_sats: u64,
    },

    #[error("Payment has expired at {0}")]
    PaymentExpired(chrono::DateTime<chrono::Utc>),

    #[error("Payment is not yet eligible for refund (locktime: {0})")]
    PaymentNotExpired(chrono::DateTime<chrono::Utc>),

    #[error("Proof already spent: {0}")]
    TokenAlreadySpent(String),

    #[error("Locking condition error: {0}")]
    LockingCondition(String),

    #[error("CDK error: {0}")]
    Cdk(String),

    #[error("Provider internal error: {0}")]
    ProviderError(String),
}

pub type Result<T> = std::result::Result<T, ProtectedPaymentError>;
