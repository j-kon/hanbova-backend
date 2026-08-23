use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    amount::SatoshiAmount,
    error::{CoreError, Result},
    payment_status::PaymentStatus,
    payment_type::PaymentType,
};

/// Core domain model representing a Payment Intent in Hanbova.
///
/// A Payment Intent encapsulates the intent to transfer value, either
/// instantaneously or under protected/conditional terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentIntent {
    pub id: Uuid,
    pub payment_type: PaymentType,
    pub status: PaymentStatus,
    pub amount_sats: SatoshiAmount,
    pub sender_id: Option<String>,
    pub recipient_identifier: String,
    pub description: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub claim_reference: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PaymentIntent {
    /// Constructs a new PaymentIntent with validated inputs.
    pub fn new(
        payment_type: PaymentType,
        amount_sats: SatoshiAmount,
        recipient_identifier: impl Into<String>,
        sender_id: Option<String>,
        description: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self> {
        let recipient_str = recipient_identifier.into().trim().to_string();
        if recipient_str.is_empty() {
            return Err(CoreError::ValidationError(
                "Recipient identifier cannot be empty".to_string(),
            ));
        }

        if amount_sats.is_zero() {
            return Err(CoreError::ValidationError(
                "Amount must be greater than zero satoshis".to_string(),
            ));
        }

        let now = Utc::now();
        if let Some(exp) = expires_at {
            if exp <= now {
                return Err(CoreError::ValidationError(
                    "Expiration time must be in the future".to_string(),
                ));
            }
        }

        Ok(Self {
            id: Uuid::new_v4(),
            payment_type,
            status: PaymentStatus::Created,
            amount_sats,
            sender_id,
            recipient_identifier: recipient_str,
            description,
            expires_at,
            claim_reference: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Progresses the status of the payment intent according to the state machine.
    pub fn update_status(&mut self, next_status: PaymentStatus) -> Result<()> {
        let new_status = self.status.transition_to(next_status)?;
        self.status = new_status;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Checks whether the payment has expired based on current timestamp.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        match self.expires_at {
            Some(exp) => now >= exp,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_create_valid_payment_intent() {
        let expires = Utc::now() + Duration::hours(24);
        let intent = PaymentIntent::new(
            PaymentType::Protected,
            SatoshiAmount::from_sats(25_000),
            "alice@hanbova.me",
            Some("bob-sender-uuid".to_string()),
            Some("Coffee and lunch".to_string()),
            Some(expires),
        )
        .unwrap();

        assert_eq!(intent.status, PaymentStatus::Created);
        assert_eq!(intent.payment_type, PaymentType::Protected);
        assert_eq!(intent.amount_sats.as_u64(), 25_000);
        assert_eq!(intent.recipient_identifier, "alice@hanbova.me");
    }

    #[test]
    fn test_validation_empty_recipient() {
        let res = PaymentIntent::new(
            PaymentType::Instant,
            SatoshiAmount::from_sats(100),
            "  ",
            None,
            None,
            None,
        );
        assert!(matches!(res, Err(CoreError::ValidationError(_))));
    }

    #[test]
    fn test_validation_zero_amount() {
        let res = PaymentIntent::new(
            PaymentType::Instant,
            SatoshiAmount::ZERO,
            "recipient",
            None,
            None,
            None,
        );
        assert!(matches!(res, Err(CoreError::ValidationError(_))));
    }

    #[test]
    fn test_status_transition_on_intent() {
        let mut intent = PaymentIntent::new(
            PaymentType::Protected,
            SatoshiAmount::from_sats(500),
            "carol",
            None,
            None,
            None,
        )
        .unwrap();

        intent.update_status(PaymentStatus::Pending).unwrap();
        assert_eq!(intent.status, PaymentStatus::Pending);

        intent.update_status(PaymentStatus::Claimable).unwrap();
        assert_eq!(intent.status, PaymentStatus::Claimable);

        intent.update_status(PaymentStatus::Claimed).unwrap();
        assert_eq!(intent.status, PaymentStatus::Claimed);

        // Terminal state cannot transition to Refunded
        assert!(intent.update_status(PaymentStatus::Refunded).is_err());
    }
}
