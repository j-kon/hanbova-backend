use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;
use hanbova_core::SatoshiAmount;
use hanbova_lightning::{CreateInvoiceRequest, PayInvoiceRequest};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInvoiceDto {
    pub amount_sats: u64,
    pub description: Option<String>,
    pub expiry_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayInvoiceDto {
    pub bolt11: String,
    pub max_fee_sats: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintQuoteDto {
    pub amount_sats: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MeltQuoteDto {
    pub bolt11: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lightning/invoice", post(create_invoice))
        .route("/lightning/pay", post(pay_invoice))
        .route("/lightning/mint-quote", post(create_mint_quote))
        .route("/lightning/mint-quote/:quote_id", get(check_mint_quote))
        .route("/lightning/melt-quote", post(create_melt_quote))
}

async fn create_invoice(
    State(state): State<AppState>,
    Json(payload): Json<CreateInvoiceDto>,
) -> impl IntoResponse {
    let amount = match SatoshiAmount::new(payload.amount_sats) {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let req = CreateInvoiceRequest {
        amount_sats: amount,
        description: payload.description.unwrap_or_else(|| "Hanbova Lightning Receive".to_string()),
        expiry_seconds: payload.expiry_seconds.map(|s| s as u32),
    };

    match state.lightning_provider.create_invoice(req).await {
        Ok(invoice) => (StatusCode::CREATED, Json(serde_json::to_value(invoice).unwrap())).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn pay_invoice(
    State(state): State<AppState>,
    Json(payload): Json<PayInvoiceDto>,
) -> impl IntoResponse {
    let req = PayInvoiceRequest {
        bolt11: payload.bolt11,
        max_fee_sats: payload.max_fee_sats,
    };

    match state.lightning_provider.pay_invoice(req).await {
        Ok(payment) => (StatusCode::OK, Json(serde_json::to_value(payment).unwrap())).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn create_mint_quote(
    State(state): State<AppState>,
    Json(payload): Json<MintQuoteDto>,
) -> impl IntoResponse {
    match state.cashu_bridge.create_mint_quote(payload.amount_sats).await {
        Ok(quote) => (StatusCode::OK, Json(serde_json::to_value(quote).unwrap())).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn check_mint_quote(
    State(state): State<AppState>,
    Path(quote_id): Path<String>,
) -> impl IntoResponse {
    match state.cashu_bridge.check_mint_quote(&quote_id).await {
        Ok(quote) => (StatusCode::OK, Json(serde_json::to_value(quote).unwrap())).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn create_melt_quote(
    State(state): State<AppState>,
    Json(payload): Json<MeltQuoteDto>,
) -> impl IntoResponse {
    match state.cashu_bridge.create_melt_quote(&payload.bolt11).await {
        Ok(quote) => (StatusCode::OK, Json(serde_json::to_value(quote).unwrap())).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
