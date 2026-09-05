use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Source indicating how a rate was derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateSource {
    LiveProvider,
    CachedProvider,
    StaleCache,
    Mock,
}

/// Freshness status of the rate data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateFreshness {
    Fresh,
    Stale,
    Expired,
}

/// Represents the customer-facing settlement / conversion rate that Hanbova
/// can actually offer through its configured provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HanbovaRate {
    /// Market / country code (e.g., "NG")
    pub market: String,
    /// Base display currency (e.g., "USD")
    pub base: String,
    /// Target quote currency (e.g., "NGN")
    pub quote: String,
    /// Customer-facing formatted string (e.g., "$1 = ₦1,365.00")
    pub display: String,
    /// Settlement asset (e.g., "USDT")
    pub settlement_asset: String,
    /// Numerical rate value (e.g., 1365.00)
    pub rate: f64,
    /// Provider identifier (e.g., "bitnob")
    pub provider: String,
    /// Whether this is a verified live rate from the provider
    pub is_live: bool,
    /// Whether this rate is currently stale (provider temporarily unreachable)
    pub is_stale: bool,
    /// When this rate was retrieved/updated
    pub updated_at: DateTime<Utc>,
    /// Optional expiration timestamp from provider
    pub expires_at: Option<DateTime<Utc>>,
}

impl HanbovaRate {
    /// Formats a USD/NGN rate into the standard customer-facing display string.
    pub fn format_display(base: &str, quote: &str, rate: f64) -> String {
        let base_symbol = match base {
            "USD" => "$1",
            "EUR" => "€1",
            "GBP" => "£1",
            _ => base,
        };

        let quote_formatted = if quote == "NGN" {
            format!("₦{}", format_number_with_commas(rate))
        } else if quote == "KES" {
            format!("KSh {}", format_number_with_commas(rate))
        } else if quote == "GHS" {
            format!("GH₵ {}", format_number_with_commas(rate))
        } else if quote == "ZAR" {
            format!("R {}", format_number_with_commas(rate))
        } else {
            format!("{quote} {:.2}", rate)
        };

        format!("{base_symbol} = {quote_formatted}")
    }

    /// Creates a new HanbovaRate instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        market: impl Into<String>,
        base: impl Into<String>,
        quote: impl Into<String>,
        settlement_asset: impl Into<String>,
        rate: f64,
        provider: impl Into<String>,
        is_live: bool,
        is_stale: bool,
        updated_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        let base_str = base.into();
        let quote_str = quote.into();
        let display = Self::format_display(&base_str, &quote_str, rate);

        Self {
            market: market.into(),
            base: base_str,
            quote: quote_str,
            display,
            settlement_asset: settlement_asset.into(),
            rate,
            provider: provider.into(),
            is_live,
            is_stale,
            updated_at,
            expires_at,
        }
    }

    /// Creates a deterministic mock rate for development and testing.
    ///
    /// NOTE: Mock rates are NEVER marked `is_live: true`.
    pub fn mock_ngn(rate: f64, updated_at: DateTime<Utc>) -> Self {
        Self::new(
            "NG", "USD", "NGN", "USDT", rate, "bitnob", false, // is_live: FALSE
            false, // is_stale: false
            updated_at, None,
        )
    }
}

/// Helper function to format numbers with commas and 2 decimal places.
fn format_number_with_commas(num: f64) -> String {
    let integer_part = num.trunc().abs() as u64;
    let decimal_part = ((num.abs() - (integer_part as f64)) * 100.0).round() as u64;

    let integer_str = integer_part.to_string();
    let mut result = String::new();
    let chars: Vec<char> = integer_str.chars().collect();
    let len = chars.len();

    for (i, &ch) in chars.iter().enumerate() {
        result.push(ch);
        let remaining = len - 1 - i;
        if remaining > 0 && remaining.is_multiple_of(3) {
            result.push(',');
        }
    }

    format!("{}.{:02}", result, decimal_part)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_display() {
        assert_eq!(
            HanbovaRate::format_display("USD", "NGN", 1365.0),
            "$1 = ₦1,365.00"
        );
        assert_eq!(
            HanbovaRate::format_display("USD", "NGN", 1520.50),
            "$1 = ₦1,520.50"
        );
        assert_eq!(
            HanbovaRate::format_display("USD", "KES", 130.25),
            "$1 = KSh 130.25"
        );
    }

    #[test]
    fn test_mock_rate_is_never_live() {
        let rate = HanbovaRate::mock_ngn(1365.0, Utc::now());
        assert!(!rate.is_live);
        assert!(!rate.is_stale);
        assert_eq!(rate.market, "NG");
        assert_eq!(rate.base, "USD");
        assert_eq!(rate.quote, "NGN");
        assert_eq!(rate.settlement_asset, "USDT");
        assert_eq!(rate.display, "$1 = ₦1,365.00");
    }
}
