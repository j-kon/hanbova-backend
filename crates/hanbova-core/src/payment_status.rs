use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::{CoreError, Result};

/// Represents the lifecycle status of a Hanbova payment.
///
/// Under Cashu NUT-11 Protected Send semantics:
/// 1. Sender creates P2PK-locked ecash proofs with recipient key, refund key, and locktime.
/// 2. Status becomes `Protected` / `Claimable`.
/// 3. Before locktime, only the recipient can claim (`Claiming` -> `Claimed`).
/// 4. After locktime, the refund path activates (`RefundAvailable` / `Refunding` -> `Refunded`).
///    *Note: Cashu NUT-11 does NOT automatically invalidate the recipient spending path upon locktime.
///    Both recipient claim and sender refund paths remain possible until the first valid spend is confirmed by the mint.*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    /// Intent created in client or server, awaiting funding / signature.
    Created,
    /// Minting or funding proofs in flight.
    Funding,
    /// Pending initial network / mint confirmation.
    Pending,
    /// Proofs cryptographically locked in escrow under NUT-11 P2PK conditions.
    Protected,
    /// Recipient is authorized to claim the locked funds.
    Claimable,
    /// Recipient claim transaction submitted and in flight.
    Claiming,
    /// Recipient has successfully claimed and swapped proofs at the mint. (Terminal)
    Claimed,
    /// Locktime passed; refund spending path is now active for the sender.
    RefundAvailable,
    /// Sender refund transaction submitted and in flight.
    Refunding,
    /// Sender has recovered/refunded the funds at the mint. (Terminal)
    Refunded,
    /// Locktime reached (retained for protocol compatibility).
    Expired,
    /// Unrecoverable failure. (Terminal)
    Failed,
}

impl PaymentStatus {
    /// Validates whether a state transition from `self` to `target` is valid.
    pub fn can_transition_to(&self, target: Self) -> bool {
        match (self, target) {
            // Self-transitions (no-op)
            (s, t) if *s == t => true,

            // Terminal states cannot transition further
            (Self::Claimed, _) => false,
            (Self::Refunded, _) => false,
            (Self::Failed, _) => false,

            // Created
            (Self::Created, Self::Funding) => true,
            (Self::Created, Self::Pending) => true,
            (Self::Created, Self::Protected) => true,
            (Self::Created, Self::Claimable) => true,
            (Self::Created, Self::Failed) => true,

            // Funding
            (Self::Funding, Self::Protected) => true,
            (Self::Funding, Self::Claimable) => true,
            (Self::Funding, Self::Pending) => true,
            (Self::Funding, Self::Failed) => true,

            // Pending
            (Self::Pending, Self::Protected) => true,
            (Self::Pending, Self::Claimable) => true,
            (Self::Pending, Self::Claimed) => true, // Instant payments
            (Self::Pending, Self::Failed) => true,

            // Protected & Claimable
            (Self::Protected, Self::Claimable) => true,
            (Self::Protected, Self::Claiming) => true,
            (Self::Protected, Self::RefundAvailable) => true,
            (Self::Protected, Self::Expired) => true,
            (Self::Protected, Self::Claimed) => true,
            (Self::Protected, Self::Failed) => true,

            (Self::Claimable, Self::Claiming) => true,
            (Self::Claimable, Self::Claimed) => true,
            (Self::Claimable, Self::RefundAvailable) => true,
            (Self::Claimable, Self::Expired) => true,
            (Self::Claimable, Self::Failed) => true,

            // Claiming
            (Self::Claiming, Self::Claimed) => true,
            (Self::Claiming, Self::Claimable) => true, // Claim retry
            (Self::Claiming, Self::RefundAvailable) => true,
            (Self::Claiming, Self::Refunded) => true, // Sender beat recipient after locktime
            (Self::Claiming, Self::Failed) => true,

            // RefundAvailable / Expired (Locktime passed)
            (Self::RefundAvailable, Self::Refunding) => true,
            (Self::RefundAvailable, Self::Refunded) => true,
            (Self::RefundAvailable, Self::Claiming) => true, // Recipient still can claim if refund not settled!
            (Self::RefundAvailable, Self::Claimed) => true,
            (Self::RefundAvailable, Self::Failed) => true,

            (Self::Expired, Self::RefundAvailable) => true,
            (Self::Expired, Self::Refunding) => true,
            (Self::Expired, Self::Refunded) => true,
            (Self::Expired, Self::Claimed) => true,
            (Self::Expired, Self::Failed) => true,

            // Refunding
            (Self::Refunding, Self::Refunded) => true,
            (Self::Refunding, Self::Claimed) => true, // Recipient beat sender after locktime
            (Self::Refunding, Self::RefundAvailable) => true, // Refund retry
            (Self::Refunding, Self::Failed) => true,

            _ => false,
        }
    }

    /// Enforces the state transition or returns a `CoreError::InvalidStateTransition`.
    pub fn transition_to(&self, target: Self) -> Result<Self> {
        if self.can_transition_to(target) {
            Ok(target)
        } else {
            Err(CoreError::InvalidStateTransition {
                from: *self,
                to: target,
            })
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Claimed | Self::Refunded | Self::Failed)
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Created
                | Self::Funding
                | Self::Pending
                | Self::Protected
                | Self::Claimable
                | Self::Claiming
                | Self::RefundAvailable
                | Self::Refunding
                | Self::Expired
        )
    }
}

impl fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Funding => write!(f, "funding"),
            Self::Pending => write!(f, "pending"),
            Self::Protected => write!(f, "protected"),
            Self::Claimable => write!(f, "claimable"),
            Self::Claiming => write!(f, "claiming"),
            Self::Claimed => write!(f, "claimed"),
            Self::RefundAvailable => write!(f, "refund_available"),
            Self::Refunding => write!(f, "refunding"),
            Self::Refunded => write!(f, "refunded"),
            Self::Expired => write!(f, "expired"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for PaymentStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "created" => Ok(Self::Created),
            "funding" => Ok(Self::Funding),
            "pending" => Ok(Self::Pending),
            "protected" => Ok(Self::Protected),
            "claimable" => Ok(Self::Claimable),
            "claiming" => Ok(Self::Claiming),
            "claimed" => Ok(Self::Claimed),
            "refund_available" | "refundavailable" => Ok(Self::RefundAvailable),
            "refunding" => Ok(Self::Refunding),
            "refunded" => Ok(Self::Refunded),
            "expired" => Ok(Self::Expired),
            "failed" => Ok(Self::Failed),
            other => Err(format!("Unknown payment status: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_state_transitions() {
        assert!(PaymentStatus::Created.can_transition_to(PaymentStatus::Protected));
        assert!(PaymentStatus::Protected.can_transition_to(PaymentStatus::Claimable));
        assert!(PaymentStatus::Claimable.can_transition_to(PaymentStatus::Claiming));
        assert!(PaymentStatus::Claiming.can_transition_to(PaymentStatus::Claimed));
        assert!(PaymentStatus::Protected.can_transition_to(PaymentStatus::RefundAvailable));
        assert!(PaymentStatus::RefundAvailable.can_transition_to(PaymentStatus::Refunding));
        assert!(PaymentStatus::Refunding.can_transition_to(PaymentStatus::Refunded));
        // Bob claims after locktime
        assert!(PaymentStatus::RefundAvailable.can_transition_to(PaymentStatus::Claimed));
        assert!(PaymentStatus::Refunding.can_transition_to(PaymentStatus::Claimed));
    }

    #[test]
    fn test_invalid_state_transitions() {
        assert!(!PaymentStatus::Claimed.can_transition_to(PaymentStatus::Claimable));
        assert!(!PaymentStatus::Refunded.can_transition_to(PaymentStatus::Claimed));
        assert!(!PaymentStatus::Created.can_transition_to(PaymentStatus::Refunded));
    }
}
