use chrono::{Duration, Utc};
use hanbova_core::{PaymentIntent, PaymentStatus, PaymentType, SatoshiAmount};
use hanbova_protected_payments::ProtectedPaymentProvider;
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
    #[allow(dead_code)]
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

        match intent.payment_type {
            PaymentType::Protected => {
                let expiry = expires_at.unwrap_or_else(|| now + Duration::hours(24));
                intent.expires_at = Some(expiry);
                intent.status = PaymentStatus::Created;
                let claim_ref = format!("hnbv_claim_{}", intent.id.simple());
                intent.claim_reference = Some(claim_ref);
            }
            PaymentType::Instant => {
                intent.status = PaymentStatus::Pending;
            }
        }

        self.repo.save(&intent).await?;

        let response: PaymentIntentResponse = intent.into();
        Ok(response)
    }

    pub async fn get_payment_intent(
        &self,
        id: Uuid,
        user_id: Option<&str>,
        username: Option<&str>,
    ) -> Result<PaymentIntentResponse> {
        let intent = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Payment intent {id} not found")))?;

        if let Some(uid) = user_id {
            let is_sender = intent
                .sender_id
                .as_deref()
                .map(|s| matches_actor(uid, username, s))
                .unwrap_or(false);
            let is_recipient = matches_actor(uid, username, &intent.recipient_identifier);

            if !is_sender && !is_recipient {
                return Err(ApiError::Forbidden(
                    "You do not have permission to access this payment intent".into(),
                ));
            }
        }

        let response: PaymentIntentResponse = intent.into();
        Ok(response)
    }

    pub async fn get_payment_intent_by_reference(
        &self,
        reference: &str,
        user_id: Option<&str>,
        username: Option<&str>,
    ) -> Result<PaymentIntentResponse> {
        let intent = self
            .repo
            .find_by_reference(reference)
            .await?
            .ok_or_else(|| {
                ApiError::NotFound(format!(
                    "Payment intent with reference '{reference}' not found"
                ))
            })?;

        if let Some(uid) = user_id {
            let is_sender = intent
                .sender_id
                .as_deref()
                .map(|s| matches_actor(uid, username, s))
                .unwrap_or(false);
            let is_recipient = matches_actor(uid, username, &intent.recipient_identifier);

            if !is_sender && !is_recipient {
                return Err(ApiError::Forbidden(
                    "You do not have permission to access this payment intent".into(),
                ));
            }
        }

        let response: PaymentIntentResponse = intent.into();
        Ok(response)
    }

    pub async fn list_user_payment_intents(
        &self,
        user_id: &str,
        username: Option<&str>,
    ) -> Result<Vec<PaymentIntentResponse>> {
        let mut list = self.repo.find_by_user(user_id).await?;
        if let Some(uname) = username {
            let by_name = self.repo.find_by_user(uname).await?;
            for item in by_name {
                if !list.iter().any(|existing| existing.id == item.id) {
                    list.push(item);
                }
            }
        }
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(list.into_iter().map(Into::into).collect())
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
        user_id: &str,
        username: Option<&str>,
    ) -> Result<PaymentIntentResponse> {
        let mut intent =
            self.repo.find_by_id(payment_id).await?.ok_or_else(|| {
                ApiError::NotFound(format!("Payment intent {payment_id} not found"))
            })?;

        let is_sender = intent
            .sender_id
            .as_deref()
            .map(|s| matches_actor(user_id, username, s))
            .unwrap_or(false);
        let is_recipient = matches_actor(user_id, username, &intent.recipient_identifier);

        // Strict actor authorization rules
        match new_status {
            PaymentStatus::Claimed => {
                if !is_recipient {
                    return Err(ApiError::Forbidden(
                        "Only the intended recipient can report claimed status".into(),
                    ));
                }
            }
            PaymentStatus::Refunded => {
                if !is_sender {
                    return Err(ApiError::Forbidden(
                        "Only the sender can report refunded status".into(),
                    ));
                }
            }
            _ => {
                if !is_sender && !is_recipient {
                    return Err(ApiError::Forbidden(
                        "Unauthorized to update this payment intent".into(),
                    ));
                }
            }
        }

        // Validate state machine transition
        intent.status = intent.status.transition_to(new_status)?;
        intent.updated_at = Utc::now();

        self.repo.save(&intent).await?;

        Ok(intent.into())
    }
}

fn matches_actor(user_id: &str, username: Option<&str>, target: &str) -> bool {
    let clean_target = target.strip_prefix('@').unwrap_or(target);
    let clean_user_id = user_id.strip_prefix('@').unwrap_or(user_id);
    if target == user_id || clean_target == clean_user_id {
        return true;
    }
    if let Some(uname) = username {
        let clean_uname = uname.strip_prefix('@').unwrap_or(uname);
        if target.eq_ignore_ascii_case(uname)
            || clean_target.eq_ignore_ascii_case(clean_uname)
            || target == uname
            || clean_target == clean_uname
        {
            return true;
        }
    }
    false
}
