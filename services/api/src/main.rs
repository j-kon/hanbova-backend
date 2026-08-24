use axum::Router;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod auth;
mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod repositories;
mod routes;
mod services;
mod state;

use config::AppConfig;
use state::AppState;

pub fn build_app(state: AppState) -> Router {
    let api_v1 = routes::create_api_router();

    Router::new()
        .nest("/api/v1", api_v1)
        .layer(middleware::cors_layer())
        .layer(middleware::request_limit_layer())
        .layer(middleware::trace_layer())
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,hanbova_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();
    tracing::info!(
        "Starting Hanbova API v{} in [{}] mode",
        config.app_version,
        config.env
    );

    let pool = if let Some(db_url) = &config.database_url {
        tracing::info!("Connecting to PostgreSQL database...");
        match PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(db_url)
            .await
        {
            Ok(pool) => {
                tracing::info!("PostgreSQL connected successfully.");
                // Run migrations if database is available
                if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
                    tracing::warn!("Failed to automatically run SQL migrations: {:?}", e);
                } else {
                    tracing::info!("Database migrations applied cleanly.");
                }
                Some(pool)
            }
            Err(e) => {
                tracing::warn!(
                    "PostgreSQL connection failed ({}). Falling back to in-memory repositories.",
                    e
                );
                None
            }
        }
    } else {
        tracing::info!("No DATABASE_URL configured. Running with in-memory persistence.");
        None
    };

    let state = AppState::new(config.clone(), pool);
    let app = build_app(state);

    let addr_str = config.socket_addr();
    let listener = tokio::net::TcpListener::bind(&addr_str).await?;
    tracing::info!("Hanbova API listening on http://{}", addr_str);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    fn setup_test_app() -> Router {
        let config = AppConfig {
            env: "development".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            database_url: None,
            app_version: "0.1.0".to_string(),
            jwt_secret: "test_jwt_secret_for_automated_testing_purposes".to_string(),
            mint_url: "http://127.0.0.1:3338".to_string(),
        };
        let state = AppState::new(config, None);
        build_app(state)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = setup_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["environment"], "development");
        assert_eq!(json["database"], "in_memory");
    }

    #[tokio::test]
    async fn test_version_endpoint() {
        let app = setup_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/version")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["name"], "hanbova-api");
        assert_eq!(json["version"], "0.1.0");
    }

    #[tokio::test]
    async fn test_create_get_and_claim_payment_intent() {
        let app = setup_test_app();

        // 1. Create Protected Payment Intent
        let payload = serde_json::json!({
            "payment_type": "protected",
            "amount_sats": 21000,
            "recipient_identifier": "merchant@hanbova.africa",
            "description": "Artisan coffee order #402",
            "claim_window_seconds": 3600
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/payment-intents")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["payment_type"], "protected");
        assert_eq!(json["status"], "claimable");
        assert_eq!(json["amount_sats"], 21000);
        assert_eq!(json["recipient_identifier"], "merchant@hanbova.africa");
        assert!(json["claim_reference"].is_string());

        let id = json["id"].as_str().unwrap();

        // 2. Fetch by ID
        let get_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/payment-intents/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(get_response.status(), StatusCode::OK);
        let get_body = get_response.into_body().collect().await.unwrap().to_bytes();
        let get_json: Value = serde_json::from_slice(&get_body).unwrap();
        assert_eq!(get_json["id"], id);
        assert_eq!(get_json["amount_sats"], 21000);

        // 3. Status Coordination Update after Client Mint Settlement
        let status_payload = serde_json::json!({
            "status": "claimed"
        });

        let status_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/payment-intents/{id}/status"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&status_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(status_response.status(), StatusCode::OK);
        let status_body = status_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let status_json: Value = serde_json::from_slice(&status_body).unwrap();
        assert_eq!(status_json["status"], "claimed");
    }

    #[tokio::test]
    async fn test_auth_full_lifecycle() {
        let app = setup_test_app();

        // 1. Register User
        let register_payload = serde_json::json!({
            "username": "jeremiah",
            "email": "jeremiah@hanbova.africa",
            "first_name": "Jeremiah",
            "last_name": "Kon",
            "phone": "+2348012345678",
            "password": "StrongPassword2026!"
        });

        let reg_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(reg_res.status(), StatusCode::CREATED);
        let reg_body = reg_res.into_body().collect().await.unwrap().to_bytes();
        let reg_json: Value = serde_json::from_slice(&reg_body).unwrap();
        assert_eq!(reg_json["user"]["username"], "jeremiah");
        assert_eq!(reg_json["user"]["handle"], "@jeremiah");
        assert!(reg_json["access_token"].is_string());
        assert!(reg_json["refresh_token"].is_string());

        let access_token = reg_json["access_token"].as_str().unwrap();
        let refresh_token = reg_json["refresh_token"].as_str().unwrap();

        // 2. Register Duplicate Username should fail
        let dup_username_payload = serde_json::json!({
            "username": "jeremiah",
            "email": "another@hanbova.africa",
            "first_name": "Other",
            "last_name": "User",
            "password": "StrongPassword2026!"
        });
        let dup_user_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&dup_username_payload).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(dup_user_res.status(), StatusCode::BAD_REQUEST);

        // 3. Register Duplicate Email should fail
        let dup_email_payload = serde_json::json!({
            "username": "different",
            "email": "jeremiah@hanbova.africa",
            "first_name": "Other",
            "last_name": "User",
            "password": "StrongPassword2026!"
        });
        let dup_email_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&dup_email_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(dup_email_res.status(), StatusCode::BAD_REQUEST);

        // 4. Login with Username & Valid Password
        let login_payload = serde_json::json!({
            "login": "jeremiah",
            "password": "StrongPassword2026!"
        });
        let login_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_res.status(), StatusCode::OK);

        // 5. Login with Invalid Password should fail
        let invalid_login_payload = serde_json::json!({
            "login": "jeremiah",
            "password": "WrongPassword!"
        });
        let invalid_login_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&invalid_login_payload).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid_login_res.status(), StatusCode::BAD_REQUEST);

        // 6. Get Profile with Bearer Token (/api/v1/me)
        let me_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/me")
                    .header("authorization", format!("Bearer {access_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(me_res.status(), StatusCode::OK);
        let me_body = me_res.into_body().collect().await.unwrap().to_bytes();
        let me_json: Value = serde_json::from_slice(&me_body).unwrap();
        assert_eq!(me_json["username"], "jeremiah");
        assert_eq!(me_json["email"], "jeremiah@hanbova.africa");

        // 7. Refresh Token
        let refresh_payload = serde_json::json!({
            "refresh_token": refresh_token
        });
        let refresh_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/refresh")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&refresh_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(refresh_res.status(), StatusCode::OK);
        let refresh_body = refresh_res.into_body().collect().await.unwrap().to_bytes();
        let refresh_json: Value = serde_json::from_slice(&refresh_body).unwrap();
        assert!(refresh_json["access_token"].is_string());

        // 8. Forgot Password Flow
        let forgot_payload = serde_json::json!({
            "email": "jeremiah@hanbova.africa"
        });
        let forgot_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/forgot-password")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&forgot_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(forgot_res.status(), StatusCode::OK);
        let forgot_body = forgot_res.into_body().collect().await.unwrap().to_bytes();
        let forgot_json: Value = serde_json::from_slice(&forgot_body).unwrap();
        let reset_token = forgot_json["dev_reset_token"].as_str().unwrap();

        // 9. Reset Password Flow
        let reset_payload = serde_json::json!({
            "token": reset_token,
            "new_password": "NewSuperSecretPassword2026!"
        });
        let reset_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/reset-password")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&reset_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(reset_res.status(), StatusCode::OK);

        // 10. Login with Old Password should fail, new password succeeds
        let old_login_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(old_login_res.status(), StatusCode::BAD_REQUEST);

        let new_login_payload = serde_json::json!({
            "login": "jeremiah",
            "password": "NewSuperSecretPassword2026!"
        });
        let new_login_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&new_login_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(new_login_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_payment_keys_and_encrypted_protected_messages() {
        let app = setup_test_app();

        // 1. Register Alice
        let alice_reg = serde_json::json!({
            "username": "alice",
            "email": "alice@hanbova.africa",
            "first_name": "Alice",
            "last_name": "Send",
            "password": "SecurePassword123!"
        });
        let alice_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&alice_reg).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(alice_res.status(), StatusCode::CREATED);
        let alice_body = alice_res.into_body().collect().await.unwrap().to_bytes();
        let alice_json: Value = serde_json::from_slice(&alice_body).unwrap();
        let alice_token = alice_json["access_token"].as_str().unwrap();

        // 2. Register Bob
        let bob_reg = serde_json::json!({
            "username": "bob",
            "email": "bob@hanbova.africa",
            "first_name": "Bob",
            "last_name": "Receive",
            "password": "SecurePassword123!"
        });
        let bob_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&bob_reg).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bob_res.status(), StatusCode::CREATED);
        let bob_body = bob_res.into_body().collect().await.unwrap().to_bytes();
        let bob_json: Value = serde_json::from_slice(&bob_body).unwrap();
        let bob_token = bob_json["access_token"].as_str().unwrap();

        // 3. Register Charlie (Attacker / Unauthorized user)
        let charlie_reg = serde_json::json!({
            "username": "charlie",
            "email": "charlie@hanbova.africa",
            "first_name": "Charlie",
            "last_name": "Attacker",
            "password": "SecurePassword123!"
        });
        let charlie_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&charlie_reg).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(charlie_res.status(), StatusCode::CREATED);
        let charlie_body = charlie_res.into_body().collect().await.unwrap().to_bytes();
        let charlie_json: Value = serde_json::from_slice(&charlie_body).unwrap();
        let charlie_token = charlie_json["access_token"].as_str().unwrap();

        // 4. Bob Publishes Public Payment Keys
        let bob_keys_payload = serde_json::json!({
            "protected_payment_pubkey": "02a1633cafcc01ebfb6d78e39f687a1f0995c62fc95f51ead10a02ee0be551b5af",
            "transport_encryption_pubkey": "6d9b4b9b9c9f0b83e3c09f8e434f0e9d6d9b4b9b9c9f0b83e3c09f8e434f0e9d"
        });
        let bob_keys_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/me/payment-keys")
                    .header("authorization", format!("Bearer {}", bob_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&bob_keys_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bob_keys_res.status(), StatusCode::OK);

        // 5. Alice looks up Bob's Public Payment Profile
        let lookup_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/users/bob/payment-profile")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(lookup_res.status(), StatusCode::OK);
        let lookup_body = lookup_res.into_body().collect().await.unwrap().to_bytes();
        let lookup_json: Value = serde_json::from_slice(&lookup_body).unwrap();
        assert_eq!(lookup_json["username"], "bob");
        assert_eq!(lookup_json["handle"], "@bob");
        assert_eq!(
            lookup_json["protected_payment_pubkey"],
            "02a1633cafcc01ebfb6d78e39f687a1f0995c62fc95f51ead10a02ee0be551b5af"
        );
        assert_eq!(
            lookup_json["transport_encryption_pubkey"],
            "6d9b4b9b9c9f0b83e3c09f8e434f0e9d6d9b4b9b9c9f0b83e3c09f8e434f0e9d"
        );

        // 6. Alice sends End-to-End Encrypted Envelope to Bob
        let mock_ciphertext = "enc_v1:98fae83b109dc08a9c8b7e6f5d4c3b2a10:opaque_authenticated_ciphertext_payload";
        let message_payload = serde_json::json!({
            "recipient_username": "@bob",
            "encrypted_payload": mock_ciphertext,
            "payload_version": 1
        });
        let send_msg_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/protected-messages")
                    .header("authorization", format!("Bearer {}", alice_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&message_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(send_msg_res.status(), StatusCode::CREATED);
        let send_msg_body = send_msg_res.into_body().collect().await.unwrap().to_bytes();
        let send_msg_json: Value = serde_json::from_slice(&send_msg_body).unwrap();
        let message_id = send_msg_json["id"].as_str().unwrap();
        assert_eq!(send_msg_json["sender_username"], "alice");
        assert_eq!(send_msg_json["recipient_username"], "bob");
        assert_eq!(send_msg_json["encrypted_payload"], mock_ciphertext);
        assert_eq!(send_msg_json["status"], "delivered");

        // 7. Bob checks his Inbox -> sees message
        let inbox_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/protected-messages/inbox")
                    .header("authorization", format!("Bearer {}", bob_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(inbox_res.status(), StatusCode::OK);
        let inbox_body = inbox_res.into_body().collect().await.unwrap().to_bytes();
        let inbox_json: Value = serde_json::from_slice(&inbox_body).unwrap();
        assert!(inbox_json.is_array());
        assert_eq!(inbox_json.as_array().unwrap().len(), 1);
        assert_eq!(inbox_json[0]["id"], message_id);

        // 8. Bob fetches message by ID -> OK
        let get_msg_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/protected-messages/{}", message_id))
                    .header("authorization", format!("Bearer {}", bob_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_msg_res.status(), StatusCode::OK);

        // 9. Charlie (Unauthorized) attempts to fetch Bob's message -> Must be REJECTED
        let charlie_attempt_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/protected-messages/{}", message_id))
                    .header("authorization", format!("Bearer {}", charlie_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(charlie_attempt_res.status(), StatusCode::BAD_REQUEST);

        // 10. Bob acknowledges message (e.g. status = "claimed")
        let ack_payload = serde_json::json!({
            "status": "claimed"
        });
        let ack_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/protected-messages/{}/ack", message_id))
                    .header("authorization", format!("Bearer {}", bob_token))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&ack_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ack_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_lightning_invoice_and_pay() {
        let app = setup_test_app();

        // 1. Create Invoice
        let invoice_payload = serde_json::json!({
            "amount_sats": 2500,
            "description": "Coffee on Lightning",
            "expiry_seconds": 3600
        });

        let invoice_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/lightning/invoice")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&invoice_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(invoice_res.status(), StatusCode::CREATED);
        let invoice_body = invoice_res.into_body().collect().await.unwrap().to_bytes();
        let invoice_json: Value = serde_json::from_slice(&invoice_body).unwrap();
        let bolt11 = invoice_json["bolt11"].as_str().unwrap();
        assert!(bolt11.starts_with("lnbc") || bolt11.starts_with("lntb"));

        // 2. Pay Invoice
        let pay_payload = serde_json::json!({
            "bolt11": bolt11,
            "max_fee_sats": 10
        });

        let pay_res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/lightning/pay")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&pay_payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(pay_res.status(), StatusCode::OK);
        let pay_body = pay_res.into_body().collect().await.unwrap().to_bytes();
        let pay_json: Value = serde_json::from_slice(&pay_body).unwrap();
        assert_eq!(pay_json["status"], "succeeded");
    }
}

