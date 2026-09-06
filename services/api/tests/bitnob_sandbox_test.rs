use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use hanbova_api::{
    config::ProviderMode,
    providers::{BitnobRateProvider, PlatformRateProvider, ProviderError},
    services::HanbovaRateService,
};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Clone)]
struct MockServerState {
    response_mode: Arc<AtomicUsize>,
}

// Response modes for mock server
const MODE_SUCCESS_NUMERIC: usize = 0;
const MODE_SUCCESS_STRING: usize = 1;
const MODE_SUCCESS_NESTED_QUOTE: usize = 2;
const MODE_HTTP_401_UNAUTHORIZED: usize = 3;
const MODE_HTTP_500_SERVER_ERROR: usize = 4;
const MODE_MALFORMED_JSON: usize = 5;
const MODE_ZERO_RATE: usize = 6;
const MODE_NEGATIVE_RATE: usize = 7;
const MODE_MISSING_RATE: usize = 8;
const MODE_STATUS_FALSE: usize = 9;

async fn mock_payout_quotes_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    // Verify required Bitnob authentication headers are present
    assert!(
        headers.contains_key("x-auth-client"),
        "Missing X-Auth-Client header"
    );
    assert!(
        headers.contains_key("x-auth-timestamp"),
        "Missing X-Auth-Timestamp header"
    );
    assert!(
        headers.contains_key("x-auth-nonce"),
        "Missing X-Auth-Nonce header"
    );
    assert!(
        headers.contains_key("x-auth-signature"),
        "Missing X-Auth-Signature header"
    );
    assert_eq!(
        headers.get("content-type").unwrap().to_str().unwrap(),
        "application/json"
    );

    // Verify body is valid JSON payload
    let req_json: serde_json::Value =
        serde_json::from_str(&body).expect("Valid JSON payload expected");
    assert_eq!(req_json["from_asset"], "USDT");
    assert_eq!(req_json["to_currency"], "NGN");

    match state.response_mode.load(Ordering::SeqCst) {
        MODE_SUCCESS_NUMERIC => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": true,
                "message": "Quote created successfully",
                "data": {
                    "payout": {
                        "id": "quote-12345",
                        "status": "QUOTE",
                        "from_asset": "USDT",
                        "to_currency": "NGN",
                        "amount": "1",
                        "settlement_amount": "1620.50",
                        "exchange_rate": {
                            "rate": 1620.50,
                            "currency": "ngn"
                        }
                    }
                }
            })
            .to_string(),
        ),
        MODE_SUCCESS_STRING => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": true,
                "message": "Quote created successfully",
                "data": {
                    "exchange_rate": {
                        "rate": "1625.75",
                        "currency": "ngn"
                    }
                }
            })
            .to_string(),
        ),
        MODE_SUCCESS_NESTED_QUOTE => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "success": true,
                "data": {
                    "quote": {
                        "exchange_rate": {
                            "rate": "1630.00"
                        }
                    }
                }
            })
            .to_string(),
        ),
        MODE_HTTP_401_UNAUTHORIZED => (
            StatusCode::UNAUTHORIZED,
            [("content-type", "application/json")],
            json!({
                "status": false,
                "message": "Invalid API Key or Signature"
            })
            .to_string(),
        ),
        MODE_HTTP_500_SERVER_ERROR => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("content-type", "application/json")],
            json!({
                "status": false,
                "message": "Internal service error"
            })
            .to_string(),
        ),
        MODE_MALFORMED_JSON => (
            StatusCode::OK,
            [("content-type", "application/json")],
            r#"{"status": true, "data": {"broken": "#.to_string(),
        ),
        MODE_ZERO_RATE => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": true,
                "data": {
                    "rate": 0.0
                }
            })
            .to_string(),
        ),
        MODE_NEGATIVE_RATE => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": true,
                "data": {
                    "rate": -1550.0
                }
            })
            .to_string(),
        ),
        MODE_MISSING_RATE => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": true,
                "data": {
                    "payout": {
                        "id": "123"
                    }
                }
            })
            .to_string(),
        ),
        MODE_STATUS_FALSE => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": false,
                "message": "Quote expired"
            })
            .to_string(),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("content-type", "application/json")],
            json!({"status": false}).to_string(),
        ),
    }
}

async fn start_mock_server() -> (String, Arc<AtomicUsize>) {
    let response_mode = Arc::new(AtomicUsize::new(MODE_SUCCESS_NUMERIC));
    let state = MockServerState {
        response_mode: response_mode.clone(),
    };

    let app = Router::new()
        .route("/api/v1/payouts/quotes", post(mock_payout_quotes_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener bind");
    let addr = listener.local_addr().expect("listener addr");
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, response_mode)
}

// 1. mock mode remains deterministic and is_live=false
#[tokio::test]
async fn test_1_mock_mode_deterministic_and_not_live() {
    let provider = BitnobRateProvider::with_config(None, None, ProviderMode::Mock, None);
    let rate = provider
        .get_rate("NG", "USDT", "NGN")
        .await
        .expect("mock rate");

    assert_eq!(rate.market, "NG");
    assert_eq!(rate.settlement_asset, "USDT");
    assert_eq!(rate.quote, "NGN");
    assert_eq!(rate.rate, 1565.0);
    assert_eq!(rate.environment, "mock");
    assert_eq!(rate.provider, "bitnob");
    assert!(
        !rate.is_live,
        "Mock rate must strictly have is_live = false"
    );
    assert!(
        !rate.is_stale,
        "Mock rate must strictly have is_stale = false"
    );
}

// 2. sandbox without credentials fails
#[tokio::test]
async fn test_2_sandbox_without_credentials_fails() {
    let provider = BitnobRateProvider::with_config(None, None, ProviderMode::Sandbox, None);
    let result = provider.get_rate("NG", "USDT", "NGN").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::NotConfigured(msg) => {
            assert!(msg.contains("BITNOB_CLIENT_ID"));
            assert!(msg.contains("BITNOB_CLIENT_SECRET"));
        }
        other => panic!("Expected ProviderError::NotConfigured, got {other:?}"),
    }
}

// 3. sandbox does not fallback to mock
#[tokio::test]
async fn test_3_sandbox_does_not_fallback_to_mock() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_HTTP_500_SERVER_ERROR, Ordering::SeqCst);

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    );

    let result = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(result.is_err(), "Sandbox failure must not return mock rate");
    assert!(matches!(result.unwrap_err(), ProviderError::Unavailable(_)));
}

// 4. invalid sandbox provider response fails
#[tokio::test]
async fn test_4_invalid_sandbox_provider_response_fails() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_HTTP_401_UNAUTHORIZED, Ordering::SeqCst);

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    );

    let result = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("401"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

// 5. malformed provider JSON fails safely
#[tokio::test]
async fn test_5_malformed_provider_json_fails_safely() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_MALFORMED_JSON, Ordering::SeqCst);

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    );

    let result = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Internal(msg) => {
            assert!(msg.contains("Failed to parse Bitnob quote"));
        }
        other => panic!("Expected ProviderError::Internal, got {other:?}"),
    }
}

// 6. zero/negative/missing rate is rejected
#[tokio::test]
async fn test_6_zero_negative_missing_rate_is_rejected() {
    let (base_url, mode) = start_mock_server().await;

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url.clone()),
    );

    // Zero rate rejected
    mode.store(MODE_ZERO_RATE, Ordering::SeqCst);
    let zero_res = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(zero_res.is_err(), "Zero rate must be rejected");

    // Negative rate rejected
    mode.store(MODE_NEGATIVE_RATE, Ordering::SeqCst);
    let neg_res = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(neg_res.is_err(), "Negative rate must be rejected");

    // Missing rate rejected
    mode.store(MODE_MISSING_RATE, Ordering::SeqCst);
    let missing_res = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(missing_res.is_err(), "Missing rate must be rejected");

    // Status: false rejected
    mode.store(MODE_STATUS_FALSE, Ordering::SeqCst);
    let status_false_res = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(status_false_res.is_err(), "status=false must be rejected");
}

// 7. successful sandbox response maps correctly to HanbovaRate
#[tokio::test]
async fn test_7_successful_sandbox_response_maps_correctly() {
    let (base_url, mode) = start_mock_server().await;

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url.clone()),
    );

    // Numeric rate
    mode.store(MODE_SUCCESS_NUMERIC, Ordering::SeqCst);
    let rate1 = provider
        .get_rate("NG", "USDT", "NGN")
        .await
        .expect("numeric rate");
    assert_eq!(rate1.market, "NG");
    assert_eq!(rate1.settlement_asset, "USDT");
    assert_eq!(rate1.quote, "NGN");
    assert_eq!(rate1.rate, 1620.50);

    // String rate
    mode.store(MODE_SUCCESS_STRING, Ordering::SeqCst);
    let rate2 = provider
        .get_rate("NG", "USDT", "NGN")
        .await
        .expect("string rate");
    assert_eq!(rate2.rate, 1625.75);

    // Nested under quote
    mode.store(MODE_SUCCESS_NESTED_QUOTE, Ordering::SeqCst);
    let rate3 = provider
        .get_rate("NG", "USDT", "NGN")
        .await
        .expect("nested rate");
    assert_eq!(rate3.rate, 1630.00);
}

// 8. sandbox successful response has: environment=sandbox, provider=bitnob, is_live=false
#[tokio::test]
async fn test_8_sandbox_successful_response_metadata_and_not_live() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_SUCCESS_NUMERIC, Ordering::SeqCst);

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    );

    let rate = provider
        .get_rate("NG", "USDT", "NGN")
        .await
        .expect("sandbox rate");

    assert_eq!(rate.environment, "sandbox");
    assert_eq!(rate.provider, "bitnob");
    assert!(
        !rate.is_live,
        "Sandbox rate MUST strictly have is_live = false"
    );
    assert!(!rate.is_stale);
}

// 9. provider secrets never appear in serialized HanbovaRate/errors
#[tokio::test]
async fn test_9_provider_secrets_never_appear_in_serialized_rate_or_errors() {
    let secret = "super-secret-key-that-must-never-leak-12345";
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_SUCCESS_NUMERIC, Ordering::SeqCst);

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some(secret.to_string()),
        ProviderMode::Sandbox,
        Some(base_url.clone()),
    );

    // 1. Serialized HanbovaRate must not contain secret
    let rate = provider.get_rate("NG", "USDT", "NGN").await.unwrap();
    let serialized_rate = serde_json::to_string(&rate).unwrap();
    assert!(
        !serialized_rate.contains(secret),
        "Secret must never leak in serialized rate"
    );

    // 2. Error message must not contain secret
    mode.store(MODE_HTTP_401_UNAUTHORIZED, Ordering::SeqCst);
    let err = provider.get_rate("NG", "USDT", "NGN").await.unwrap_err();
    let err_str = err.to_string();
    assert!(
        !err_str.contains(secret),
        "Secret must never leak in error string"
    );
}

// 10. stale provider-derived cache works
#[tokio::test]
async fn test_10_stale_provider_derived_cache_works() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_SUCCESS_NUMERIC, Ordering::SeqCst);

    let provider = Arc::new(BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    ));

    // Fast cache windows for testing: 50ms fresh TTL, 10s stale TTL
    let service =
        HanbovaRateService::with_ttls(provider, Duration::from_millis(50), Duration::from_secs(10));

    // 1. First fetch succeeds from provider
    let initial_rate = service
        .get_rate("NG", "USDT", "NGN")
        .await
        .expect("initial rate");
    assert_eq!(initial_rate.rate, 1620.50);
    assert!(!initial_rate.is_stale);

    // 2. Provider now fails
    mode.store(MODE_HTTP_500_SERVER_ERROR, Ordering::SeqCst);

    // 3. Sleep beyond fresh TTL but within stale TTL
    tokio::time::sleep(Duration::from_millis(80)).await;

    // 4. Next fetch returns stale provider-derived rate
    let stale_rate = service
        .get_rate("NG", "USDT", "NGN")
        .await
        .expect("stale rate");
    assert_eq!(stale_rate.rate, 1620.50);
    assert!(stale_rate.is_stale, "Must be marked stale");
    assert_eq!(stale_rate.provider, "bitnob");
    assert_eq!(stale_rate.environment, "sandbox");
}

// 11. expired cache returns unavailable
#[tokio::test]
async fn test_11_expired_cache_returns_unavailable() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_SUCCESS_NUMERIC, Ordering::SeqCst);

    let provider = Arc::new(BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    ));

    // Extremely short stale TTL: 25ms fresh, 50ms stale
    let service = HanbovaRateService::with_ttls(
        provider,
        Duration::from_millis(25),
        Duration::from_millis(50),
    );

    // Initial fetch succeeds
    let initial = service.get_rate("NG", "USDT", "NGN").await.unwrap();
    assert_eq!(initial.rate, 1620.50);

    // Provider fails
    mode.store(MODE_HTTP_500_SERVER_ERROR, Ordering::SeqCst);

    // Sleep past stale TTL (100ms > 50ms)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Cache is expired, provider is down -> Unavailable error (HTTP 503)
    let result = service.get_rate("NG", "USDT", "NGN").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ProviderError::Unavailable(_)));
}

// 12. production behavior is not accidentally enabled
#[tokio::test]
async fn test_12_production_behavior_not_accidentally_enabled_in_sandbox() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_SUCCESS_NUMERIC, Ordering::SeqCst);

    let sandbox_provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url.clone()),
    );

    let prod_provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Production,
        Some(base_url),
    );

    let sandbox_rate = sandbox_provider
        .get_rate("NG", "USDT", "NGN")
        .await
        .unwrap();
    let prod_rate = prod_provider.get_rate("NG", "USDT", "NGN").await.unwrap();

    // Sandbox MUST be is_live = false
    assert!(!sandbox_rate.is_live);
    assert_eq!(sandbox_rate.environment, "sandbox");

    // Production provider with valid response sets is_live = true
    assert!(prod_rate.is_live);
    assert_eq!(prod_rate.environment, "production");
}

// Opt-in real Bitnob sandbox test: only executed when explicitly triggered and credentials exist
#[tokio::test]
#[ignore]
async fn test_real_bitnob_sandbox_connectivity() {
    let client_id = std::env::var("BITNOB_CLIENT_ID").ok();
    let client_secret = std::env::var("BITNOB_CLIENT_SECRET").ok();

    if client_id.as_deref().unwrap_or("").trim().is_empty()
        || client_secret.as_deref().unwrap_or("").trim().is_empty()
    {
        eprintln!("[SKIP] BITNOB_CLIENT_ID or BITNOB_CLIENT_SECRET not set; skipping live call");
        return;
    }

    let provider = BitnobRateProvider::with_config(
        client_id,
        client_secret,
        ProviderMode::Sandbox,
        None, // Uses official sandbox base URL: https://sandboxapi.bitnob.co
    );

    println!("[INFO] Attempting real Bitnob sandbox quote request: USDT -> NGN...");
    let result = provider.get_rate("NG", "USDT", "NGN").await;

    match result {
        Ok(rate) => {
            println!("\n====================================================");
            println!("REAL BITNOB SANDBOX RESPONSE RECEIVED");
            println!("Provider: {}", rate.provider);
            println!("Environment: {}", rate.environment);
            println!("Market: {}", rate.market);
            println!("Pair: {} -> {}", rate.settlement_asset, rate.quote);
            println!("Rate: {}", rate.rate);
            println!("Display: {}", rate.display);
            println!("is_live: {}", rate.is_live);
            println!("is_stale: {}", rate.is_stale);
            println!("====================================================\n");
            assert_eq!(rate.provider, "bitnob");
            assert_eq!(rate.environment, "sandbox");
            assert!(!rate.is_live, "Sandbox rate must NEVER be live");
            assert!(!rate.is_stale);
            assert!(rate.rate > 0.0);
        }
        Err(err) => {
            println!("\n====================================================");
            println!("FAILED TO CONNECT");
            println!("Error: {err}");
            println!("====================================================\n");
            panic!("Live Bitnob Sandbox request failed: {err}");
        }
    }
}
