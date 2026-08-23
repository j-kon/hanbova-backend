use async_trait::async_trait;
use chrono::Utc;
use hanbova_core::{PaymentIntent, PaymentStatus, PaymentType};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    error::{ProtectedPaymentError, Result},
    models::{
        ClaimPaymentRequest, CreateProtectedPaymentRequest, ProtectedPaymentReceipt,
        RefundPaymentRequest, WalletBalance,
    },
    traits::ProtectedPaymentProvider,
};

/// In-memory development provider implementing `ProtectedPaymentProvider`.
///
/// Simulates the exact state machine rules of Hanbova Protected Payments.
#[derive(Debug, Clone, Default)]
pub struct MockProtectedPaymentProvider {
    intents: Arc<RwLock<HashMap<Uuid, PaymentIntent>>>,
}

impl MockProtectedPaymentProvider {
    pub fn new() -> Self {
        Self {
            intents: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ProtectedPaymentProvider for MockProtectedPaymentProvider {
    async fn check_mint_support(&self) -> Result<bool> {
        Ok(true)
    }

    async fn create_protected_payment(
        &self,
        request: CreateProtectedPaymentRequest,
    ) -> Result<ProtectedPaymentReceipt> {
        let mut intent = PaymentIntent::new(
            PaymentType::Protected,
            request.amount_sats,
            request.recipient_identifier.clone(),
            request.sender_id,
            request.description,
            Some(request.expires_at),
        )?;

        if let Some(id) = request.payment_id {
            intent.id = id;
        }

        intent.update_status(PaymentStatus::Protected)?;
        intent.update_status(PaymentStatus::Claimable)?;

        let receipt = ProtectedPaymentReceipt {
            payment_id: intent.id,
            status: intent.status,
            amount_sats: intent.amount_sats,
            recipient_identifier: intent.recipient_identifier.clone(),
            expires_at: request.expires_at,
            claim_reference: format!("hnbv_claim_{}", intent.id.simple()),
            cashu_token: Some(format!("cashuA_mock_{}", intent.id.simple())),
            created_at: intent.created_at,
        };

        let mut lock = self.intents.write().await;
        lock.insert(intent.id, intent);

        Ok(receipt)
    }

    async fn claim_payment(&self, request: ClaimPaymentRequest) -> Result<ProtectedPaymentReceipt> {
        let mut lock = self.intents.write().await;
        let intent = lock
            .get_mut(&request.payment_id)
            .ok_or_else(|| ProtectedPaymentError::NotFound(request.payment_id.to_string()))?;

        let now = Utc::now();
        if intent.is_expired(now) {
            intent.update_status(PaymentStatus::RefundAvailable)?;
            return Err(ProtectedPaymentError::PaymentExpired(
                intent.expires_at.unwrap_or(now),
            ));
        }

        if intent.status != PaymentStatus::Claimable && intent.status != PaymentStatus::Protected {
            return Err(ProtectedPaymentError::Core(
                hanbova_core::CoreError::CannotClaim(intent.status),
            ));
        }

        if request.claim_proof.trim().is_empty() {
            return Err(ProtectedPaymentError::InvalidClaimProof(
                "Claim proof is empty".to_string(),
            ));
        }

        intent.update_status(PaymentStatus::Claimed)?;

        Ok(ProtectedPaymentReceipt {
            payment_id: intent.id,
            status: intent.status,
            amount_sats: intent.amount_sats,
            recipient_identifier: intent.recipient_identifier.clone(),
            expires_at: intent.expires_at.unwrap_or(now),
            claim_reference: format!("claimed_{}", intent.id.simple()),
            cashu_token: None,
            created_at: intent.created_at,
        })
    }

    async fn refund_payment(
        &self,
        request: RefundPaymentRequest,
    ) -> Result<ProtectedPaymentReceipt> {
        let mut lock = self.intents.write().await;
        let intent = lock
            .get_mut(&request.payment_id)
            .ok_or_else(|| ProtectedPaymentError::NotFound(request.payment_id.to_string()))?;

        let now = Utc::now();
        if !intent.is_expired(now) {
            return Err(ProtectedPaymentError::PaymentNotExpired(
                intent.expires_at.unwrap_or(now),
            ));
        }

        if intent.status != PaymentStatus::RefundAvailable
            && intent.status != PaymentStatus::Expired
        {
            intent.update_status(PaymentStatus::RefundAvailable)?;
        }

        intent.update_status(PaymentStatus::Refunded)?;

        Ok(ProtectedPaymentReceipt {
            payment_id: intent.id,
            status: intent.status,
            amount_sats: intent.amount_sats,
            recipient_identifier: intent.recipient_identifier.clone(),
            expires_at: intent.expires_at.unwrap_or(now),
            claim_reference: format!("refunded_{}", intent.id.simple()),
            cashu_token: None,
            created_at: intent.created_at,
        })
    }

    async fn get_payment_status(&self, payment_id: Uuid) -> Result<PaymentStatus> {
        let lock = self.intents.read().await;
        let intent = lock
            .get(&payment_id)
            .ok_or_else(|| ProtectedPaymentError::NotFound(payment_id.to_string()))?;
        Ok(intent.status)
    }

    async fn get_wallet_balance(&self) -> Result<WalletBalance> {
        Ok(WalletBalance {
            spendable_sats: 100_000,
            protected_outgoing_sats: 0,
            protected_incoming_sats: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use hanbova_core::SatoshiAmount;

    #[tokio::test]
    async fn test_mock_create_and_claim_flow() {
        let provider = MockProtectedPaymentProvider::new();
        let expires = Utc::now() + Duration::hours(12);

        let create_req = CreateProtectedPaymentRequest {
            payment_id: None,
            amount_sats: SatoshiAmount::from_sats(15_000),
            recipient_identifier: "recipient@hanbova.me".to_string(),
            sender_id: Some("sender_123".to_string()),
            description: Some("Work contract milestone".to_string()),
            expires_at: expires,
            locking_conditions: None,
        };

        let receipt = provider.create_protected_payment(create_req).await.unwrap();
        assert_eq!(receipt.status, PaymentStatus::Claimable);

        let claim_req = ClaimPaymentRequest {
            payment_id: receipt.payment_id,
            claim_proof: "valid_secret_or_sig".to_string(),
            claimer_identifier: "recipient@hanbova.me".to_string(),
            cashu_token: None,
        };

        let claimed_receipt = provider.claim_payment(claim_req).await.unwrap();
        assert_eq!(claimed_receipt.status, PaymentStatus::Claimed);
    }

    #[tokio::test]
    async fn test_mock_refund_before_locktime_fails() {
        let provider = MockProtectedPaymentProvider::new();
        let expires = Utc::now() + Duration::hours(24);

        let create_req = CreateProtectedPaymentRequest {
            payment_id: None,
            amount_sats: SatoshiAmount::from_sats(50_000),
            recipient_identifier: "bob@hanbova.me".to_string(),
            sender_id: Some("alice".to_string()),
            description: None,
            expires_at: expires,
            locking_conditions: None,
        };

        let receipt = provider.create_protected_payment(create_req).await.unwrap();

        let refund_req = RefundPaymentRequest {
            payment_id: receipt.payment_id,
            sender_id: "alice".to_string(),
            refund_proof: Some("refund_key".to_string()),
            cashu_token: None,
        };

        let res = provider.refund_payment(refund_req).await;
        assert!(res.is_err());
    }
}
