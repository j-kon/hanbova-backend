use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    providers::{CreateCardRequest, CreatePayoutRequest, PayoutQuoteRequest},
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

use crate::providers::CapabilityStatus;

fn payouts_disabled_response() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "code": "provider_unavailable",
            "message": "Payout services are not available in this environment",
            "error": "Payout services are not available in this environment"
        })),
    )
}

fn cards_disabled_response() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "code": "provider_unavailable",
            "message": "Card services are not available in this environment",
            "error": "Card services are not available in this environment"
        })),
    )
}

async fn get_corridors(
    State(state): State<AppState>,
    Query(q): Query<CorridorsQuery>,
) -> impl IntoResponse {
    if state.capabilities().bitnob_payouts == CapabilityStatus::Disabled {
        return payouts_disabled_response();
    }
    match state
        .payout_provider
        .get_supported_corridors(q.country.as_deref())
        .await
    {
        Ok(corridors) => (StatusCode::OK, Json(json!({ "corridors": corridors }))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn create_payout_quote(
    State(state): State<AppState>,
    Json(req): Json<PayoutQuoteRequest>,
) -> impl IntoResponse {
    if state.capabilities().bitnob_payouts == CapabilityStatus::Disabled {
        return payouts_disabled_response();
    }
    match state.payout_provider.get_payout_quote(&req).await {
        Ok(quote) => (StatusCode::OK, Json(json!(quote))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn execute_payout(
    State(state): State<AppState>,
    Json(req): Json<CreatePayoutRequest>,
) -> impl IntoResponse {
    if state.capabilities().bitnob_payouts == CapabilityStatus::Disabled {
        return payouts_disabled_response();
    }
    match state.payout_provider.create_payout(&req).await {
        Ok(tx) => (StatusCode::OK, Json(json!(tx))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn get_payout_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.capabilities().bitnob_payouts == CapabilityStatus::Disabled {
        return payouts_disabled_response();
    }
    match state.payout_provider.get_payout_status(&id).await {
        Ok(tx) => (StatusCode::OK, Json(json!(tx))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn check_card_eligibility(
    State(state): State<AppState>,
    Query(q): Query<CardEligibilityQuery>,
) -> impl IntoResponse {
    if state.capabilities().bitnob_wallet == CapabilityStatus::Disabled {
        return cards_disabled_response();
    }
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    match state.card_provider.check_card_eligibility(&country).await {
        Ok(eligibility) => (StatusCode::OK, Json(json!(eligibility))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn create_card(
    State(state): State<AppState>,
    Json(req): Json<CreateCardRequest>,
) -> impl IntoResponse {
    if state.capabilities().bitnob_wallet == CapabilityStatus::Disabled {
        return cards_disabled_response();
    }
    match state.card_provider.create_virtual_card(&req).await {
        Ok(card) => (StatusCode::OK, Json(json!(card))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn get_card_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.capabilities().bitnob_wallet == CapabilityStatus::Disabled {
        return cards_disabled_response();
    }
    match state.card_provider.get_card_status(&id).await {
        Ok(card) => (StatusCode::OK, Json(json!(card))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}
