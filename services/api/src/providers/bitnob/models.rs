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
    pub reference: String,
}

impl BitnobPayoutQuoteRequest {
    /// Constructs a standard indicative rate quote request for Hanbova.
    /// Uses `source = "offchain"` as required by the official API.
    /// Generates a unique reference for idempotency and tracking.
    pub fn new_indicative(market: &str, from_asset: &str, to_currency: &str) -> Self {
        Self {
            from_asset: from_asset.trim().to_uppercase(),
            to_currency: to_currency.trim().to_uppercase(),
            country: market.trim().to_uppercase(),
            source: "offchain".to_string(),
            amount: "1".to_string(),
            reference: format!("HANBOVA_RATE_{}", uuid::Uuid::new_v4()),
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

/// Dedicated exchange rate payload returned by `GET /api/exchange-rates`.
#[derive(Debug, Clone, Deserialize)]
pub struct BitnobExchangeRateData {
    pub base_currency: Option<String>,
    pub target_currency: Option<String>,
    pub buy_rate: Option<String>,
    pub sell_rate: Option<String>,
    pub mid_rate: Option<String>,
    pub inverse_rate: Option<String>,
    pub valid_for_seconds: Option<u64>,
    pub percent_change_24h: Option<String>,
    pub timestamp: Option<String>,
}

impl BitnobExchangeRateData {
    /// Safely parses the indicative numeric exchange rate from mid_rate or buy_rate.
    pub fn parse_rate(&self) -> Option<f64> {
        self.mid_rate
            .as_ref()
            .or(self.buy_rate.as_ref())
            .and_then(|r| {
                let clean = r.replace(',', "");
                clean.trim().parse::<f64>().ok()
            })
            .filter(|r| r.is_finite() && *r > 0.0)
    }
}

/// Root response structure for `GET /api/exchange-rates`.
#[derive(Debug, Clone, Deserialize)]
pub struct BitnobExchangeRateResponse {
    #[serde(default)]
    pub status: Option<bool>,
    #[serde(default)]
    pub success: Option<bool>,
    pub data: Option<BitnobExchangeRateData>,
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
        assert!(json_str.contains(r#""reference":"HANBOVA_RATE_"#));

        // Two generated requests must not have the same reference
        let req2 = BitnobPayoutQuoteRequest::new_indicative("NG", "USDT", "NGN");
        assert_ne!(
            req.reference, req2.reference,
            "Reference must be unique per request"
        );
    }

    #[test]
    fn test_exchange_rate_data_deserialization() {
        let json_data = r#"{
            "success": true,
            "message": "Exchange rate retrieved",
            "data": {
                "base_currency": "USD",
                "target_currency": "NGN",
                "buy_rate": "1388.52644587",
                "sell_rate": "1374.71306400",
                "mid_rate": "1381.61975494",
                "inverse_rate": "0.00072742",
                "timestamp": "2026-06-18T15:29:17Z",
                "valid_for_seconds": 300,
                "percent_change_24h": "0.00"
            }
        }"#;

        let resp: BitnobExchangeRateResponse = serde_json::from_str(json_data).unwrap();
        assert_eq!(resp.success, Some(true));
        let data = resp.data.unwrap();
        assert_eq!(data.base_currency.as_deref(), Some("USD"));
        assert_eq!(data.target_currency.as_deref(), Some("NGN"));
        assert_eq!(data.parse_rate(), Some(1381.61975494));
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
