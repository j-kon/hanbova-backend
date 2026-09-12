use async_trait::async_trait;
use chrono::Utc;
use hanbova_core::rate::HanbovaRate;

use super::{
    bitnob::{BitnobClient, BitnobPayoutQuoteRequest},
    ProviderError, ProviderResult,
};

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
    client: BitnobClient,
}

impl BitnobRateProvider {
    /// Constructs a BitnobRateProvider from environment variables.
    pub fn new() -> Self {
        Self {
            client: BitnobClient::new(),
        }
    }

    pub fn with_mode(mode: crate::config::ProviderMode) -> Self {
        Self {
            client: BitnobClient::with_mode(mode),
        }
    }

    /// Builder method for deterministic testing with explicit config.
    pub fn with_config(
        client_id: Option<String>,
        client_secret: Option<String>,
        mode: crate::config::ProviderMode,
        base_url: Option<String>,
    ) -> Self {
        Self {
            client: BitnobClient::with_config(client_id, client_secret, mode, base_url),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.client.is_configured()
    }

    pub fn mode(&self) -> crate::config::ProviderMode {
        self.client.mode()
    }

    pub fn client(&self) -> &BitnobClient {
        &self.client
    }
}

impl Default for BitnobRateProvider {
    fn default() -> Self {
        Self::new()
    }
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
        if self.client.mode() == crate::config::ProviderMode::Mock {
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

        // 2. Production or Sandbox mode: query Bitnob API via BitnobClient
        let req =
            BitnobPayoutQuoteRequest::new_indicative(&market_upper, &asset_upper, &currency_upper);

        let payout = self.client.create_payout_quote(&req).await?;

        // Validate returned corridor fields if present
        if let Some(ref from) = payout.from_asset {
            if from.trim().to_uppercase() != asset_upper {
                return Err(ProviderError::Unavailable(format!(
                    "Bitnob quote returned unexpected from_asset: {from}"
                )));
            }
        }
        if let Some(ref to) = payout.to_currency {
            if to.trim().to_uppercase() != currency_upper {
                return Err(ProviderError::Unavailable(format!(
                    "Bitnob quote returned unexpected to_currency: {to}"
                )));
            }
        }
        if let Some(ref er) = payout.exchange_rate {
            if let Some(ref cur) = er.currency {
                if cur.trim().to_uppercase() != currency_upper {
                    return Err(ProviderError::Unavailable(format!(
                        "Bitnob quote returned unexpected exchange_rate currency: {cur}"
                    )));
                }
            }
        }

        // Strict rate extraction from payout.exchange_rate.rate
        let rate = payout
            .exchange_rate
            .as_ref()
            .and_then(|er| er.parse_rate())
            .ok_or_else(|| {
                ProviderError::Unavailable(
                    "Missing or invalid exchange rate in Bitnob quote".to_string(),
                )
            })?;

        // Under no circumstances can sandbox produce is_live = true
        let is_live = self.client.mode() == crate::config::ProviderMode::Production;

        Ok(HanbovaRate::new(
            market_upper,
            "USD",
            currency_upper,
            asset_upper,
            rate,
            "bitnob",
            self.client.mode().to_string(),
            is_live, // true ONLY for verified production provider quote, NEVER sandbox
            false,   // is_stale = false
            Utc::now(),
            None,
        ))
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
    use crate::providers::bitnob::auth::generate_signature;

    #[test]
    fn test_hmac_signature_generation() {
        let client_id = "test-client-id";
        let client_secret = "test-client-secret-1234567890";
        let timestamp = 1719236465;
        let nonce = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
        let payload = r#"{"amount":"1"}"#;

        let sig = generate_signature(client_id, client_secret, timestamp, nonce, payload)
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
