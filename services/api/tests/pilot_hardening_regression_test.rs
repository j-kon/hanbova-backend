use hanbova_api::{
    config::{AppConfig, ProviderMode},
    handlers::health::{health_check, readiness_check},
    providers::{
        bitnob::BitnobAdapter, dtone::DtOneAdapter, rates::BitnobRateProvider, BillQuoteRequest,
        BillServiceType, CapabilityStatus, CardProvider, CreatePayoutRequest,
        DigitalServicesProvider, EsimProvider, PayoutProvider, PlatformRateProvider, ProviderError,
        PurchaseEsimRequest,
    },
    state::{AppState, StateError},
};

#[tokio::test]
async fn test_sandbox_bitnob_payout_execution_fails_closed() {
    let adapter = BitnobAdapter::with_mode(ProviderMode::Sandbox);
    let req = CreatePayoutRequest {
        quote_id: "quote_123".to_string(),
        recipient_name: "Alice Smith".to_string(),
        recipient_account: "0712345678".to_string(),
        reference: Some("Rent".to_string()),
    };

    let result = adapter.create_payout(&req).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("payout execution is not live in this milestone"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_sandbox_bitnob_payout_status_fails_closed() {
    let adapter = BitnobAdapter::with_mode(ProviderMode::Sandbox);
    let result = adapter.get_payout_status("tx_123").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("payout status is not live in this milestone"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_sandbox_bitnob_card_status_fails_closed() {
    let adapter = BitnobAdapter::with_mode(ProviderMode::Sandbox);
    let result = adapter.get_card_status("card_123").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("card status is not live in this milestone"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_sandbox_bitnob_card_eligibility_fails_closed() {
    let adapter = BitnobAdapter::with_mode(ProviderMode::Sandbox);
    let result = adapter.check_card_eligibility("KE").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("Bitnob card eligibility is not live in this milestone"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_sandbox_dtone_customer_validation_fails_closed() {
    let adapter = DtOneAdapter::with_mode(ProviderMode::Sandbox);
    let result = adapter.validate_customer("kplc", "14123456789").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(
                msg.contains("DT One sandbox customer validation is not live in this milestone")
            );
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_sandbox_dtone_bill_quote_fails_closed() {
    let adapter = DtOneAdapter::with_mode(ProviderMode::Sandbox);
    let req = BillQuoteRequest {
        biller_id: "kplc".to_string(),
        product_id: None,
        amount_fiat: 1000.0,
        customer_account: "14123456789".to_string(),
    };

    let result = adapter.get_bill_quote(&req).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("DT One sandbox bill quotes are not live in this milestone"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_sandbox_dtone_bill_status_fails_closed() {
    let adapter = DtOneAdapter::with_mode(ProviderMode::Sandbox);
    let result = adapter.get_bill_status("tx_123").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("DT One sandbox bill status lookup is not live in this milestone"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_sandbox_esim_status_and_purchase_fails_closed() {
    let adapter = DtOneAdapter::with_mode(ProviderMode::Sandbox);
    let status_res = adapter.get_esim_status("esim_123").await;
    assert!(status_res.is_err());
    match status_res.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("DT One sandbox eSIM status is not live in this milestone"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }

    let purchase_res = adapter
        .purchase_esim(&PurchaseEsimRequest {
            package_id: "pkg_1".to_string(),
            user_email: None,
        })
        .await;
    assert!(purchase_res.is_err());
    match purchase_res.unwrap_err() {
        ProviderError::Unavailable(msg) => {
            assert!(msg.contains("eSIM purchase is not live in this milestone"));
        }
        other => panic!("Expected ProviderError::Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_mock_mode_deterministic_fixtures() {
    let bitnob = BitnobAdapter::with_mode(ProviderMode::Mock);
    let corridors = bitnob.get_supported_corridors(Some("KE")).await.unwrap();
    assert!(!corridors.is_empty());

    let dtone = DtOneAdapter::with_mode(ProviderMode::Mock);
    let services = dtone.get_supported_services("KE").await.unwrap();
    assert!(services.contains(&BillServiceType::Airtime));
}

#[tokio::test]
async fn test_pilot_capabilities_show_disabled_for_unimplemented_providers() {
    let config = AppConfig::from_iter([
        ("HANBOVA_ENV", "pilot"),
        ("HANBOVA_API_HOST", "0.0.0.0"),
        ("HANBOVA_API_PORT", "8080"),
        ("PROVIDER_MODE", "sandbox"),
        ("LIGHTNING_ENABLED", "false"),
        (
            "DATABASE_URL",
            "postgres://pilot:pilot@127.0.0.1:5432/hanbova_pilot",
        ),
        ("MINT_URL", "https://mint.hanbova.test"),
        ("JWT_SECRET", "pilot-secret-key-at-least-32-chars-long"),
        ("CORS_ALLOWED_ORIGINS", "https://pilot.hanbova.test"),
    ])
    .unwrap();

    let state = AppState::new(config, None);
    let caps = state.capabilities();

    // In current M3B.3A with no credentials configured, bitnob_rates is disabled
    assert_eq!(caps.bitnob_rates, CapabilityStatus::Disabled);
    assert_eq!(caps.bitnob_wallet, CapabilityStatus::Disabled);
    assert_eq!(caps.bitnob_payouts, CapabilityStatus::Disabled);
    assert_eq!(caps.dtone_bills, CapabilityStatus::Disabled);
    assert_eq!(caps.lightning, CapabilityStatus::Disabled);
    assert_eq!(caps.protected_send, CapabilityStatus::Test);
}

#[tokio::test]
async fn test_pilot_bitnob_rates_only_sandbox_if_valid_credentials() {
    let unconfigured_provider =
        BitnobRateProvider::with_config(None, None, ProviderMode::Sandbox, None);
    assert!(!unconfigured_provider.is_configured());

    let configured_provider = BitnobRateProvider::with_config(
        Some("client_id_123".to_string()),
        Some("client_secret_456".to_string()),
        ProviderMode::Sandbox,
        None,
    );
    assert!(configured_provider.is_configured());
}

#[tokio::test]
async fn test_bitnob_rate_provider_cannot_downgrade_sandbox_to_mock() {
    let provider = BitnobRateProvider::with_config(None, None, ProviderMode::Sandbox, None);
    let result = provider.get_rate("NG", "USDT", "NGN").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::NotConfigured(msg) => {
            assert!(msg.contains("BITNOB_CLIENT_ID"));
        }
        other => panic!("Expected ProviderError::NotConfigured, got {other:?}"),
    }
}

#[tokio::test]
async fn test_pilot_lightning_mock_hole_is_strictly_closed() {
    let invalid_pilot_res = AppConfig::from_iter([
        ("HANBOVA_ENV", "pilot"),
        ("HANBOVA_API_HOST", "0.0.0.0"),
        ("HANBOVA_API_PORT", "8080"),
        ("PROVIDER_MODE", "sandbox"),
        ("LIGHTNING_ENABLED", "true"),
        (
            "DATABASE_URL",
            "postgres://pilot:pilot@127.0.0.1:5432/hanbova_pilot",
        ),
        ("MINT_URL", "https://mint.hanbova.test"),
        ("JWT_SECRET", "pilot-secret-key-at-least-32-chars-long"),
        ("CORS_ALLOWED_ORIGINS", "https://pilot.hanbova.test"),
    ]);

    assert!(invalid_pilot_res.is_err());
    let err_msg = invalid_pilot_res.unwrap_err().problems;
    assert!(
        err_msg.contains(
            "Lightning cannot be enabled in pilot until a non-mock provider is configured"
        ),
        "Expected lightning pilot guard error, got: {err_msg}"
    );

    let valid_pilot_config = AppConfig::from_iter([
        ("HANBOVA_ENV", "pilot"),
        ("HANBOVA_API_HOST", "0.0.0.0"),
        ("HANBOVA_API_PORT", "8080"),
        ("PROVIDER_MODE", "sandbox"),
        ("LIGHTNING_ENABLED", "false"),
        (
            "DATABASE_URL",
            "postgres://pilot:pilot@127.0.0.1:5432/hanbova_pilot",
        ),
        ("MINT_URL", "https://mint.hanbova.test"),
        ("JWT_SECRET", "pilot-secret-key-at-least-32-chars-long"),
        ("CORS_ALLOWED_ORIGINS", "https://pilot.hanbova.test"),
    ])
    .unwrap();

    let state = AppState::new(valid_pilot_config.clone(), None);
    assert_eq!(state.capabilities().lightning, CapabilityStatus::Disabled);

    // Also verify AppState::try_new guard
    let mut forced_config = valid_pilot_config;
    forced_config.lightning_enabled = true;
    let try_new_res = AppState::try_new(forced_config, None);
    assert!(matches!(
        try_new_res,
        Err(StateError::LightningMockForbiddenInPilot) | Err(StateError::MissingDatabase)
    ));
}

#[tokio::test]
async fn test_public_health_and_readiness_responses_do_not_leak_raw_db_errors() {
    let config = AppConfig::from_iter([
        ("HANBOVA_ENV", "development"),
        ("PROVIDER_MODE", "mock"),
        ("DATABASE_URL", "postgres://invalid:5432/db"),
    ])
    .unwrap();

    let state = AppState::new(config, None);
    let (health_status, health_body) = health_check(axum::extract::State(state.clone())).await;
    assert_eq!(health_status, axum::http::StatusCode::OK);
    assert_eq!(health_body.database, "in_memory");
    assert!(!health_body.database.contains("error"));
    assert!(!health_body.database.contains("Connection refused"));

    let (readiness_status, readiness_body) = readiness_check(axum::extract::State(state)).await;
    assert_eq!(readiness_status, axum::http::StatusCode::OK);
    assert_eq!(readiness_body.database, "in_memory");
    assert!(!readiness_body.database.contains("error"));
}
