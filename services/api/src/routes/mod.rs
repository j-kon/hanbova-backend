use axum::Router;

use crate::state::AppState;

pub mod auth;
pub mod health;
pub mod payment_intents;

pub fn create_api_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .nest("/payment-intents", payment_intents::router())
}
