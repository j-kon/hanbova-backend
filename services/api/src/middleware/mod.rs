use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderValue, Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tokio::sync::Mutex;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};

/// 2 MB max request body limit for standard JSON payloads.
pub const MAX_REQUEST_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_REQUESTS_PER_MINUTE: usize = 120;

#[derive(Clone)]
pub struct RequestRateLimiter {
    state: Arc<Mutex<HashMap<Option<IpAddr>, RequestWindow>>>,
    limit: usize,
    window: Duration,
}

struct RequestWindow {
    started_at: Instant,
    requests: usize,
}

impl RequestRateLimiter {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            limit,
            window,
        }
    }

    pub async fn allow_request(&self, peer: Option<IpAddr>) -> bool {
        const MAX_TRACKED_PEERS: usize = 10_000;
        let mut peers = self.state.lock().await;
        let now = Instant::now();
        if !peers.contains_key(&peer) {
            peers.retain(|_, window| now.duration_since(window.started_at) < self.window);
            if peers.len() >= MAX_TRACKED_PEERS {
                return false;
            }
        }
        let state = peers.entry(peer).or_insert(RequestWindow {
            started_at: now,
            requests: 0,
        });
        if now.duration_since(state.started_at) >= self.window {
            state.started_at = now;
            state.requests = 0;
        }
        if state.requests >= self.limit {
            return false;
        }
        state.requests += 1;
        true
    }
}

pub async fn enforce_rate_limit(
    State(limiter): State<RequestRateLimiter>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // Use the socket peer supplied by Axum, never client-controlled forwarding
    // headers. Requests behind a proxy share its limit until trusted proxy
    // handling is explicitly configured.
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    if !limiter.allow_request(peer).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "60")],
            axum::Json(serde_json::json!({
                "error": "RATE_LIMITED",
                "message": "Too many requests. Please try again shortly."
            })),
        )
            .into_response();
    }
    next.run(request).await
}

use crate::config::AppConfig;

pub fn cors_layer(config: &AppConfig) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);

    if config.is_production() {
        let origins = config
            .cors_allowed_origins
            .iter()
            .map(|origin| HeaderValue::from_str(origin).expect("validated CORS origin"));
        layer.allow_origin(AllowOrigin::list(origins))
    } else {
        layer
    }
}

pub fn request_limit_layer() -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES)
}

pub fn trace_layer(
) -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>
{
    TraceLayer::new_for_http()
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    use crate::config::AppConfig;

    use super::cors_layer;

    #[tokio::test]
    async fn production_cors_allows_only_configured_origins() {
        let config = AppConfig::from_iter([
            ("HANBOVA_ENV", "production"),
            ("HANBOVA_API_HOST", "0.0.0.0"),
            ("HANBOVA_API_PORT", "8080"),
            ("DATABASE_URL", "postgres://hanbova:secret@db/hanbova"),
            ("JWT_SECRET", "production-secret-that-is-at-least-32-bytes"),
            ("MINT_URL", "https://mint.example.com"),
            ("PROVIDER_MODE", "production"),
            ("CORS_ALLOWED_ORIGINS", "https://app.example.com"),
        ])
        .unwrap();
        let app = Router::new()
            .route("/", get(|| async { StatusCode::OK }))
            .layer(cors_layer(&config));

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/")
                    .header("origin", "https://untrusted.example")
                    .header("access-control-request-method", "POST")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response
            .headers()
            .get("access-control-allow-origin")
            .is_none());
    }

    #[tokio::test]
    async fn request_rate_limiter_rejects_requests_over_its_window_limit() {
        let limiter = super::RequestRateLimiter::new(1, std::time::Duration::from_secs(60));
        assert!(limiter.allow_request(None).await);
        assert!(!limiter.allow_request(None).await);
    }

    #[tokio::test]
    async fn rate_limit_isolated_by_peer_not_spoofed_forwarded_header() {
        use axum::extract::ConnectInfo;
        use std::net::SocketAddr;
        let limiter = super::RequestRateLimiter::new(1, std::time::Duration::from_secs(60));
        let app = Router::new()
            .route("/", get(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn_with_state(
                limiter,
                super::enforce_rate_limit,
            ));
        for (ip, expected) in [
            ("127.0.0.1:1000", StatusCode::OK),
            ("127.0.0.1:1001", StatusCode::TOO_MANY_REQUESTS),
            ("127.0.0.2:1000", StatusCode::OK),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/")
                        .extension(ConnectInfo(ip.parse::<SocketAddr>().unwrap()))
                        .header("x-forwarded-for", "untrusted-client")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
    }
}
