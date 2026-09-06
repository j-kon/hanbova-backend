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

/// All supported Hanbova markets with their settlement currency.
pub const ALL_MARKETS: &[(&str, &str, &str)] = &[
    ("NG", "USDT", "NGN"),
    ("KE", "USDT", "KES"),
    ("GH", "USDT", "GHS"),
    ("ZA", "USDT", "ZAR"),
    ("UG", "USDT", "UGX"),
    ("RW", "USDT", "RWF"),
    ("TZ", "USDT", "TZS"),
    ("US", "USDT", "USD"),
];

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

    /// Retrieve rates for all known Hanbova markets concurrently.
    /// Default implementation fans out to `get_rate` for each market.
    /// Individual market failures return `None` for that slot — they never
    /// poison the overall response.
    async fn get_all_rates(&self) -> Vec<Option<HanbovaRate>> {
        let mut results = Vec::with_capacity(ALL_MARKETS.len());
        for (market, asset, currency) in ALL_MARKETS {
            let result = self.get_rate(market, asset, currency).await.ok();
            results.push(result);
        }
        results
    }

    /// Short identifier for the provider (e.g., "bitnob", "flutterwave", "mock").
    fn provider_id(&self) -> &'static str;
}

/// Bitnob rate provider supporting mock, sandbox, and production modes.
#[derive(Debug, Clone)]
pub struct BitnobRateProvider {
    client_id: Option<String>,
    client_secret: Option<String>,
    mode: crate::config::ProviderMode,
    base_url: String,
    http_client: Client,
}

impl BitnobRateProvider {
    /// Constructs a BitnobRateProvider from environment variables.
    pub fn new() -> Self {
        let mode = match std::env::var("PROVIDER_MODE").as_deref() {
            Ok("sandbox") => crate::config::ProviderMode::Sandbox,
            Ok("production") => crate::config::ProviderMode::Production,
            _ => crate::config::ProviderMode::Mock,
        };
        Self::with_mode(mode)
    }

    pub fn with_mode(mode: crate::config::ProviderMode) -> Self {
        let client_id = std::env::var("BITNOB_CLIENT_ID")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let client_secret = std::env::var("BITNOB_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let base_url = match mode {
            crate::config::ProviderMode::Production => "https://api.bitnob.com".to_string(),
            _ => "https://sandboxapi.bitnob.co".to_string(),
        };

        let http_client = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        Self {
            client_id,
            client_secret,
            mode,
            base_url,
            http_client,
        }
    }

    /// Builder method for deterministic testing with explicit config.
    pub fn with_config(
        client_id: Option<String>,
        client_secret: Option<String>,
        mode: crate::config::ProviderMode,
        base_url: Option<String>,
    ) -> Self {
        let default_url = match mode {
            crate::config::ProviderMode::Production => "https://api.bitnob.com".to_string(),
            _ => "https://sandboxapi.bitnob.co".to_string(),
        };

        let http_client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            client_id,
            client_secret,
            mode,
            base_url: base_url.unwrap_or(default_url),
            http_client,
        }
    }

    pub fn is_configured(&self) -> bool {
        match self.mode {
            crate::config::ProviderMode::Mock => true,
            crate::config::ProviderMode::Sandbox | crate::config::ProviderMode::Production => {
                self.client_id.is_some() && self.client_secret.is_some()
            }
        }
    }

    pub fn mode(&self) -> crate::config::ProviderMode {
        self.mode
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

/// Helper to recursively extract a numeric rate from various Bitnob response layouts.
/// Bitnob may return the exchange rate as a float (`1610.50`) or as a string (`"1610.50"`),
/// nested under `data.payout.exchange_rate.rate`, `data.quote.exchange_rate.rate`,
/// `data.exchange_rate.rate`, or directly as `data.rate`.
fn extract_rate_from_value(val: &serde_json::Value) -> Option<f64> {
    match val {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => {
            let clean = s.replace(',', "");
            clean.trim().parse::<f64>().ok()
        }
        serde_json::Value::Object(map) => {
            // 1. Direct "rate"
            if let Some(r) = map.get("rate").and_then(extract_rate_from_value) {
                return Some(r);
            }
            // 2. "exchange_rate" object
            if let Some(er) = map.get("exchange_rate").and_then(extract_rate_from_value) {
                return Some(er);
            }
            // 3. "payout" container
            if let Some(payout) = map.get("payout").and_then(extract_rate_from_value) {
                return Some(payout);
            }
            // 4. "quote" container
            if let Some(quote) = map.get("quote").and_then(extract_rate_from_value) {
                return Some(quote);
            }
            None
        }
        serde_json::Value::Array(arr) => arr.first().and_then(extract_rate_from_value),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct BitnobQuoteResponse {
    #[serde(default)]
    status: Option<bool>,
    #[serde(default)]
    success: Option<bool>,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
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
        if self.mode == crate::config::ProviderMode::Mock {
            // USD→USD is meaningless; instead express as 1 USDT = $1.00 (tether peg)
            // Rates are indicative mock values only.
            let mock_rate = match (asset_upper.as_str(), currency_upper.as_str()) {
                ("USDT", "NGN") | ("USD", "NGN") => 1_565.00,
                ("USDT", "KES") | ("USD", "KES") => 132.50,
                ("USDT", "GHS") | ("USD", "GHS") => 15.40,
                ("USDT", "ZAR") | ("USD", "ZAR") => 18.20,
                ("USDT", "UGX") | ("USD", "UGX") => 3_750.00,
                ("USDT", "RWF") | ("USD", "RWF") => 1_310.00,
                ("USDT", "TZS") | ("USD", "TZS") => 2_680.00,
                // USD market: 1 USDT ≈ $1.00 (tether peg)
                ("USDT", "USD") | ("USD", "USD") => 1.00,
                _ => 1_565.00,
            };

            return Ok(HanbovaRate::new(
                market_upper,
                "USD",
                currency_upper,
                asset_upper,
                mock_rate,
                "bitnob",
                "mock",
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
                let env_name = self.mode.to_string();
                return Err(ProviderError::NotConfigured(format!(
                    "Bitnob credentials (BITNOB_CLIENT_ID, BITNOB_CLIENT_SECRET) missing in {env_name}"
                )));
            }
        };

        let start_time = std::time::Instant::now();

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

        let signature =
            Self::generate_signature(client_id, client_secret, timestamp, &nonce, &payload)?;

        let request = self
            .http_client
            .post(format!("{}/api/v1/payouts/quotes", self.base_url))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("X-Auth-Client", client_id)
            .header("X-Auth-Timestamp", timestamp.to_string())
            .header("X-Auth-Nonce", nonce)
            .header("X-Auth-Signature", signature);

        let response = match request.body(payload).send().await {
            Ok(resp) => resp,
            Err(err) => {
                let latency_ms = start_time.elapsed().as_millis() as u64;
                tracing::warn!(
                    provider = "bitnob",
                    environment = %self.mode,
                    operation = "rate_quote",
                    market = %market_upper,
                    result = "failure",
                    latency_ms = latency_ms,
                    error = %err,
                    "Bitnob request network failure"
                );
                return Err(ProviderError::Unavailable(format!(
                    "Bitnob request failed: {err}"
                )));
            }
        };

        let status = response.status();
        let latency_ms = start_time.elapsed().as_millis() as u64;

        if !status.is_success() {
            let err_body = response.text().await.unwrap_or_default();
            tracing::warn!(
                provider = "bitnob",
                environment = %self.mode,
                operation = "rate_quote",
                market = %market_upper,
                result = "failure",
                latency_ms = latency_ms,
                http_status = %status,
                "Bitnob rate endpoint returned non-success HTTP status"
            );
            return Err(ProviderError::Unavailable(format!(
                "Bitnob HTTP {status}: {err_body}"
            )));
        }

        let parsed = match response.json::<BitnobQuoteResponse>().await {
            Ok(p) => p,
            Err(err) => {
                tracing::warn!(
                    provider = "bitnob",
                    environment = %self.mode,
                    operation = "rate_quote",
                    market = %market_upper,
                    result = "failure",
                    latency_ms = latency_ms,
                    error = %err,
                    "Failed to parse Bitnob quote JSON response"
                );
                return Err(ProviderError::Internal(format!(
                    "Failed to parse Bitnob quote: {err}"
                )));
            }
        };

        // Check success flag if explicitly provided
        let is_status_ok = parsed.status.unwrap_or(true) && parsed.success.unwrap_or(true);
        if !is_status_ok {
            let msg = parsed
                .message
                .unwrap_or_else(|| "Bitnob returned unsuccessful status".to_string());
            tracing::warn!(
                provider = "bitnob",
                environment = %self.mode,
                operation = "rate_quote",
                market = %market_upper,
                result = "failure",
                latency_ms = latency_ms,
                message = %msg,
                "Bitnob response indicated failure"
            );
            return Err(ProviderError::Unavailable(msg));
        }

        let extracted_rate = parsed.data.as_ref().and_then(extract_rate_from_value);

        // Under no circumstances can sandbox produce is_live = true
        let is_live = self.mode == crate::config::ProviderMode::Production;

        match extracted_rate {
            Some(rate) if rate.is_finite() && rate > 0.0 => {
                tracing::info!(
                    provider = "bitnob",
                    environment = %self.mode,
                    operation = "rate_quote",
                    market = %market_upper,
                    result = "success",
                    latency_ms = latency_ms,
                    "Bitnob rate quote retrieved successfully"
                );

                Ok(HanbovaRate::new(
                    market_upper,
                    "USD",
                    currency_upper,
                    asset_upper,
                    rate,
                    "bitnob",
                    self.mode.to_string(),
                    is_live, // true ONLY for verified production provider quote, NEVER sandbox
                    false,   // is_stale = false
                    Utc::now(),
                    None,
                ))
            }
            _ => {
                let msg = parsed.message.unwrap_or_else(|| {
                    "No valid positive rate returned in Bitnob response".to_string()
                });
                tracing::warn!(
                    provider = "bitnob",
                    environment = %self.mode,
                    operation = "rate_quote",
                    market = %market_upper,
                    result = "failure",
                    latency_ms = latency_ms,
                    reason = "rate_missing_or_non_positive",
                    "Bitnob rate quote rejected"
                );
                Err(ProviderError::Unavailable(msg))
            }
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
        let provider =
            BitnobRateProvider::with_config(None, None, crate::config::ProviderMode::Mock, None);
        let rate = provider
            .get_rate("NG", "USDT", "NGN")
            .await
            .expect("mock rate");

        assert_eq!(rate.market, "NG");
        assert_eq!(rate.quote, "NGN");
        assert_eq!(rate.settlement_asset, "USDT");
        assert_eq!(rate.rate, 1565.0);
        assert!(!rate.is_live, "Mock rate must NOT be live");
        assert!(!rate.is_stale);
    }

    #[tokio::test]
    async fn test_production_without_credentials_fails() {
        let provider = BitnobRateProvider::with_config(
            None,
            None,
            crate::config::ProviderMode::Production,
            None,
        );
        let result = provider.get_rate("NG", "USDT", "NGN").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::NotConfigured(_)
        ));
    }

    #[tokio::test]
    async fn test_sandbox_without_credentials_fails() {
        let provider =
            BitnobRateProvider::with_config(None, None, crate::config::ProviderMode::Sandbox, None);
        let result = provider.get_rate("NG", "USDT", "NGN").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::NotConfigured(_)
        ));
    }
}
