use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: crate::payment_status::PaymentStatus,
        to: crate::payment_status::PaymentStatus,
    },

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Payment intent expired at {0}")]
    Expired(chrono::DateTime<chrono::Utc>),

    #[error("Payment cannot be claimed in current status: {0:?}")]
    CannotClaim(crate::payment_status::PaymentStatus),

    #[error("Payment cannot be refunded in current status: {0:?}")]
    CannotRefund(crate::payment_status::PaymentStatus),
}

pub type Result<T> = std::result::Result<T, CoreError>;
