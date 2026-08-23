use async_trait::async_trait;
use hanbova_core::PaymentStatus;
use uuid::Uuid;

use crate::{
    error::Result,
    models::{
        ClaimPaymentRequest, CreateProtectedPaymentRequest, ProtectedPaymentReceipt,
        RefundPaymentRequest, WalletBalance,
    },
};

/// Interface defining protected payment operations.
///
/// Decouples the application and domain models from underlying Cashu / CDK cryptography.
#[async_trait]
pub trait ProtectedPaymentProvider: Send + Sync {
    /// Validates whether the active mint supports NUT-10 and NUT-11 spending conditions.
    async fn check_mint_support(&self) -> Result<bool>;

    /// Locks funds into a protected payment with P2PK and timelock spending conditions.
    async fn create_protected_payment(
        &self,
        request: CreateProtectedPaymentRequest,
    ) -> Result<ProtectedPaymentReceipt>;

    /// Claims locked funds by presenting the recipient's signature / proof.
    async fn claim_payment(&self, request: ClaimPaymentRequest) -> Result<ProtectedPaymentReceipt>;

    /// Refunds expired funds back to the sender after the locktime has elapsed.
    async fn refund_payment(
        &self,
        request: RefundPaymentRequest,
    ) -> Result<ProtectedPaymentReceipt>;

    /// Queries the current lifecycle status of a protected payment.
    async fn get_payment_status(&self, payment_id: Uuid) -> Result<PaymentStatus>;

    /// Returns the current balance broken down by spendable and protected escrow pools.
    async fn get_wallet_balance(&self) -> Result<WalletBalance>;
}
