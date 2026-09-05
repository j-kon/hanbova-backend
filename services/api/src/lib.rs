use axum::Router;

pub mod auth;
pub mod config;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod providers;
pub mod repositories;
pub mod routes;
pub mod services;
pub mod startup;
pub mod state;

use state::AppState;

pub fn build_app(state: AppState) -> Router {
    let api_v1 = routes::create_api_router(&state.config);
    let rate_limiter = middleware::RequestRateLimiter::new(
        middleware::MAX_REQUESTS_PER_MINUTE,
        std::time::Duration::from_secs(60),
    );

    Router::new()
        .nest("/api/v1", api_v1)
        .layer(axum::middleware::from_fn_with_state(
            rate_limiter,
            middleware::enforce_rate_limit,
        ))
        .layer(middleware::cors_layer(&state.config))
        .layer(middleware::request_limit_layer())
        .layer(middleware::trace_layer())
        .with_state(state)
}
