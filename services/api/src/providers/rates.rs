use async_trait::async_trait;
use chrono::Utc;
use hanbova_core::rate::HanbovaRate;
use hex::ToHex;
use hmac::{Hmac, Mac};
use rand::RngCore;
use reqwest::Client;
use serde::Deserialize;
use sha2::Sha256;
use std::time::Duration;

use super::{ProviderError, ProviderResult};

type HmacSha256 = Hmac<Sha256>;

/// Provider-neutral abstraction for retrieving live indicative platform rates.
#[async_trait]
pub trait PlatformRateProvider: Send + Sync {
    /// Retrieve the current indicative rate for a market and asset pair.
    async fn get_rate(
        &self,
        market: &str,
        settlement_asset: &str,
        target_currency: &str,
    ) -> ProviderResult<HanbovaRate>;

    /// Short identifier for the provider (e.g., "bitnob", "flutterwave", "mock").
    fn provider_id(&self) -> &'static str;
}

/// Bitnob rate provider supporting mock, sandbox, and production modes.
#[derive(Debug, Clone)]
pub struct BitnobRateProvider {
    client_id: Option<String>,
    client_secret: Option<String>,
    api_key: Option<String>,
    environment: String, // "mock", "sandbox", "production"
    base_url: String,
    http_client: Client,
}

impl BitnobRateProvider {
    /// Constructs a BitnobRateProvider from environment variables.
    pub fn new() -> Self {
        let client_id = std::env::var("BITNOB_CLIENT_ID")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let client_secret = std::env::var("BITNOB_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let api_key = std::env::var("BITNOB_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let environment = std::env::var("BITNOB_ENVIRONMENT").unwrap_or_else(|_| {
            if client_id.is_some() || api_key.is_some() {
                "sandbox".to_string()
            } else {
                "mock".to_string()
            }
        });

        let base_url = match environment.as_str() {
            "production" => "https://api.bitnob.co".to_string(),
            _ => "https://sandboxapi.bitnob.co".to_string(),
        };

        let http_client = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        Self {
            client_id,
            client_secret,
            api_key,
            environment,
            base_url,
            http_client,
        }
    }

    /// Builder method for deterministic testing with explicit config.
    pub fn with_config(
        client_id: Option<String>,
        client_secret: Option<String>,
        environment: &str,
        base_url: Option<String>,
    ) -> Self {
        let env_str = environment.to_string();
        let default_url = match env_str.as_str() {
            "production" => "https://api.bitnob.co".to_string(),
            _ => "https://sandboxapi.bitnob.co".to_string(),
        };

        let http_client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            client_id,
            client_secret,
            api_key: None,
            environment: env_str,
            base_url: base_url.unwrap_or(default_url),
            http_client,
        }
    }

    pub fn environment(&self) -> &str {
        &self.environment
    }

    /// Computes HMAC-SHA256 signature for Bitnob request authentication.
    /// Canonical format: `CLIENT_ID:TIMESTAMP:NONCE:PAYLOAD`
    fn generate_signature(
        client_id: &str,
        client_secret: &str,
        timestamp: u64,
        nonce: &str,
        payload: &str,
    ) -> ProviderResult<String> {
        let canonical_message = format!("{client_id}:{timestamp}:{nonce}:{payload}");
        let mut mac = HmacSha256::new_from_slice(client_secret.as_bytes())
            .map_err(|e| ProviderError::Internal(format!("HMAC initialization failed: {e}")))?;
        mac.update(canonical_message.as_bytes());
        let result = mac.finalize();
        Ok(result.into_bytes().as_slice().encode_hex::<String>())
    }
}

impl Default for BitnobRateProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BitnobExchangeRateDetails {
    rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BitnobQuoteData {
    rate: Option<f64>,
    exchange_rate: Option<BitnobExchangeRateDetails>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BitnobQuoteResponse {
    status: Option<bool>,
    data: Option<BitnobQuoteData>,
    message: Option<String>,
}

#[async_trait]
impl PlatformRateProvider for BitnobRateProvider {
    fn provider_id(&self) -> &'static str {
        "bitnob"
    }

    async fn get_rate(
        &self,
        market: &str,
        settlement_asset: &str,
        target_currency: &str,
    ) -> ProviderResult<HanbovaRate> {
        let market_upper = market.trim().to_uppercase();
        let asset_upper = settlement_asset.trim().to_uppercase();
        let currency_upper = target_currency.trim().to_uppercase();

        // 1. Explicit mock mode returns deterministic rate (NEVER marked live)
        if self.environment == "mock" {
            let mock_rate = match (asset_upper.as_str(), currency_upper.as_str()) {
                ("USDT", "NGN") | ("USD", "NGN") => 1365.00,
                ("USDT", "KES") | ("USD", "KES") => 132.50,
                ("USDT", "GHS") | ("USD", "GHS") => 15.40,
                ("USDT", "ZAR") | ("USD", "ZAR") => 18.20,
                _ => 1365.00,
            };

            return Ok(HanbovaRate::new(
                market_upper,
                "USD",
                currency_upper,
                asset_upper,
                mock_rate,
                "bitnob",
                false, // is_live = false for mock
                false, // is_stale = false
                Utc::now(),
                None,
            ));
        }

        // 2. Production or Sandbox mode with credentials: query Bitnob API
        let (client_id, client_secret) = match (&self.client_id, &self.client_secret) {
            (Some(cid), Some(sec)) => (cid.as_str(), sec.as_str()),
            _ => {
                if self.environment == "production" {
                    return Err(ProviderError::NotConfigured(
                        "Bitnob API credentials missing in production".to_string(),
                    ));
                } else if let Some(key) = &self.api_key {
                    // Legacy API key fallback in sandbox only
                    (key.as_str(), "")
                } else {
                    return Err(ProviderError::NotConfigured(
                        "Bitnob credentials not configured".to_string(),
                    ));
                }
            }
        };

        let timestamp = Utc::now().timestamp() as u64;
        let mut nonce_bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = nonce_bytes.encode_hex::<String>();

        let payload = serde_json::json!({
            "from_asset": asset_upper,
            "to_currency": currency_upper,
            "amount": "1",
            "country": market_upper
        })
        .to_string();

        let mut request = self
            .http_client
            .post(format!("{}/api/v1/payouts/quotes", self.base_url))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");

        if !client_secret.is_empty() {
            let signature =
                Self::generate_signature(client_id, client_secret, timestamp, &nonce, &payload)?;

            request = request
                .header("X-Auth-Client", client_id)
                .header("X-Auth-Timestamp", timestamp.to_string())
                .header("X-Auth-Nonce", nonce)
                .header("X-Auth-Signature", signature);
        } else {
            // Bearer token fallback for legacy sandbox
            request = request.header("Authorization", format!("Bearer {client_id}"));
        }

        let response = request
            .body(payload)
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(format!("Bitnob request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let err_body = response.text().await.unwrap_or_default();
            tracing::warn!(
                status = %status,
                "Bitnob rate endpoint returned non-success"
            );
            return Err(ProviderError::Unavailable(format!(
                "Bitnob HTTP {status}: {err_body}"
            )));
        }

        let parsed = response
            .json::<BitnobQuoteResponse>()
            .await
            .map_err(|e| ProviderError::Internal(format!("Failed to parse Bitnob quote: {e}")))?;

        let extracted_rate = parsed.data.as_ref().and_then(|d| {
            d.rate
                .or_else(|| d.exchange_rate.as_ref().and_then(|er| er.rate))
        });

        match extracted_rate {
            Some(rate) if rate > 0.0 => Ok(HanbovaRate::new(
                market_upper,
                "USD",
                currency_upper,
                asset_upper,
                rate,
                "bitnob",
                true,  // is_live = true for verified provider quote
                false, // is_stale = false
                Utc::now(),
                None,
            )),
            _ => Err(ProviderError::Unavailable(parsed.message.unwrap_or_else(
                || "No rate returned in Bitnob response".to_string(),
            ))),
        }
    }
}

/// A controllable mock provider for tests and local determinism.
#[derive(Debug, Clone)]
pub struct MockRateProvider {
    rate: f64,
    should_fail: bool,
}

impl MockRateProvider {
    pub fn new(rate: f64) -> Self {
        Self {
            rate,
            should_fail: false,
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
    }

    pub fn set_should_fail(&mut self, fail: bool) {
        self.should_fail = fail;
    }
}

#[async_trait]
impl PlatformRateProvider for MockRateProvider {
    fn provider_id(&self) -> &'static str {
        "mock"
    }

    async fn get_rate(
        &self,
        market: &str,
        settlement_asset: &str,
        target_currency: &str,
    ) -> ProviderResult<HanbovaRate> {
        if self.should_fail {
            return Err(ProviderError::Unavailable(
                "Simulated provider failure".to_string(),
            ));
        }

        Ok(HanbovaRate::new(
            market,
            "USD",
            target_currency,
            settlement_asset,
            self.rate,
            "mock",
            false, // mock is never live
            false,
            Utc::now(),
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_signature_generation() {
        let client_id = "test-client-id";
        let client_secret = "test-client-secret-1234567890";
        let timestamp = 1719236465;
        let nonce = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
        let payload = r#"{"amount":"1"}"#;

        let sig = BitnobRateProvider::generate_signature(
            client_id,
            client_secret,
            timestamp,
            nonce,
            payload,
        )
        .expect("signature");

        assert!(!sig.is_empty());
        assert_eq!(sig.len(), 64); // SHA-256 hex string is 64 characters
    }

    #[tokio::test]
    async fn test_mock_environment_returns_mock_rate_not_live() {
        let provider = BitnobRateProvider::with_config(None, None, "mock", None);
        let rate = provider
            .get_rate("NG", "USDT", "NGN")
            .await
            .expect("mock rate");

        assert_eq!(rate.market, "NG");
        assert_eq!(rate.quote, "NGN");
        assert_eq!(rate.settlement_asset, "USDT");
        assert_eq!(rate.rate, 1365.0);
        assert!(!rate.is_live, "Mock rate must NOT be live");
        assert!(!rate.is_stale);
    }

    #[tokio::test]
    async fn test_production_without_credentials_fails() {
        let provider = BitnobRateProvider::with_config(None, None, "production", None);
        let result = provider.get_rate("NG", "USDT", "NGN").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::NotConfigured(_)
        ));
    }
}
