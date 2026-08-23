use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::{
        claim_payment_intent, create_payment_intent, get_payment_intent, list_payment_intents,
        refund_payment_intent,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_payment_intent).get(list_payment_intents))
        .route("/:id", get(get_payment_intent))
        .route("/:id/claim", post(claim_payment_intent))
        .route("/:id/refund", post(refund_payment_intent))
}
