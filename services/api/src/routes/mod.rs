use axum::Router;

use crate::{config::AppConfig, state::AppState};

pub mod auth;
pub mod bills;
pub mod esim;
pub mod health;
pub mod lightning;
pub mod markets;
pub mod payment_intents;
pub mod payouts;
pub mod protected_messages;

pub fn create_api_router(config: &AppConfig) -> Router<AppState> {
    let router = Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(protected_messages::router())
        .merge(markets::router())
        .merge(bills::router())
        .merge(esim::router())
        .merge(payouts::router())
        .nest("/payment-intents", payment_intents::router());

    if config.is_production() {
        router
    } else {
        router.merge(lightning::router())
    }
}
