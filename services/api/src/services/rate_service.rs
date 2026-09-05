use chrono::{DateTime, Duration as ChronoDuration, Utc};
use hanbova_core::rate::HanbovaRate;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;

use crate::providers::{PlatformRateProvider, ProviderError, ProviderResult};

#[derive(Debug, Clone)]
struct CachedRateEntry {
    rate: HanbovaRate,
    cached_at: DateTime<Utc>,
}

type RateCacheMap = Arc<RwLock<HashMap<(String, String, String), CachedRateEntry>>>;

/// Service managing indicative platform rates with short caching and a stale fallback window.
#[derive(Clone)]
pub struct HanbovaRateService {
    provider: Arc<dyn PlatformRateProvider>,
    cache: RateCacheMap,
    fresh_ttl: Duration,
    stale_ttl: Duration,
}

impl HanbovaRateService {
    /// Creates a new rate service with default cache windows (45s fresh, 10 min stale).
    pub fn new(provider: Arc<dyn PlatformRateProvider>) -> Self {
        Self::with_ttls(
            provider,
            Duration::from_secs(45),
            Duration::from_secs(600), // 10 minutes
        )
    }

    /// Creates a rate service with explicit cache windows (useful for unit testing).
    pub fn with_ttls(
        provider: Arc<dyn PlatformRateProvider>,
        fresh_ttl: Duration,
        stale_ttl: Duration,
    ) -> Self {
        Self {
            provider,
            cache: Arc::new(RwLock::new(HashMap::new())),
            fresh_ttl,
            stale_ttl,
        }
    }

    pub fn provider(&self) -> &Arc<dyn PlatformRateProvider> {
        &self.provider
    }

    /// Retrieves the indicative rate for the given market and currency pair.
    pub async fn get_rate(
        &self,
        market: &str,
        settlement_asset: &str,
        target_currency: &str,
    ) -> ProviderResult<HanbovaRate> {
        let key = (
            market.trim().to_uppercase(),
            settlement_asset.trim().to_uppercase(),
            target_currency.trim().to_uppercase(),
        );

        let now = Utc::now();

        // 1. Check fresh cache
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&key) {
                let age = now.signed_duration_since(entry.cached_at);
                if age
                    < ChronoDuration::from_std(self.fresh_ttl)
                        .unwrap_or(ChronoDuration::seconds(45))
                {
                    let mut fresh_rate = entry.rate.clone();
                    fresh_rate.is_stale = false;
                    return Ok(fresh_rate);
                }
            }
        }

        // 2. Fetch fresh rate from provider
        match self.provider.get_rate(&key.0, &key.1, &key.2).await {
            Ok(fresh_rate) => {
                let mut cache = self.cache.write().await;
                cache.insert(
                    key,
                    CachedRateEntry {
                        rate: fresh_rate.clone(),
                        cached_at: now,
                    },
                );
                Ok(fresh_rate)
            }
            Err(err) => {
                // 3. Fallback to stale cached entry if within stale window
                let cache = self.cache.read().await;
                if let Some(entry) = cache.get(&key) {
                    let age = now.signed_duration_since(entry.cached_at);
                    if age
                        < ChronoDuration::from_std(self.stale_ttl)
                            .unwrap_or(ChronoDuration::seconds(600))
                    {
                        tracing::warn!(
                            market = %key.0,
                            asset = %key.1,
                            currency = %key.2,
                            error = %err,
                            "Provider failed; returning stale rate within fallback window"
                        );
                        let mut stale_rate = entry.rate.clone();
                        stale_rate.is_stale = true;
                        return Ok(stale_rate);
                    }
                }

                tracing::error!(
                    market = %key.0,
                    asset = %key.1,
                    currency = %key.2,
                    error = %err,
                    "Provider failed and no trustworthy stale rate available"
                );

                Err(ProviderError::Unavailable(
                    "Rate temporarily unavailable".to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockRateProvider;

    #[tokio::test]
    async fn test_rate_service_caches_fresh_rate() {
        let mock = MockRateProvider::new(1365.0);
        let mock_ptr = Arc::new(mock.clone());
        let service = HanbovaRateService::with_ttls(
            mock_ptr.clone(),
            Duration::from_secs(60),
            Duration::from_secs(600),
        );

        let rate1 = service.get_rate("NG", "USDT", "NGN").await.unwrap();
        assert_eq!(rate1.rate, 1365.0);
        assert!(!rate1.is_stale);

        // Even if underlying provider were changed, cached value persists
        let rate2 = service.get_rate("NG", "USDT", "NGN").await.unwrap();
        assert_eq!(rate2.rate, 1365.0);
    }

    #[tokio::test]
    async fn test_rate_service_stale_fallback_on_provider_error() {
        let mock = Arc::new(RwLock::new(MockRateProvider::new(1400.0)));

        struct DynamicMock {
            inner: Arc<RwLock<MockRateProvider>>,
        }
        #[async_trait::async_trait]
        impl PlatformRateProvider for DynamicMock {
            async fn get_rate(
                &self,
                market: &str,
                asset: &str,
                cur: &str,
            ) -> ProviderResult<HanbovaRate> {
                self.inner.read().await.get_rate(market, asset, cur).await
            }
            fn provider_id(&self) -> &'static str {
                "dynamic_mock"
            }
        }

        let dynamic_mock = Arc::new(DynamicMock {
            inner: mock.clone(),
        });
        // Set fresh TTL very short (1ms) and stale TTL 5s
        let service = HanbovaRateService::with_ttls(
            dynamic_mock,
            Duration::from_millis(1),
            Duration::from_secs(5),
        );

        // First call populates cache
        let rate1 = service.get_rate("NG", "USDT", "NGN").await.unwrap();
        assert_eq!(rate1.rate, 1400.0);
        assert!(!rate1.is_stale);

        // Wait for fresh TTL to expire
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Force provider to fail
        mock.write().await.set_should_fail(true);

        // Next call should return stale cached rate with is_stale = true
        let rate2 = service.get_rate("NG", "USDT", "NGN").await.unwrap();
        assert_eq!(rate2.rate, 1400.0);
        assert!(rate2.is_stale, "Should be marked stale");
    }

    #[tokio::test]
    async fn test_rate_service_unavailable_when_no_cache_and_provider_fails() {
        let mut mock = MockRateProvider::new(1365.0);
        mock.set_should_fail(true);
        let service = HanbovaRateService::new(Arc::new(mock));

        let result = service.get_rate("NG", "USDT", "NGN").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::Unavailable(_)));
    }
}
