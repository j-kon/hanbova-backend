use async_trait::async_trait;

use crate::{
    error::Result,
    models::{CreateInvoiceRequest, Invoice, LightningBalance, PayInvoiceRequest, PaymentDetails},
};

/// Abstraction trait decoupling Hanbova from specific Lightning implementations
/// (e.g. Breez SDK, LDK, Greenlight, CLN, LND).
#[async_trait]
pub trait LightningProvider: Send + Sync {
    /// Generates a Lightning invoice (BOLT11) to receive funds.
    async fn create_invoice(&self, request: CreateInvoiceRequest) -> Result<Invoice>;

    /// Pays a BOLT11 invoice.
    async fn pay_invoice(&self, request: PayInvoiceRequest) -> Result<PaymentDetails>;

    /// Retrieves status and details for a payment hash.
    async fn get_payment(&self, payment_hash: &str) -> Result<PaymentDetails>;

    /// Returns current wallet balance and capacity.
    async fn get_balance(&self) -> Result<LightningBalance>;
}
