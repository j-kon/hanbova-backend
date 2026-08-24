//! # Hanbova Lightning Integration Crate
//!
//! Provides traits, requests, and adapters for Bitcoin Lightning Network interactions.

pub mod error;
pub mod mock;
pub mod models;
pub mod nut04_nut05;
pub mod traits;

pub use error::{LightningError, Result};
pub use mock::MockLightningProvider;
pub use models::{
    CreateInvoiceRequest, Invoice, LightningBalance, LightningPaymentStatus, PayInvoiceRequest,
    PaymentDetails,
};
pub use nut04_nut05::{CashuLightningBridge, MeltQuoteResponse, MintQuoteResponse};
pub use traits::LightningProvider;
