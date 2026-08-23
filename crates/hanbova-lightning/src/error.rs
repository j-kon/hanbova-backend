use hanbova_core::CoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LightningError {
    #[error("Core domain error: {0}")]
    Core(#[from] CoreError),

    #[error("Invalid invoice: {0}")]
    InvalidInvoice(String),

    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    #[error("Invoice not found: {0}")]
    InvoiceNotFound(String),

    #[error("Insufficient balance: requested {requested}, available {available}")]
    InsufficientBalance { requested: u64, available: u64 },

    #[error("Node / Provider communication error: {0}")]
    NodeError(String),
}

pub type Result<T> = std::result::Result<T, LightningError>;
