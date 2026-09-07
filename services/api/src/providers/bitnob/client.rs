use chrono::Utc;
use reqwest::{Client, Method, Response, StatusCode};
use std::time::Duration;

use super::{
    auth::{generate_nonce, generate_signature},
    models::{
        BitnobErrorDetail, BitnobExchangeRateData, BitnobExchangeRateResponse, BitnobPayoutQuote,
        BitnobPayoutQuoteRequest, BitnobQuoteResponse, WhoamiResponse,
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

pub const OFFICIAL_BITNOB_BASE_URL: &str = "https://api.bitnob.com";

fn is_allowed_local_test_url(url: &str) -> bool {
    url.starts_with("http://127.0.0.1")
        || url.starts_with("http://localhost")
        || url.starts_with("http://[::1]")
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
    /// Both Sandbox and Production strictly enforce `https://api.bitnob.com`
    /// and reject arbitrary insecure remote HTTP URLs.
    pub fn with_mode(mode: ProviderMode) -> Self {
        let client_id = std::env::var("BITNOB_CLIENT_ID")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let client_secret = std::env::var("BITNOB_CLIENT_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let base_url = match mode {
            ProviderMode::Sandbox | ProviderMode::Production => {
                if let Ok(custom_url) = std::env::var("BITNOB_BASE_URL") {
                    let trimmed = custom_url.trim();
                    if trimmed.is_empty() {
                        OFFICIAL_BITNOB_BASE_URL.to_string()
                    } else if trimmed.starts_with("https://") || is_allowed_local_test_url(trimmed)
                    {
                        trimmed.to_string()
                    } else {
                        tracing::warn!(
                            provider = "bitnob",
                            environment = %mode,
                            url = %trimmed,
                            "Insecure HTTP BITNOB_BASE_URL rejected in {:?}; falling back to official {}",
                            mode,
                            OFFICIAL_BITNOB_BASE_URL
                        );
                        OFFICIAL_BITNOB_BASE_URL.to_string()
                    }
                } else {
                    OFFICIAL_BITNOB_BASE_URL.to_string()
                }
            }
            ProviderMode::Mock => std::env::var("BITNOB_BASE_URL")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| OFFICIAL_BITNOB_BASE_URL.to_string()),
        };

        let mut builder = Client::builder().timeout(Duration::from_secs(8));
        if std::env::var("BITNOB_DIAGNOSTIC_FORCE_IPV4").as_deref() == Ok("true") {
            builder = builder.local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
        }
        let http_client = builder.build().unwrap_or_default();

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
        let normalized_id = client_id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let normalized_secret = client_secret
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let default_url = OFFICIAL_BITNOB_BASE_URL.to_string();
        let final_base_url = match base_url {
            Some(url) => {
                let trimmed = url.trim();
                if (mode == ProviderMode::Sandbox || mode == ProviderMode::Production)
                    && trimmed.starts_with("http://")
                    && !is_allowed_local_test_url(trimmed)
                {
                    tracing::warn!(
                        "Insecure remote HTTP URL rejected for Bitnob in {:?}; using official {}",
                        mode,
                        OFFICIAL_BITNOB_BASE_URL
                    );
                    default_url
                } else {
                    trimmed.to_string()
                }
            }
            None => default_url,
        };

        let mut builder = Client::builder().timeout(Duration::from_secs(5));
        if std::env::var("BITNOB_DIAGNOSTIC_FORCE_IPV4").as_deref() == Ok("true") {
            builder = builder.local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
        }
        let http_client = builder.build().unwrap_or_default();

        Self {
            client_id: normalized_id,
            client_secret: normalized_secret,
            base_url: final_base_url,
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

        if (self.mode == ProviderMode::Sandbox || self.mode == ProviderMode::Production)
            && self.base_url.starts_with("http://")
            && !is_allowed_local_test_url(&self.base_url)
        {
            return Err(ProviderError::Unavailable(
                "Insecure HTTP provider URL not permitted in sandbox/production".to_string(),
            ));
        }

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
            // Extract potential correlation/request IDs from headers first
            let header_corr_id = response
                .headers()
                .get("x-correlation-id")
                .or_else(|| response.headers().get("x-request-id"))
                .or_else(|| response.headers().get("x-amzn-requestid"))
                .or_else(|| response.headers().get("cf-ray"))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            // Read response safely to extract error code and correlation_id without leaking payload
            let err_text = response.text().await.unwrap_or_default();
            let parsed_err: Option<BitnobErrorDetail> = serde_json::from_str(&err_text).ok();
            let correlation_id = parsed_err
                .as_ref()
                .and_then(|e| e.correlation_id.clone().or_else(|| e.request_id.clone()))
                .or(header_corr_id)
                .unwrap_or_else(|| "unavailable".to_string());

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

            let corr_suffix = if correlation_id != "unavailable" && correlation_id != "none" {
                format!(" [correlation_id: {correlation_id}]")
            } else {
                String::new()
            };

            // Map HTTP status codes to differentiated ProviderError types without leaking auth material
            match status {
                StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                    let safe_detail = if error_detail.trim().is_empty() {
                        "Bitnob rejected the request (validation failed)".to_string()
                    } else {
                        error_detail
                    };
                    return Err(ProviderError::ValidationFailed(format!(
                        "{safe_detail}{corr_suffix}"
                    )));
                }
                StatusCode::CONFLICT => {
                    let safe_detail = if error_detail.trim().is_empty() {
                        "Bitnob request conflict".to_string()
                    } else {
                        format!("Bitnob conflict: {error_detail}")
                    };
                    return Err(ProviderError::ValidationFailed(format!(
                        "{safe_detail}{corr_suffix}"
                    )));
                }
                StatusCode::UNAUTHORIZED => {
                    return Err(ProviderError::Unavailable(format!(
                        "Bitnob authentication failed{corr_suffix}"
                    )));
                }
                StatusCode::FORBIDDEN => {
                    if error_detail.contains("IP address not whitelisted") {
                        return Err(ProviderError::Unavailable(format!(
                            "Bitnob access forbidden (IP address not whitelisted){corr_suffix}"
                        )));
                    } else {
                        return Err(ProviderError::Unavailable(format!(
                            "Bitnob access forbidden{corr_suffix}"
                        )));
                    }
                }
                StatusCode::NOT_FOUND => {
                    return Err(ProviderError::Unavailable(format!(
                        "Bitnob endpoint not found{corr_suffix}"
                    )));
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    return Err(ProviderError::RateLimit(format!(
                        "Bitnob rate limit exceeded{corr_suffix}"
                    )));
                }
                s if s.is_server_error() => {
                    return Err(ProviderError::Unavailable(format!(
                        "Bitnob provider unavailable{corr_suffix}"
                    )));
                }
                _ => {
                    return Err(ProviderError::Unavailable(format!(
                        "Bitnob HTTP error {status}{corr_suffix}"
                    )));
                }
            }
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

    /// Fetches the real-time indicative exchange rate: `GET /api/exchange-rates?from={from}&to={to}`.
    /// Official Bitnob endpoint for dedicated exchange rate discovery.
    pub async fn get_exchange_rate(
        &self,
        from: &str,
        to: &str,
    ) -> ProviderResult<BitnobExchangeRateData> {
        let from_clean = from.trim().to_uppercase();
        let to_clean = to.trim().to_uppercase();
        let path = format!("/api/exchange-rates?from={from_clean}&to={to_clean}");

        let resp = self.send_signed_request(Method::GET, &path, "").await?;
        let body = resp.text().await.map_err(|_| {
            ProviderError::Internal("Failed to read Bitnob exchange-rates response body".into())
        })?;

        let parsed: BitnobExchangeRateResponse = serde_json::from_str(&body).map_err(|_| {
            ProviderError::Internal("Failed to parse Bitnob exchange-rates response".to_string())
        })?;

        let is_ok = parsed.status.unwrap_or(true) && parsed.success.unwrap_or(true);
        if !is_ok {
            let msg = parsed
                .message
                .unwrap_or_else(|| "Bitnob returned exchange-rates failure status".to_string());
            return Err(ProviderError::Unavailable(msg));
        }

        parsed.data.ok_or_else(|| {
            ProviderError::Unavailable("Missing exchange rate data in response".to_string())
        })
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

/// Classifies a ProviderError into a standardized diagnostic classification string.
pub fn classify_error(err: &ProviderError) -> &'static str {
    match err {
        ProviderError::ValidationFailed(msg) => {
            if msg.to_lowercase().contains("conflict") {
                "PROVIDER_CONFLICT"
            } else {
                "REQUEST_VALIDATION_FAILED"
            }
        }
        ProviderError::RateLimit(_) => "RATE_LIMITED",
        ProviderError::NotConfigured(_) => "MISSING_CREDENTIALS",
        ProviderError::Internal(msg) => {
            if msg.contains("parse") || msg.contains("JSON") {
                "MALFORMED_RESPONSE"
            } else {
                "INTERNAL_ERROR"
            }
        }
        ProviderError::Unavailable(msg) => {
            if msg.contains("IP address not whitelisted") {
                "IP_NOT_WHITELISTED"
            } else if msg.contains("authentication failed") {
                "AUTHENTICATION_FAILED"
            } else if msg.contains("access forbidden") {
                "PROVIDER_FORBIDDEN"
            } else if msg.contains("endpoint not found") || msg.contains("not found") {
                "ENDPOINT_NOT_FOUND"
            } else if msg.contains("network") {
                "NETWORK_ERROR"
            } else {
                "PROVIDER_UNAVAILABLE"
            }
        }
        _ => "PROVIDER_UNAVAILABLE",
    }
}

/// Safely extracts the correlation ID from a ProviderError if present.
pub fn extract_correlation_id(err: &ProviderError) -> Option<String> {
    let msg = match err {
        ProviderError::Unavailable(m)
        | ProviderError::ValidationFailed(m)
        | ProviderError::RateLimit(m)
        | ProviderError::Internal(m) => m,
        _ => return None,
    };
    if let Some(start) = msg.find("[correlation_id: ") {
        let rest = &msg[start + "[correlation_id: ".len()..];
        if let Some(end) = rest.find(']') {
            return Some(rest[..end].trim().to_string());
        }
    }
    None
}
