use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::{CoreError, Result};

/// Represents an amount in Satoshis (1 BTC = 100,000,000 Satoshis).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct SatoshiAmount(u64);

impl SatoshiAmount {
    pub const ZERO: Self = Self(0);
    pub const MAX_SATS: u64 = 21_000_000 * 100_000_000;

    pub fn new(sats: u64) -> Result<Self> {
        if sats > Self::MAX_SATS {
            return Err(CoreError::InvalidAmount(format!(
                "Amount {sats} exceeds maximum 21M BTC supply in satoshis"
            )));
        }
        Ok(Self(sats))
    }

    pub const fn from_sats(sats: u64) -> Self {
        Self(sats)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn as_btc(&self) -> f64 {
        (self.0 as f64) / 100_000_000.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for SatoshiAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} sats", self.0)
    }
}

impl From<u64> for SatoshiAmount {
    fn from(sats: u64) -> Self {
        Self(sats)
    }
}

impl From<SatoshiAmount> for u64 {
    fn from(amount: SatoshiAmount) -> Self {
        amount.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_creation() {
        let amt = SatoshiAmount::new(10_000).unwrap();
        assert_eq!(amt.as_u64(), 10_000);
        assert_eq!(amt.as_btc(), 0.0001);
        assert_eq!(amt.to_string(), "10000 sats");
    }

    #[test]
    fn test_amount_exceeds_max() {
        let res = SatoshiAmount::new(SatoshiAmount::MAX_SATS + 1);
        assert!(res.is_err());
    }
}
