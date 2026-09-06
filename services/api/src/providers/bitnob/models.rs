use serde::{Deserialize, Serialize};

/// Request payload for creating a payout exchange rate quote on Bitnob.
/// Official endpoint: `POST /api/payouts/quotes`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BitnobPayoutQuoteRequest {
    pub from_asset: String,
    pub to_currency: String,
    pub country: String,
    pub source: String,
    pub amount: String,
}

impl BitnobPayoutQuoteRequest {
    /// Constructs a standard indicative rate quote request for Hanbova.
    /// Uses `source = "offchain"` as required by the official API.
    pub fn new_indicative(market: &str, from_asset: &str, to_currency: &str) -> Self {
        Self {
            from_asset: from_asset.trim().to_uppercase(),
            to_currency: to_currency.trim().to_uppercase(),
            country: market.trim().to_uppercase(),
            source: "offchain".to_string(),
            amount: "1".to_string(),
        }
    }
}

/// Exchange rate details inside a Bitnob payout quote.
#[derive(Debug, Clone, Deserialize)]
pub struct BitnobExchangeRate {
    /// Rate can be returned as a JSON number (`1620.50`) or string (`"1620.50"`).
    pub rate: serde_json::Value,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub effective_rate: Option<serde_json::Value>,
}

impl BitnobExchangeRate {
    /// Safely parses the numeric exchange rate and verifies that it is strictly positive and finite.
    pub fn parse_rate(&self) -> Option<f64> {
        match &self.rate {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => {
                let clean = s.replace(',', "");
                clean.trim().parse::<f64>().ok()
            }
            _ => None,
        }
        .filter(|r| r.is_finite() && *r > 0.0)
    }
}

/// The payout quote object returned in `data.payout`.
#[derive(Debug, Clone, Deserialize)]
pub struct BitnobPayoutQuote {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub quote_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub from_asset: Option<String>,
    #[serde(default)]
    pub to_currency: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
    pub exchange_rate: Option<BitnobExchangeRate>,
}

/// Data container holding the payout quote.
#[derive(Debug, Clone, Deserialize)]
pub struct BitnobQuoteData {
    pub payout: Option<BitnobPayoutQuote>,
}

/// Root response structure for `POST /api/payouts/quotes`.
#[derive(Debug, Clone, Deserialize)]
pub struct BitnobQuoteResponse {
    #[serde(default)]
    pub status: Option<bool>,
    #[serde(default)]
    pub success: Option<bool>,
    pub data: Option<BitnobQuoteData>,
    #[serde(default)]
    pub message: Option<String>,
}

/// Response structure for `GET /api/whoami`.
#[derive(Debug, Clone, Deserialize)]
pub struct WhoamiResponse {
    #[serde(default)]
    pub status: Option<bool>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub message: Option<String>,
}

/// Structured error returned by Bitnob API.
#[derive(Debug, Clone, Deserialize)]
pub struct BitnobErrorDetail {
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = BitnobPayoutQuoteRequest::new_indicative("NG", "USDT", "NGN");
        let json_str = serde_json::to_string(&req).expect("serialize request");
        assert!(json_str.contains(r#""from_asset":"USDT""#));
        assert!(json_str.contains(r#""to_currency":"NGN""#));
        assert!(json_str.contains(r#""country":"NG""#));
        assert!(json_str.contains(r#""source":"offchain""#));
        assert!(json_str.contains(r#""amount":"1""#));
    }

    #[test]
    fn test_strict_response_deserialization_numeric_rate() {
        let json_data = r#"{
            "status": true,
            "message": "Quote created successfully",
            "data": {
                "payout": {
                    "id": "payout-quote-001",
                    "from_asset": "USDT",
                    "to_currency": "NGN",
                    "exchange_rate": {
                        "rate": 1620.50,
                        "currency": "ngn"
                    }
                }
            }
        }"#;

        let parsed: BitnobQuoteResponse = serde_json::from_str(json_data).unwrap();
        let payout = parsed.data.unwrap().payout.unwrap();
        assert_eq!(payout.from_asset.as_deref(), Some("USDT"));
        assert_eq!(payout.to_currency.as_deref(), Some("NGN"));
        let rate = payout.exchange_rate.unwrap().parse_rate().unwrap();
        assert_eq!(rate, 1620.50);
    }

    #[test]
    fn test_strict_response_deserialization_string_rate() {
        let json_data = r#"{
            "status": true,
            "data": {
                "payout": {
                    "from_asset": "USDT",
                    "to_currency": "NGN",
                    "exchange_rate": {
                        "rate": "1,625.75",
                        "currency": "ngn"
                    }
                }
            }
        }"#;

        let parsed: BitnobQuoteResponse = serde_json::from_str(json_data).unwrap();
        let payout = parsed.data.unwrap().payout.unwrap();
        let rate = payout.exchange_rate.unwrap().parse_rate().unwrap();
        assert_eq!(rate, 1625.75);
    }

    #[test]
    fn test_rate_validation_rejects_zero_or_negative() {
        let er_zero = BitnobExchangeRate {
            rate: serde_json::json!(0.0),
            currency: Some("ngn".to_string()),
            effective_rate: None,
        };
        assert!(er_zero.parse_rate().is_none());

        let er_neg = BitnobExchangeRate {
            rate: serde_json::json!(-1500.0),
            currency: Some("ngn".to_string()),
            effective_rate: None,
        };
        assert!(er_neg.parse_rate().is_none());

        let er_invalid_str = BitnobExchangeRate {
            rate: serde_json::json!("not_a_number"),
            currency: Some("ngn".to_string()),
            effective_rate: None,
        };
        assert!(er_invalid_str.parse_rate().is_none());
    }
}
