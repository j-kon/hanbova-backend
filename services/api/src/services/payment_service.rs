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
            .map(|secs| {
                i64::try_from(secs)
                    .ok()
                    .filter(|seconds| *seconds > 0)
                    .and_then(Duration::try_seconds)
                    .and_then(|duration| now.checked_add_signed(duration))
                    .ok_or_else(|| {
                        ApiError::BadRequest(
                            "Expiry must be a positive, representable number of seconds".into(),
                        )
                    })
            })
            .transpose()?;

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

    /// Bind an encrypted envelope only to a protected intent belonging to the
    /// authenticated sender and the resolved recipient. This does not inspect
    /// the encrypted payload or attest that mint settlement occurred.
    pub async fn validate_protected_message_link(
        &self,
        payment_id: Uuid,
        sender_id: Uuid,
        sender_username: &str,
        recipient_id: Uuid,
        recipient_username: &str,
    ) -> Result<()> {
        let intent =
            self.repo.find_by_id(payment_id).await?.ok_or_else(|| {
                ApiError::NotFound(format!("Payment intent {payment_id} not found"))
            })?;
        let is_sender = intent.sender_id.as_deref().is_some_and(|sender| {
            matches_actor(&sender_id.to_string(), Some(sender_username), sender)
        });
        let is_recipient = matches_actor(
            &recipient_id.to_string(),
            Some(recipient_username),
            &intent.recipient_identifier,
        );
        if !is_sender || !is_recipient {
            return Err(ApiError::Forbidden(
                "Payment intent does not belong to this sender and recipient".into(),
            ));
        }
        if intent.payment_type != PaymentType::Protected {
            return Err(ApiError::BadRequest(
                "Protected messages can only link to protected payment intents".into(),
            ));
        }
        Ok(())
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

        // Validate against the latest stored state under the repository lock.
        // Saving this earlier snapshot could overwrite a concurrent terminal report.
        intent.updated_at = self.repo.update_status(payment_id, new_status).await?;
        intent.status = new_status;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::InMemoryPaymentIntentRepository;
    use hanbova_protected_payments::MockProtectedPaymentProvider;

    // Force both service calls to read the same pre-settlement snapshot. Only
    // the read scheduling is controlled; persistence remains the real repository.
    struct ConcurrentReadRepository {
        inner: InMemoryPaymentIntentRepository,
        barrier: tokio::sync::Barrier,
    }

    #[async_trait::async_trait]
    impl PaymentIntentRepository for ConcurrentReadRepository {
        async fn save(&self, intent: &PaymentIntent) -> Result<()> {
            self.inner.save(intent).await
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentIntent>> {
            let snapshot = self.inner.find_by_id(id).await?;
            self.barrier.wait().await;
            Ok(snapshot)
        }
        async fn find_by_reference(&self, reference: &str) -> Result<Option<PaymentIntent>> {
            self.inner.find_by_reference(reference).await
        }
        async fn list_all(&self) -> Result<Vec<PaymentIntent>> {
            self.inner.list_all().await
        }
        async fn find_by_user(&self, user: &str) -> Result<Vec<PaymentIntent>> {
            self.inner.find_by_user(user).await
        }
        async fn update_status(
            &self,
            id: Uuid,
            status: PaymentStatus,
        ) -> Result<chrono::DateTime<Utc>> {
            self.inner.update_status(id, status).await
        }
    }

    #[tokio::test]
    async fn payment_status_service_does_not_save_a_stale_terminal_snapshot() {
        let inner = InMemoryPaymentIntentRepository::new();
        let mut intent = PaymentIntent::new(
            PaymentType::Protected,
            SatoshiAmount::new(100).unwrap(),
            "bob",
            Some("alice".into()),
            None,
            None,
        )
        .unwrap();
        intent.status = PaymentStatus::RefundAvailable;
        inner.save(&intent).await.unwrap();
        let service = PaymentService::new(
            Arc::new(ConcurrentReadRepository {
                inner: inner.clone(),
                barrier: tokio::sync::Barrier::new(2),
            }),
            Arc::new(MockProtectedPaymentProvider::new()),
        );
        let (claim, refund) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(
                service.update_payment_status(intent.id, PaymentStatus::Claimed, "bob", None),
                service.update_payment_status(intent.id, PaymentStatus::Refunded, "alice", None),
            )
        })
        .await
        .unwrap();
        assert_ne!(
            claim.is_ok(),
            refund.is_ok(),
            "stale service snapshots must not overwrite each other"
        );
        let winner = claim.or(refund).unwrap();
        let stored = inner.find_by_id(intent.id).await.unwrap().unwrap();
        assert_eq!(stored.status, winner.status);
        assert_eq!(stored.updated_at, winner.updated_at);
    }

    #[tokio::test]
    async fn rejects_expiry_overflow_without_panicking() {
        let service = PaymentService::new(
            Arc::new(InMemoryPaymentIntentRepository::new()),
            Arc::new(MockProtectedPaymentProvider::new()),
        );
        for seconds in [0, u64::MAX, i64::MAX as u64, 10_000_000_000_000] {
            let req: CreatePaymentIntentRequest = serde_json::from_value(serde_json::json!({
                "payment_type": "protected", "amount_sats": 100,
                "recipient_identifier": "bob", "expires_in_seconds": seconds
            }))
            .unwrap();
            assert!(service.create_payment_intent(req).await.is_err());
        }
    }

    #[tokio::test]
    async fn preserves_valid_expiry_and_default_claim_window() {
        let service = PaymentService::new(
            Arc::new(InMemoryPaymentIntentRepository::new()),
            Arc::new(MockProtectedPaymentProvider::new()),
        );
        for seconds in [None, Some(3600)] {
            let req: CreatePaymentIntentRequest = serde_json::from_value(serde_json::json!({
                "payment_type": "protected", "amount_sats": 100,
                "recipient_identifier": "bob", "expires_in_seconds": seconds
            }))
            .unwrap();
            let before = Utc::now();
            let response = service.create_payment_intent(req).await.unwrap();
            let expected = seconds.unwrap_or(86400);
            let duration = response.expires_at.unwrap() - before;
            assert!((duration.num_seconds() - expected).abs() <= 1);
        }
    }
}
