use axum::Router;

use crate::state::AppState;

pub mod auth;
pub mod health;
pub mod lightning;
pub mod payment_intents;
pub mod protected_messages;

pub fn create_api_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(protected_messages::router())
        .merge(lightning::router())
        .nest("/payment-intents", payment_intents::router())
}
