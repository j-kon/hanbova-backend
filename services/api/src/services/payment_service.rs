use chrono::{Duration, Utc};
use hanbova_core::{PaymentIntent, PaymentStatus, PaymentType, SatoshiAmount};
use hanbova_protected_payments::{
    CreateProtectedPaymentRequest, LockingConditions, ProtectedPaymentProvider,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{ApiError, Result},
    models::{CreatePaymentIntentRequest, PaymentIntentResponse},
    repositories::PaymentIntentRepository,
};

#[derive(Clone)]
pub struct PaymentService {
    repo: Arc<dyn PaymentIntentRepository>,
    protected_provider: Arc<dyn ProtectedPaymentProvider>,
}

impl PaymentService {
    pub fn new(
        repo: Arc<dyn PaymentIntentRepository>,
        protected_provider: Arc<dyn ProtectedPaymentProvider>,
    ) -> Self {
        Self {
            repo,
            protected_provider,
        }
    }

    pub async fn create_payment_intent(
        &self,
        req: CreatePaymentIntentRequest,
    ) -> Result<PaymentIntentResponse> {
        let amount = SatoshiAmount::new(req.amount_sats)?;
        let now = Utc::now();

        let expires_at = req
            .expires_in_seconds
            .map(|secs| now + Duration::seconds(secs as i64));

        let mut intent = PaymentIntent::new(
            req.payment_type,
            amount,
            req.recipient_identifier,
            req.sender_id,
            req.description,
            expires_at,
        )?;

        let mut token_out = None;

        match intent.payment_type {
            PaymentType::Protected => {
                let expiry = expires_at.unwrap_or_else(|| now + Duration::hours(24));
                intent.expires_at = Some(expiry);

                let locking_conditions = req.recipient_pubkey.map(|rec_pub| LockingConditions {
                    recipient_pubkey: rec_pub,
                    locktime: expiry,
                    refund_pubkey: req.sender_refund_pubkey,
                    sig_flag: Some("SIG_INPUTS".to_string()),
                });

                let protected_req = CreateProtectedPaymentRequest {
                    payment_id: Some(intent.id),
                    amount_sats: intent.amount_sats,
                    recipient_identifier: intent.recipient_identifier.clone(),
                    sender_id: intent.sender_id.clone(),
                    description: intent.description.clone(),
                    expires_at: expiry,
                    locking_conditions,
                };

                let receipt = self
                    .protected_provider
                    .create_protected_payment(protected_req)
                    .await?;

                intent.status = receipt.status;
                intent.claim_reference = Some(receipt.claim_reference);
                token_out = receipt.cashu_token;
            }
            PaymentType::Instant => {
                intent.status = PaymentStatus::Pending;
            }
        }

        self.repo.save(&intent).await?;

        let mut response: PaymentIntentResponse = intent.into();
        response.cashu_token = token_out;

        Ok(response)
    }

    pub async fn get_payment_intent(&self, id: Uuid) -> Result<PaymentIntentResponse> {
        let intent = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Payment intent {id} not found")))?;

        let live_status = self
            .protected_provider
            .get_payment_status(id)
            .await
            .unwrap_or(intent.status);

        let mut response: PaymentIntentResponse = intent.into();
        response.status = live_status;

        Ok(response)
    }

    pub async fn list_payment_intents(&self) -> Result<Vec<PaymentIntentResponse>> {
        let intents = self.repo.list_all().await?;
        Ok(intents.into_iter().map(Into::into).collect())
    }

    /// Updates the coordination status of a payment intent after client-side Cashu mint settlement.
    pub async fn update_payment_status(
        &self,
        payment_id: Uuid,
        new_status: PaymentStatus,
    ) -> Result<PaymentIntentResponse> {
        let mut intent = self
            .repo
            .find_by_id(payment_id)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Payment intent {payment_id} not found")))?;

        // Validate state machine transition
        intent.status = intent.status.transition_to(new_status)?;
        intent.updated_at = Utc::now();

        self.repo.save(&intent).await?;

        Ok(intent.into())
    }
}
