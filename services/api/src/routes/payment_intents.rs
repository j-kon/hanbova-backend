use axum::{
    routing::{get, patch, post},
    Router,
};

use crate::{
    handlers::{
        create_payment_intent, get_payment_intent, get_payment_intent_by_reference,
        list_payment_intents, update_payment_intent_status,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_payment_intent).get(list_payment_intents))
        .route(
            "/by-reference/:reference",
            get(get_payment_intent_by_reference),
        )
        .route("/:id", get(get_payment_intent))
        .route(
            "/:id/status",
            patch(update_payment_intent_status).post(update_payment_intent_status),
        )
}
