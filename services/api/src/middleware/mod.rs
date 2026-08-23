use axum::http::Method;
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};

/// 2 MB max request body limit for standard JSON payloads.
pub const MAX_REQUEST_BODY_BYTES: usize = 2 * 1024 * 1024;

pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any)
}

pub fn request_limit_layer() -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES)
}

pub fn trace_layer(
) -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>
{
    TraceLayer::new_for_http()
}
