//! # Hanbova Core Domain Types
//!
//! Shared, pure domain models and business rules for Hanbova.
//! This crate does NOT depend on HTTP frameworks or storage engines.

pub mod amount;
pub mod error;
pub mod payment_intent;
pub mod payment_status;
pub mod payment_type;

pub use amount::SatoshiAmount;
pub use error::{CoreError, Result};
pub use payment_intent::PaymentIntent;
pub use payment_status::PaymentStatus;
pub use payment_type::PaymentType;
