use chrono::Utc;
use reqwest::{Client, Method, Response, StatusCode};
use std::time::Duration;

use super::{
    auth::{generate_nonce, generate_signature},
    models::{
        BitnobErrorDetail, BitnobPayoutQuote, BitnobPayoutQuoteRequest, BitnobQuoteResponse,
        WhoamiResponse,
    },
};
use crate::config::ProviderMode;
use crate::providers::{ProviderError, ProviderResult};

/// Official Bitnob HTTP client providing authenticated request signing and response handling.
#[derive(Debug, Clone)]
pub struct BitnobClient {
    client_id: Option<String>,
    client_secret: Option<String>,
    base_url: String,
    mode: ProviderMode,
    http_client: Client,
}

impl Default for BitnobClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BitnobClient {
    /// Builds a BitnobClient from environment variables.
    pub fn new() -> Self {
        let mode = match std::env::var("PROVIDER_MODE").as_deref() {
            Ok("sandbox") => ProviderMode::Sandbox,
            Ok("production") => ProviderMode::Production,
            _ => ProviderMode::Mock,
        };
        Self::with_mode(mode)
    }

    /// Builds a BitnobClient for a specific provider mode.
    /// Both Sandbox and Production default to `https://api.bitnob.com`.
    pub fn with_mode(mode: ProviderMode) -> Self {
        let client_id = std::env::var("BITNOB_CLIENT_ID")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let client_secret = std::env::var("BITNOB_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let base_url = std::env::var("BITNOB_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://api.bitnob.com".to_string());

        let http_client = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        Self {
            client_id,
            client_secret,
            base_url,
            mode,
            http_client,
        }
    }

    /// Builder for testing with deterministic parameters and local mock endpoints.
    pub fn with_config(
        client_id: Option<String>,
        client_secret: Option<String>,
        mode: ProviderMode,
        base_url: Option<String>,
    ) -> Self {
        let default_url = "https://api.bitnob.com".to_string();

        let http_client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            client_id,
            client_secret,
            base_url: base_url.unwrap_or(default_url),
            mode,
            http_client,
        }
    }

    pub fn is_configured(&self) -> bool {
        match self.mode {
            ProviderMode::Mock => true,
            ProviderMode::Sandbox | ProviderMode::Production => {
                self.client_id.is_some() && self.client_secret.is_some()
            }
        }
    }

    pub fn mode(&self) -> ProviderMode {
        self.mode
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Sends an authenticated request to the Bitnob API.
    /// Computes HMAC signature over the exact payload string.
    async fn send_signed_request(
        &self,
        method: Method,
        path: &str,
        payload: &str,
    ) -> ProviderResult<Response> {
        let (client_id, client_secret) = match (&self.client_id, &self.client_secret) {
            (Some(cid), Some(sec)) => (cid.as_str(), sec.as_str()),
            _ => {
                let env_name = self.mode.to_string();
                return Err(ProviderError::NotConfigured(format!(
                    "Bitnob credentials (BITNOB_CLIENT_ID, BITNOB_CLIENT_SECRET) missing in {env_name}"
                )));
            }
        };

        let timestamp = Utc::now().timestamp() as u64;
        let nonce = generate_nonce();
        let signature = generate_signature(client_id, client_secret, timestamp, &nonce, payload)?;

        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let mut req_builder = self
            .http_client
            .request(method.clone(), &url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("X-Auth-Client", client_id)
            .header("X-Auth-Timestamp", timestamp.to_string())
            .header("X-Auth-Nonce", nonce)
            .header("X-Auth-Signature", signature);

        if !payload.is_empty() {
            req_builder = req_builder.body(payload.to_string());
        }

        let start_time = std::time::Instant::now();
        let response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(err) => {
                let latency_ms = start_time.elapsed().as_millis() as u64;
                tracing::warn!(
                    provider = "bitnob",
                    environment = %self.mode,
                    operation = %path,
                    result = "failure",
                    latency_ms = latency_ms,
                    error = %err,
                    "Bitnob network request failed"
                );
                return Err(ProviderError::Unavailable(
                    "Bitnob network request failed".to_string(),
                ));
            }
        };

        let status = response.status();
        let latency_ms = start_time.elapsed().as_millis() as u64;

        if !status.is_success() {
            // Read response safely to extract error code and correlation_id without leaking payload
            let err_text = response.text().await.unwrap_or_default();
            let parsed_err: Option<BitnobErrorDetail> = serde_json::from_str(&err_text).ok();
            let correlation_id = parsed_err
                .as_ref()
                .and_then(|e| e.correlation_id.clone().or_else(|| e.request_id.clone()))
                .unwrap_or_else(|| "none".to_string());

            let error_detail = parsed_err
                .as_ref()
                .and_then(|e| e.detail.clone().or_else(|| e.message.clone()))
                .unwrap_or_default();

            tracing::warn!(
                provider = "bitnob",
                environment = %self.mode,
                operation = %path,
                result = "failure",
                http_status = %status,
                correlation_id = %correlation_id,
                latency_ms = latency_ms,
                detail = %error_detail,
                "Bitnob upstream returned error status"
            );

            // Sanitize error string: never leak raw response body
            let sanitized_msg = match status {
                StatusCode::UNAUTHORIZED => "Bitnob authentication failed".to_string(),
                StatusCode::FORBIDDEN => {
                    if error_detail.contains("IP address not whitelisted") {
                        "Bitnob access forbidden (IP address not whitelisted)".to_string()
                    } else {
                        "Bitnob access forbidden".to_string()
                    }
                }
                StatusCode::TOO_MANY_REQUESTS => "Bitnob rate limit exceeded".to_string(),
                s if s.is_server_error() => "Bitnob provider unavailable".to_string(),
                _ => format!("Bitnob HTTP error {status}"),
            };

            return Err(ProviderError::Unavailable(sanitized_msg));
        }

        tracing::info!(
            provider = "bitnob",
            environment = %self.mode,
            operation = %path,
            result = "success",
            latency_ms = latency_ms,
            "Bitnob request completed successfully"
        );

        Ok(response)
    }

    /// Verifies authentication against the official Bitnob lightweight endpoint: `GET /api/whoami`.
    pub async fn whoami(&self) -> ProviderResult<WhoamiResponse> {
        let resp = self
            .send_signed_request(Method::GET, "/api/whoami", "")
            .await?;
        let body = resp
            .text()
            .await
            .map_err(|_| ProviderError::Internal("Failed to read Bitnob whoami response".into()))?;

        serde_json::from_str::<WhoamiResponse>(&body)
            .map_err(|_| ProviderError::Internal("Failed to parse Bitnob whoami response".into()))
    }

    /// Creates a payout exchange rate quote: `POST /api/payouts/quotes`.
    /// Serializes request ONCE so the exact signed bytes are sent over the wire.
    pub async fn create_payout_quote(
        &self,
        req: &BitnobPayoutQuoteRequest,
    ) -> ProviderResult<BitnobPayoutQuote> {
        let payload_str = serde_json::to_string(req).map_err(|e| {
            ProviderError::Internal(format!("Failed to serialize payout quote request: {e}"))
        })?;

        let resp = self
            .send_signed_request(Method::POST, "/api/payouts/quotes", &payload_str)
            .await?;

        let body = resp.text().await.map_err(|_| {
            ProviderError::Internal("Failed to read Bitnob quote response body".into())
        })?;

        let parsed: BitnobQuoteResponse = serde_json::from_str(&body).map_err(|_| {
            ProviderError::Internal("Failed to parse Bitnob quote response".to_string())
        })?;

        // Validate top-level status
        let is_ok = parsed.status.unwrap_or(true) && parsed.success.unwrap_or(true);
        if !is_ok {
            let msg = parsed
                .message
                .unwrap_or_else(|| "Bitnob returned failure status".to_string());
            return Err(ProviderError::Unavailable(msg));
        }

        parsed.data.and_then(|d| d.payout).ok_or_else(|| {
            ProviderError::Unavailable("Missing payout quote in response".to_string())
        })
    }
}
