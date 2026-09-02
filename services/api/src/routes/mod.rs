use axum::Router;

use crate::state::AppState;

pub mod auth;
pub mod bills;
pub mod esim;
pub mod health;
pub mod lightning;
pub mod markets;
pub mod payment_intents;
pub mod payouts;
pub mod protected_messages;

pub fn create_api_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(protected_messages::router())
        .merge(lightning::router())
        .merge(markets::router())
        .merge(bills::router())
        .merge(esim::router())
        .merge(payouts::router())
        .nest("/payment-intents", payment_intents::router())
}
