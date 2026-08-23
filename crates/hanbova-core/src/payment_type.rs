use serde::{Deserialize, Serialize};
use std::fmt;

/// The primary payment modality in Hanbova.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentType {
    /// Instant Bitcoin/Lightning payment that settles immediately upon invoice payment.
    Instant,
    /// Protected payment held conditionally with claim, expiration, and refund capabilities.
    Protected,
}

impl PaymentType {
    pub fn is_instant(&self) -> bool {
        matches!(self, Self::Instant)
    }

    pub fn is_protected(&self) -> bool {
        matches!(self, Self::Protected)
    }
}

impl fmt::Display for PaymentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instant => write!(f, "instant"),
            Self::Protected => write!(f, "protected"),
        }
    }
}

impl std::str::FromStr for PaymentType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "instant" => Ok(Self::Instant),
            "protected" => Ok(Self::Protected),
            other => Err(format!("Unknown payment type: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_type_serialization() {
        let json = serde_json::to_string(&PaymentType::Protected).unwrap();
        assert_eq!(json, "\"protected\"");
        let deserialized: PaymentType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PaymentType::Protected);
    }
}
