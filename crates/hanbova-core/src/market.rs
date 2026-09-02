use serde::{Deserialize, Serialize};

/// 2-letter ISO 3166-1 alpha-2 country code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CountryCode(pub String);

impl CountryCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into().trim().to_uppercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CountryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 3-letter ISO 4217 currency code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CurrencyCode(pub String);

impl CurrencyCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into().trim().to_uppercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Normalized market capability matrix driven by provider capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MarketCapabilities {
    pub payouts: bool,
    pub mobile_money: bool,
    pub cards: bool,
    pub airtime: bool,
    pub data: bool,
    pub electricity: bool,
    pub water: bool,
    pub tv: bool,
    pub internet: bool,
    pub esim: bool,
}

/// Comprehensive information for a destination/spending market.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketInfo {
    pub country: String,
    pub name: String,
    pub flag_emoji: String,
    pub currency: String,
    pub dial_code: String,
    pub environment: String,
    pub source: String,
    pub capabilities: MarketCapabilities,
}

/// User's tri-part country context separating identity, active spending market, and UI currency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCountryContext {
    /// The user's KYC / residency context.
    pub identity_country: String,
    /// The market where the user currently wants to spend or travel.
    pub spend_country: String,
    /// The currency used for display calculations.
    pub display_currency: String,
}

impl UserCountryContext {
    pub fn new(
        identity: impl Into<String>,
        spend: impl Into<String>,
        currency: impl Into<String>,
    ) -> Self {
        Self {
            identity_country: identity.into().trim().to_uppercase(),
            spend_country: spend.into().trim().to_uppercase(),
            display_currency: currency.into().trim().to_uppercase(),
        }
    }
}
