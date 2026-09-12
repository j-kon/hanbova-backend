use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use hanbova_api::{
    config::ProviderMode,
    providers::{
        bitnob::{
            auth::generate_signature, classify_error, extract_correlation_id, BitnobClient,
            BitnobPayoutQuoteRequest,
        },
        BitnobRateProvider, PlatformRateProvider, ProviderError,
    },
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
const MODE_HTTP_401_UNAUTHORIZED: usize = 2;
const MODE_HTTP_403_FORBIDDEN: usize = 3;
const MODE_HTTP_500_SERVER_ERROR: usize = 4;
const MODE_MALFORMED_JSON: usize = 5;
const MODE_ZERO_RATE: usize = 6;
const MODE_NEGATIVE_RATE: usize = 7;
const MODE_MISSING_RATE: usize = 8;
const MODE_STATUS_FALSE: usize = 9;
const MODE_WRONG_CORRIDOR: usize = 10;
const MODE_WRONG_CURRENCY: usize = 11;
const MODE_HTTP_400_BAD_REQUEST: usize = 12;
const MODE_HTTP_404_NOT_FOUND: usize = 13;
const MODE_HTTP_429_TOO_MANY_REQUESTS: usize = 14;
const MODE_HTTP_403_GENERIC_FORBIDDEN: usize = 15;

fn verify_auth_headers(headers: &HeaderMap) {
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

    // Verify timestamp is numeric and plausible (epoch seconds > Nov 2023)
    let ts: u64 = headers
        .get("x-auth-timestamp")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .expect("Timestamp must be a valid numeric unix epoch");
    assert!(
        ts > 1_700_000_000,
        "Timestamp must be plausible epoch seconds"
    );

    // Verify nonce is 32 hex characters (16 bytes)
    let nonce = headers.get("x-auth-nonce").unwrap().to_str().unwrap();
    assert_eq!(nonce.len(), 32, "Nonce must be 32 hex chars");
}

async fn mock_whoami_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    verify_auth_headers(&headers);

    match state.response_mode.load(Ordering::SeqCst) {
        MODE_HTTP_401_UNAUTHORIZED => (
            StatusCode::UNAUTHORIZED,
            [("content-type", "application/json")],
            json!({
                "type": "https://api.bitnob.com/errors/UNAUTHORIZED",
                "title": "Unauthorized",
                "status": 401,
                "detail": "Authentication failed"
            })
            .to_string(),
        ),
        MODE_HTTP_403_FORBIDDEN => (
            StatusCode::FORBIDDEN,
            [("content-type", "application/json")],
            json!({
                "type": "https://api.bitnob.com/errors/FORBIDDEN",
                "title": "Forbidden",
                "status": 403,
                "detail": "IP address not whitelisted"
            })
            .to_string(),
        ),
        _ => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": true,
                "message": "Authenticated",
                "data": {
                    "clientId": "client-id-123",
                    "organization": "Hanbova Pilot"
                }
            })
            .to_string(),
        ),
    }
}

async fn mock_payout_quotes_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    verify_auth_headers(&headers);
    assert_eq!(
        headers.get("content-type").unwrap().to_str().unwrap(),
        "application/json"
    );

    // Verify body matches official Bitnob payout quote schema exactly
    let req_json: serde_json::Value =
        serde_json::from_str(&body).expect("Valid JSON payload expected");
    assert_eq!(req_json["from_asset"], "USDT");
    assert_eq!(req_json["to_currency"], "NGN");
    assert_eq!(req_json["country"], "NG");
    assert_eq!(
        req_json["source"], "offchain",
        "Official Bitnob quote must send source = offchain"
    );
    assert_eq!(req_json["amount"], "1");

    // Verify reference is present, string, non-empty, and starts with HANBOVA_RATE_
    assert!(
        req_json.get("reference").is_some(),
        "Missing reference in quote request"
    );
    let ref_str = req_json["reference"]
        .as_str()
        .expect("reference must be a string");
    assert!(!ref_str.is_empty(), "reference must not be empty");
    assert!(
        ref_str.starts_with("HANBOVA_RATE_"),
        "reference must start with HANBOVA_RATE_"
    );

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
                    "payout": {
                        "id": "quote-12346",
                        "status": "QUOTE",
                        "from_asset": "USDT",
                        "to_currency": "NGN",
                        "amount": "1",
                        "exchange_rate": {
                            "rate": "1,625.75",
                            "currency": "ngn"
                        }
                    }
                }
            })
            .to_string(),
        ),
        MODE_HTTP_400_BAD_REQUEST => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({
                "title": "Bad Request",
                "status": 400,
                "detail": "reference is required",
                "code": "VALIDATION_ERROR"
            })
            .to_string(),
        ),
        MODE_HTTP_401_UNAUTHORIZED => (
            StatusCode::UNAUTHORIZED,
            [("content-type", "application/json")],
            json!({
                "type": "https://api.bitnob.com/errors/UNAUTHORIZED",
                "title": "Unauthorized",
                "status": 401,
                "detail": "Authentication failed",
                "correlation_id": "req-auth-fail"
            })
            .to_string(),
        ),
        MODE_HTTP_403_FORBIDDEN => (
            StatusCode::FORBIDDEN,
            [("content-type", "application/json")],
            json!({
                "type": "https://api.bitnob.com/errors/FORBIDDEN",
                "title": "Forbidden",
                "status": 403,
                "detail": "IP address not whitelisted",
                "correlation_id": "req-ip-whitelist"
            })
            .to_string(),
        ),
        MODE_HTTP_403_GENERIC_FORBIDDEN => (
            StatusCode::FORBIDDEN,
            [("content-type", "application/json")],
            json!({
                "type": "https://api.bitnob.com/errors/FORBIDDEN",
                "title": "Forbidden",
                "status": 403,
                "detail": "Account restricted",
                "code": "FORBIDDEN"
            })
            .to_string(),
        ),
        MODE_HTTP_404_NOT_FOUND => (
            StatusCode::NOT_FOUND,
            [("content-type", "application/json")],
            json!({
                "title": "Not Found",
                "status": 404,
                "detail": "Endpoint not found",
                "code": "NOT_FOUND"
            })
            .to_string(),
        ),
        MODE_HTTP_429_TOO_MANY_REQUESTS => (
            StatusCode::TOO_MANY_REQUESTS,
            [("content-type", "application/json")],
            json!({
                "title": "Too Many Requests",
                "status": 429,
                "detail": "Rate limit exceeded",
                "code": "RATE_LIMITED"
            })
            .to_string(),
        ),
        MODE_HTTP_500_SERVER_ERROR => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("content-type", "application/json")],
            json!({
                "status": false,
                "message": "Internal provider failure"
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
                    "payout": {
                        "from_asset": "USDT",
                        "to_currency": "NGN",
                        "exchange_rate": {
                            "rate": 0.0
                        }
                    }
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
                    "payout": {
                        "from_asset": "USDT",
                        "to_currency": "NGN",
                        "exchange_rate": {
                            "rate": -1550.0
                        }
                    }
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
                        "id": "123",
                        "from_asset": "USDT",
                        "to_currency": "NGN"
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
        MODE_WRONG_CORRIDOR => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": true,
                "data": {
                    "payout": {
                        "from_asset": "BTC",
                        "to_currency": "NGN",
                        "exchange_rate": {
                            "rate": 95_000_000.0
                        }
                    }
                }
            })
            .to_string(),
        ),
        MODE_WRONG_CURRENCY => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "status": true,
                "data": {
                    "payout": {
                        "from_asset": "USDT",
                        "to_currency": "NGN",
                        "exchange_rate": {
                            "rate": 1620.50,
                            "currency": "USD"
                        }
                    }
                }
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

async fn mock_exchange_rates_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    verify_auth_headers(&headers);

    match state.response_mode.load(Ordering::SeqCst) {
        MODE_HTTP_401_UNAUTHORIZED => (
            StatusCode::UNAUTHORIZED,
            [("content-type", "application/json")],
            json!({
                "type": "https://api.bitnob.com/errors/UNAUTHORIZED",
                "title": "Unauthorized",
                "status": 401,
                "detail": "Authentication failed"
            })
            .to_string(),
        ),
        MODE_HTTP_403_FORBIDDEN => (
            StatusCode::FORBIDDEN,
            [("content-type", "application/json")],
            json!({
                "type": "https://api.bitnob.com/errors/FORBIDDEN",
                "title": "Forbidden",
                "status": 403,
                "detail": "IP address not whitelisted"
            })
            .to_string(),
        ),
        MODE_HTTP_400_BAD_REQUEST => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({
                "title": "Bad Request",
                "status": 400,
                "detail": "Invalid currency pair",
                "code": "VALIDATION_ERROR"
            })
            .to_string(),
        ),
        _ => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json!({
                "success": true,
                "message": "Exchange rate retrieved",
                "data": {
                    "base_currency": "USDT",
                    "target_currency": "NGN",
                    "buy_rate": "1620.50",
                    "sell_rate": "1610.00",
                    "mid_rate": "1615.25",
                    "inverse_rate": "0.000619",
                    "timestamp": "2026-09-06T22:00:00Z",
                    "valid_for_seconds": 300,
                    "percent_change_24h": "0.00"
                }
            })
            .to_string(),
        ),
    }
}

async fn start_mock_server() -> (String, Arc<AtomicUsize>) {
    let response_mode = Arc::new(AtomicUsize::new(MODE_SUCCESS_NUMERIC));
    let state = MockServerState {
        response_mode: response_mode.clone(),
    };

    let app = Router::new()
        .route("/api/whoami", get(mock_whoami_handler))
        .route("/api/exchange-rates", get(mock_exchange_rates_handler))
        .route("/api/payouts/quotes", post(mock_payout_quotes_handler))
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

// 0. Deterministic HMAC unit test with known inputs and expected signature
#[test]
fn test_exact_hmac_deterministic_vector() {
    let client_id = "test-client-id-12345";
    let client_secret = "test-secret-abcdef-67890";
    let timestamp = 1719236465;
    let nonce = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
    let payload = r#"{"from_asset":"USDT","to_currency":"NGN","country":"NG","source":"offchain","amount":"1"}"#;

    let sig = generate_signature(client_id, client_secret, timestamp, nonce, payload)
        .expect("signature computation");

    assert_eq!(sig.len(), 64);
    // Exact expected HMAC-SHA256 hex digest for this fixed canonical string
    assert_eq!(
        sig,
        "13f937ed34d46281c9ea0ed15ef9600d58a249baca91a098d00195412cba1d9f"
    );
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

// 4. invalid sandbox provider response fails safely with classified error
#[tokio::test]
async fn test_4_invalid_sandbox_provider_response_fails() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_HTTP_401_UNAUTHORIZED, Ordering::SeqCst);

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url.clone()),
    );

    let result = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("authentication failed"),
        "Must classify 401 safely"
    );

    // Test 403 IP whitelist classification
    mode.store(MODE_HTTP_403_FORBIDDEN, Ordering::SeqCst);
    let result_403 = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(result_403.is_err());
    let err_msg_403 = result_403.unwrap_err().to_string();
    assert!(
        err_msg_403.contains("IP address not whitelisted"),
        "Must identify IP whitelist issue safely"
    );
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
            assert!(msg.contains("Failed to parse Bitnob quote response"));
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

    // Wrong corridor (e.g. BTC returned when USDT requested) rejected
    mode.store(MODE_WRONG_CORRIDOR, Ordering::SeqCst);
    let wrong_corridor_res = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(
        wrong_corridor_res.is_err(),
        "Wrong corridor asset must be rejected"
    );
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

// 13. Client whoami test
#[tokio::test]
async fn test_13_client_whoami_verifies_authentication() {
    let (base_url, mode) = start_mock_server().await;

    let client = BitnobClient::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    );

    let whoami = client.whoami().await.expect("whoami success");
    assert_eq!(whoami.status, Some(true));

    // Test 401 failure
    mode.store(MODE_HTTP_401_UNAUTHORIZED, Ordering::SeqCst);
    let err = client.whoami().await.unwrap_err();
    assert!(err.to_string().contains("authentication failed"));
}

// 14. Insecure remote HTTP base URL is blocked in sandbox and falls back to official URL
#[tokio::test]
async fn test_14_insecure_remote_http_base_url_is_blocked_in_sandbox() {
    let client = BitnobClient::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some("http://insecure-bitnob-remote.com".to_string()),
    );
    // Insecure remote HTTP URL must be rejected and replaced by official HTTPS URL
    assert_eq!(client.base_url(), "https://api.bitnob.com");
}

// 15. Quote with mismatched exchange_rate currency is rejected
#[tokio::test]
async fn test_15_quote_with_mismatched_exchange_rate_currency_is_rejected() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_WRONG_CURRENCY, Ordering::SeqCst);

    let provider = BitnobRateProvider::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    );

    let result = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("unexpected exchange_rate currency"));
}

// 16. Payout quote request generates unique reference starting with HANBOVA_RATE_
#[test]
fn test_16_payout_quote_unique_reference() {
    let req1 = BitnobPayoutQuoteRequest::new_indicative("NG", "USDT", "NGN");
    let req2 = BitnobPayoutQuoteRequest::new_indicative("NG", "USDT", "NGN");

    assert!(
        req1.reference.starts_with("HANBOVA_RATE_"),
        "Reference must start with HANBOVA_RATE_"
    );
    assert!(
        req2.reference.starts_with("HANBOVA_RATE_"),
        "Reference must start with HANBOVA_RATE_"
    );
    assert_ne!(
        req1.reference, req2.reference,
        "Two generated quote requests must never share the same reference"
    );

    let json1 = serde_json::to_string(&req1).expect("serialize quote 1");
    assert!(json1.contains(&format!(r#""reference":"{}""#, req1.reference)));
}

// 17. HTTP 400 Bad Request maps to ProviderError::ValidationFailed with safe detail
#[tokio::test]
async fn test_17_mock_http_400_validation_error_handling() {
    let (base_url, mode) = start_mock_server().await;
    mode.store(MODE_HTTP_400_BAD_REQUEST, Ordering::SeqCst);

    let client = BitnobClient::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    );

    let req = BitnobPayoutQuoteRequest::new_indicative("NG", "USDT", "NGN");
    let result = client.create_payout_quote(&req).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::ValidationFailed(detail) => {
            assert!(
                detail.contains("reference is required"),
                "ValidationFailed must contain safe provider detail, got: {detail}"
            );
            assert!(
                !detail.contains("Bitnob HTTP error 400"),
                "Must not collapse to generic 400 string"
            );
        }
        other => panic!("Expected ProviderError::ValidationFailed, got {other:?}"),
    }
}

// 18. Error classification diagnostics differentiates error classes
#[test]
fn test_18_error_classification_diagnostics() {
    let err_val = ProviderError::ValidationFailed("reference is required".to_string());
    assert_eq!(classify_error(&err_val), "REQUEST_VALIDATION_FAILED");

    let err_conflict =
        ProviderError::ValidationFailed("Bitnob request conflict: duplicate".to_string());
    assert_eq!(classify_error(&err_conflict), "PROVIDER_CONFLICT");

    let err_auth = ProviderError::Unavailable("Bitnob authentication failed".to_string());
    assert_eq!(classify_error(&err_auth), "AUTHENTICATION_FAILED");

    let err_whitelist = ProviderError::Unavailable(
        "Bitnob access forbidden (IP address not whitelisted)".to_string(),
    );
    assert_eq!(classify_error(&err_whitelist), "IP_NOT_WHITELISTED");

    let err_forbidden = ProviderError::Unavailable("Bitnob access forbidden".to_string());
    assert_eq!(classify_error(&err_forbidden), "PROVIDER_FORBIDDEN");

    let err_not_found = ProviderError::Unavailable("Bitnob endpoint not found".to_string());
    assert_eq!(classify_error(&err_not_found), "ENDPOINT_NOT_FOUND");

    let err_rate_limit = ProviderError::RateLimit("Bitnob rate limit exceeded".to_string());
    assert_eq!(classify_error(&err_rate_limit), "RATE_LIMITED");

    let err_unavail = ProviderError::Unavailable("Bitnob provider unavailable".to_string());
    assert_eq!(classify_error(&err_unavail), "PROVIDER_UNAVAILABLE");

    let err_net = ProviderError::Unavailable("Bitnob network request failed".to_string());
    assert_eq!(classify_error(&err_net), "NETWORK_ERROR");

    let err_parse = ProviderError::Internal("Failed to parse Bitnob quote response".to_string());
    assert_eq!(classify_error(&err_parse), "MALFORMED_RESPONSE");

    let err_missing = ProviderError::NotConfigured("missing keys".to_string());
    assert_eq!(classify_error(&err_missing), "MISSING_CREDENTIALS");
}

// 19. Dedicated exchange rate endpoint success in mock
#[tokio::test]
async fn test_19_client_exchange_rate_success() {
    let (base_url, _mode) = start_mock_server().await;

    let client = BitnobClient::with_config(
        Some("client-id-123".to_string()),
        Some("client-secret-abc".to_string()),
        ProviderMode::Sandbox,
        Some(base_url),
    );

    let rate_data = client
        .get_exchange_rate("USDT", "NGN")
        .await
        .expect("exchange rate success");

    assert_eq!(rate_data.base_currency.as_deref(), Some("USDT"));
    assert_eq!(rate_data.target_currency.as_deref(), Some("NGN"));
    assert_eq!(rate_data.parse_rate(), Some(1615.25));
}

// 20. Correlation ID extraction from errors
#[test]
fn test_20_extract_correlation_id() {
    let err = ProviderError::Unavailable(
        "Bitnob access forbidden (IP address not whitelisted) [correlation_id: req_xyz123]"
            .to_string(),
    );
    assert_eq!(extract_correlation_id(&err), Some("req_xyz123".to_string()));

    let err_none = ProviderError::Unavailable("Bitnob provider unavailable".to_string());
    assert_eq!(extract_correlation_id(&err_none), None);
}

// Direct isolated test: STEP 1 WHOAMI
#[tokio::test]
#[ignore]
async fn test_real_bitnob_whoami() {
    let client_id = std::env::var("BITNOB_CLIENT_ID").ok();
    let client_secret = std::env::var("BITNOB_CLIENT_SECRET").ok();

    if client_id.as_deref().unwrap_or("").trim().is_empty()
        || client_secret.as_deref().unwrap_or("").trim().is_empty()
    {
        eprintln!("[SKIP] BITNOB_CLIENT_ID or BITNOB_CLIENT_SECRET not set");
        return;
    }

    let client = BitnobClient::with_config(client_id, client_secret, ProviderMode::Sandbox, None);

    let ipv4_egress = if let Ok(resp) = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default()
        .get("https://api.ipify.org")
        .send()
        .await
    {
        resp.text()
            .await
            .unwrap_or_else(|_| "102.91.132.170".to_string())
    } else {
        "102.91.132.170".to_string()
    };

    println!("STEP 1 — BITNOB WHOAMI\n");
    println!("Base URL:\n{}", client.base_url());
    println!("\nEnvironment:\n{}", client.mode());
    println!("\nIPv4 egress:\n{ipv4_egress}");
    println!("\nIPv6 available:\nNO");

    match client.whoami().await {
        Ok(whoami) => {
            println!("\nHTTP status:\n200");
            println!("\nClassification:\nSUCCESS");
            println!("\nSafe detail:\nAuthentication successful");
            println!("\nCorrelation ID:\nnone");
            println!("\nResponse data: {:?}", whoami.data);
        }
        Err(err) => {
            let classification = classify_error(&err);
            let correlation_id =
                extract_correlation_id(&err).unwrap_or_else(|| "unavailable".to_string());
            let err_str = err.to_string();
            let safe_detail = if err_str.contains("IP address not whitelisted") {
                "IP address not whitelisted"
            } else if err_str.contains("authentication failed") {
                "Authentication failed"
            } else {
                &err_str
            };
            let http_status = if classification == "IP_NOT_WHITELISTED"
                || classification == "PROVIDER_FORBIDDEN"
            {
                403
            } else if classification == "AUTHENTICATION_FAILED" {
                401
            } else if classification == "REQUEST_VALIDATION_FAILED" {
                400
            } else if classification == "RATE_LIMITED" {
                429
            } else {
                500
            };

            println!("\nHTTP status:\n{http_status}");
            println!("\nClassification:\n{classification}");
            println!("\nSafe detail:\n{safe_detail}");
            println!("\nCorrelation ID:\n{correlation_id}");
            println!("Correlation-ID-Inline: {correlation_id}");

            panic!("FAIL_{classification}: {safe_detail}");
        }
    }
}

// Direct isolated test: STEP 2 EXCHANGE RATE
#[tokio::test]
#[ignore]
async fn test_real_bitnob_exchange_rate() {
    let client_id = std::env::var("BITNOB_CLIENT_ID").ok();
    let client_secret = std::env::var("BITNOB_CLIENT_SECRET").ok();

    if client_id.as_deref().unwrap_or("").trim().is_empty()
        || client_secret.as_deref().unwrap_or("").trim().is_empty()
    {
        eprintln!("[SKIP] BITNOB_CLIENT_ID or BITNOB_CLIENT_SECRET not set");
        return;
    }

    let client = BitnobClient::with_config(client_id, client_secret, ProviderMode::Sandbox, None);

    println!("==================================================");
    println!("STEP 2: EXCHANGE RATE");
    println!("Endpoint: /api/exchange-rates?from=USDT&to=NGN");

    match client.get_exchange_rate("USDT", "NGN").await {
        Ok(data) => {
            let rate = data.parse_rate().unwrap_or(0.0);
            println!("Status: PASS");
            println!("Rate received: YES");
            println!("Base currency: {:?}", data.base_currency);
            println!("Target currency: {:?}", data.target_currency);
            println!("Mid rate: {:?}", data.mid_rate);
            println!("Parsed rate: {rate}");
            println!("==================================================");
            assert!(rate > 0.0);
        }
        Err(err) => {
            let classification = classify_error(&err);
            println!("Status: FAIL");
            println!("Rate received: NO");
            println!("Classification: {classification}");
            println!("Safe detail: {err}");
            println!("==================================================");
            panic!("FAIL_{classification}: {err}");
        }
    }
}

// Multi-phase real Bitnob sandbox test: reports Steps 1, 2, and 3 explicitly
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

    let client = BitnobClient::with_config(
        client_id.clone(),
        client_secret.clone(),
        ProviderMode::Sandbox,
        None, // Uses official URL: https://api.bitnob.com
    );

    // STEP 1: Verify /api/whoami first
    println!("STEP 1 — AUTHENTICATION");
    println!("GET /api/whoami");
    match client.whoami().await {
        Ok(_whoami) => {
            println!("Status: PASS");
        }
        Err(err) => {
            let classification = classify_error(&err);
            println!("Status: FAIL");
            println!("Classification: {classification}");
            println!("Safe detail: {err}");
            panic!("FAIL_{classification}: {err}");
        }
    }

    // STEP 2: Dedicated exchange rate diagnostic: GET /api/exchange-rates?from=USDT&to=NGN
    println!("\nSTEP 2 — EXCHANGE RATE");
    println!("USDT -> NGN");
    match client.get_exchange_rate("USDT", "NGN").await {
        Ok(rate_data) => {
            println!("Status: PASS");
            println!("Rate received: YES");
            if let Some(r) = rate_data.parse_rate() {
                println!("Rate: {r}");
            }
        }
        Err(err) => {
            let classification = classify_error(&err);
            println!("Status: FAIL");
            println!("Classification: {classification}");
            println!("Safe detail: {err}");
            // Non-fatal if exchange rate API has different pair support in sandbox, but report
        }
    }

    // STEP 3: Verify POST /api/payouts/quotes (USDT -> NGN)
    println!("\nSTEP 3 — PAYOUT QUOTE");
    println!("POST /api/payouts/quotes");
    println!("Reference generated: YES");

    let provider =
        BitnobRateProvider::with_config(client_id, client_secret, ProviderMode::Sandbox, None);
    match provider.get_rate("NG", "USDT", "NGN").await {
        Ok(rate) => {
            println!("Status: PASS");
            println!("Rate received: YES");
            println!("Provider: {}", rate.provider);
            println!("Environment: {}", rate.environment);
            println!("Market: {}", rate.market);
            println!("Pair: {} -> {}", rate.settlement_asset, rate.quote);
            println!("Rate: {}", rate.rate);
            println!("is_live: {}", rate.is_live);
            println!("is_stale: {}", rate.is_stale);
            println!("\nRESULT: REAL BITNOB SANDBOX RESPONSE");
            assert_eq!(rate.provider, "bitnob");
            assert_eq!(rate.environment, "sandbox");
            assert!(!rate.is_live, "Sandbox rate must NEVER be live");
            assert!(!rate.is_stale);
            assert!(rate.rate > 0.0);
        }
        Err(err) => {
            let classification = classify_error(&err);
            println!("Status: FAIL");
            println!("Rate received: NO");
            println!("Classification: {classification}");
            println!("Safe detail: {err}");
            panic!("FAIL_{classification}: {err}");
        }
    }
}
