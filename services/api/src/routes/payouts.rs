use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    providers::{
        bitnob::BitnobAdapter, CardProvider, CreateCardRequest, CreatePayoutRequest,
        PayoutProvider, PayoutQuoteRequest,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/payouts/corridors", get(get_corridors))
        .route("/payouts/quote", post(create_payout_quote))
        .route("/payouts/execute", post(execute_payout))
        .route("/payouts/:id", get(get_payout_status))
        .route("/cards/eligibility", get(check_card_eligibility))
        .route("/cards/create", post(create_card))
        .route("/cards/:id", get(get_card_status))
}

#[derive(Debug, Deserialize)]
struct CorridorsQuery {
    country: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CardEligibilityQuery {
    country: Option<String>,
}

async fn get_corridors(Query(q): Query<CorridorsQuery>) -> impl IntoResponse {
    let adapter = BitnobAdapter::new();
    match adapter.get_supported_corridors(q.country.as_deref()).await {
        Ok(corridors) => (StatusCode::OK, Json(json!({ "corridors": corridors }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn create_payout_quote(Json(req): Json<PayoutQuoteRequest>) -> impl IntoResponse {
    let adapter = BitnobAdapter::new();
    match adapter.get_payout_quote(&req).await {
        Ok(quote) => (StatusCode::OK, Json(json!(quote))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn execute_payout(Json(req): Json<CreatePayoutRequest>) -> impl IntoResponse {
    let adapter = BitnobAdapter::new();
    match adapter.create_payout(&req).await {
        Ok(tx) => (StatusCode::OK, Json(json!(tx))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn get_payout_status(Path(id): Path<String>) -> impl IntoResponse {
    let adapter = BitnobAdapter::new();
    match adapter.get_payout_status(&id).await {
        Ok(tx) => (StatusCode::OK, Json(json!(tx))),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))),
    }
}

async fn check_card_eligibility(Query(q): Query<CardEligibilityQuery>) -> impl IntoResponse {
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    let adapter = BitnobAdapter::new();
    match adapter.check_card_eligibility(&country).await {
        Ok(eligibility) => (StatusCode::OK, Json(json!(eligibility))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn create_card(Json(req): Json<CreateCardRequest>) -> impl IntoResponse {
    let adapter = BitnobAdapter::new();
    match adapter.create_virtual_card(&req).await {
        Ok(card) => (StatusCode::OK, Json(json!(card))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn get_card_status(Path(id): Path<String>) -> impl IntoResponse {
    let adapter = BitnobAdapter::new();
    match adapter.get_card_status(&id).await {
        Ok(card) => (StatusCode::OK, Json(json!(card))),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))),
    }
}
