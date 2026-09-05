use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use hanbova_api::providers::{BitnobRateProvider, MockRateProvider, PlatformRateProvider};
use hanbova_core::rate::HanbovaRate;
use http_body_util::BodyExt;
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

#[test]
fn test_mock_rate_not_marked_live() {
    let rate = HanbovaRate::mock_ngn(1365.0, Utc::now());
    assert!(!rate.is_live, "Mock rate must never be marked live");
    assert!(!rate.is_stale, "Mock rate should not be stale initially");
    assert_eq!(rate.market, "NG");
    assert_eq!(rate.quote, "NGN");
    assert_eq!(rate.settlement_asset, "USDT");
    assert_eq!(rate.rate, 1365.0);
    assert_eq!(rate.display, "$1 = ₦1,365.00");
}

#[tokio::test]
async fn test_bitnob_mock_environment_returns_truthful_non_live_rate() {
    let provider = BitnobRateProvider::with_config(None, None, "mock", None);
    let rate = provider
        .get_rate("NG", "USDT", "NGN")
        .await
        .expect("rate fetch");

    assert_eq!(rate.market, "NG");
    assert_eq!(rate.rate, 1365.0);
    assert!(!rate.is_live, "Bitnob in mock mode must NOT claim is_live");
    assert!(!rate.is_stale);
    assert_eq!(rate.display, "$1 = ₦1,365.00");
}

#[tokio::test]
async fn test_production_provider_failure_never_silently_falls_back_to_mock() {
    // A production provider without credentials or when connection fails
    // must strictly error, not return mock data
    let prod_provider = BitnobRateProvider::with_config(None, None, "production", None);
    let result = prod_provider.get_rate("NG", "USDT", "NGN").await;
    assert!(
        result.is_err(),
        "Production mode must fail when unconfigured"
    );

    // In a rate service with no prior cache, it must propagate as Unavailable
    let service = hanbova_api::services::HanbovaRateService::new(Arc::new(prod_provider));
    let service_result = service.get_rate("NG", "USDT", "NGN").await;
    assert!(
        service_result.is_err(),
        "Production service failure must never silently return mock data"
    );
}

#[tokio::test]
async fn test_rate_service_caches_fresh_rate() {
    let mock = Arc::new(MockRateProvider::new(1365.0));
    let service = hanbova_api::services::HanbovaRateService::with_ttls(
        mock,
        Duration::from_secs(60),
        Duration::from_secs(600),
    );

    let rate1 = service.get_rate("NG", "USDT", "NGN").await.unwrap();
    let rate2 = service.get_rate("NG", "USDT", "NGN").await.unwrap();

    assert_eq!(rate1.rate, rate2.rate);
    assert_eq!(rate1.updated_at, rate2.updated_at);
    assert!(!rate1.is_stale);
    assert!(!rate2.is_stale);
}

#[tokio::test]
async fn test_rate_service_stale_window_and_unavailable_after_expiration() {
    use tokio::sync::RwLock;

    let mock_inner = Arc::new(RwLock::new(MockRateProvider::new(1365.0)));

    struct ControllableProvider {
        inner: Arc<RwLock<MockRateProvider>>,
    }

    #[async_trait::async_trait]
    impl PlatformRateProvider for ControllableProvider {
        async fn get_rate(
            &self,
            market: &str,
            settlement_asset: &str,
            target_currency: &str,
        ) -> hanbova_api::providers::ProviderResult<HanbovaRate> {
            self.inner
                .read()
                .await
                .get_rate(market, settlement_asset, target_currency)
                .await
        }

        fn provider_id(&self) -> &'static str {
            "controllable"
        }
    }

    let controllable = Arc::new(ControllableProvider {
        inner: mock_inner.clone(),
    });

    // 10ms fresh TTL, 50ms stale TTL
    let service = hanbova_api::services::HanbovaRateService::with_ttls(
        controllable,
        Duration::from_millis(10),
        Duration::from_millis(60),
    );

    // 1. Initial successful query
    let rate1 = service.get_rate("NG", "USDT", "NGN").await.unwrap();
    assert_eq!(rate1.rate, 1365.0);
    assert!(!rate1.is_stale);

    // Wait for fresh cache to expire
    tokio::time::sleep(Duration::from_millis(15)).await;

    // Simulate provider failure
    mock_inner.write().await.set_should_fail(true);

    // 2. Query within stale window (before 60ms) -> Returns stale rate
    let rate2 = service.get_rate("NG", "USDT", "NGN").await.unwrap();
    assert_eq!(rate2.rate, 1365.0);
    assert!(rate2.is_stale, "Should be flagged as is_stale: true");

    // Wait for stale window to expire
    tokio::time::sleep(Duration::from_millis(60)).await;

    // 3. Query after stale window expires -> Returns Unavailable error
    let rate3 = service.get_rate("NG", "USDT", "NGN").await;
    assert!(
        rate3.is_err(),
        "Must return Unavailable after stale window expires"
    );
}

#[test]
fn test_secrets_never_appear_in_serialization() {
    let rate = HanbovaRate::new(
        "NG",
        "USD",
        "NGN",
        "USDT",
        1365.0,
        "bitnob",
        true,
        false,
        Utc::now(),
        None,
    );

    let serialized = serde_json::to_string(&rate).expect("serialize");
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("api_key"));
    assert!(!serialized.contains("client_secret"));
    assert!(!serialized.contains("signature"));
    assert!(serialized.contains(r#""display":"$1 = ₦1,365.00""#));
    assert!(serialized.contains(r#""rate":1365.0"#));
    assert!(serialized.contains(r#""is_live":true"#));
    assert!(serialized.contains(r#""is_stale":false"#));
}

#[tokio::test]
async fn test_api_endpoint_get_hanbova_rate_success() {
    let config = hanbova_api::config::AppConfig::from_iter([
        ("HANBOVA_ENV", "development"),
        ("HANBOVA_API_HOST", "127.0.0.1"),
        ("HANBOVA_API_PORT", "8080"),
    ])
    .unwrap();

    let state = hanbova_api::state::AppState::new(config, None);
    let app = hanbova_api::build_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/rates/hanbova?market=NG")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["market"], "NG");
    assert_eq!(json["base"], "USD");
    assert_eq!(json["quote"], "NGN");
    assert_eq!(json["settlement_asset"], "USDT");
    assert_eq!(json["display"], "$1 = ₦1,365.00");
    assert_eq!(json["rate"], 1365.0);
    assert_eq!(json["is_stale"], false);
    // Development default is mock mode, so is_live is false
    assert_eq!(json["is_live"], false);
}

#[tokio::test]
async fn test_api_endpoint_handles_unavailable_state() {
    let config = hanbova_api::config::AppConfig::from_iter([
        ("HANBOVA_ENV", "development"),
        ("HANBOVA_API_HOST", "127.0.0.1"),
        ("HANBOVA_API_PORT", "8080"),
    ])
    .unwrap();

    let mut state = hanbova_api::state::AppState::new(config, None);

    // Override rate service with one configured to fail
    let mut failing_mock = MockRateProvider::new(1365.0);
    failing_mock.set_should_fail(true);
    state.rate_service = Arc::new(hanbova_api::services::HanbovaRateService::new(Arc::new(
        failing_mock,
    )));

    let app = hanbova_api::build_app(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/rates/hanbova?market=NG")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "Rate temporarily unavailable");
    assert_eq!(json["code"], "rate_unavailable");
}
