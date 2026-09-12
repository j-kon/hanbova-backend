use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{auth::handlers::AuthUser, state::AppState};
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

fn lightning_disabled_response() -> axum::response::Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "code": "provider_unavailable",
            "message": "Lightning service is currently disabled in this environment",
            "error": "Lightning service is currently disabled in this environment"
        })),
    )
        .into_response()
}

async fn create_invoice(
    _auth: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateInvoiceDto>,
) -> impl IntoResponse {
    if !state.config.lightning_enabled {
        return lightning_disabled_response();
    }

    let amount = match SatoshiAmount::new(payload.amount_sats) {
        Ok(a) => a,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "code": "invalid_request",
                    "message": "Invalid Lightning amount",
                    "error": "Invalid Lightning amount"
                })),
            )
                .into_response()
        }
    };

    let req = CreateInvoiceRequest {
        amount_sats: amount,
        description: payload
            .description
            .unwrap_or_else(|| "Hanbova Lightning Receive".to_string()),
        expiry_seconds: payload.expiry_seconds.map(|s| s as u32),
    };

    match state.lightning_provider.create_invoice(req).await {
        Ok(invoice) => (
            StatusCode::CREATED,
            Json(serde_json::to_value(invoice).unwrap()),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "code": "provider_internal_error",
                "message": "Unable to create Lightning invoice",
                "error": "Unable to create Lightning invoice"
            })),
        )
            .into_response(),
    }
}

async fn pay_invoice(
    _auth: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<PayInvoiceDto>,
) -> impl IntoResponse {
    if !state.config.lightning_enabled {
        return lightning_disabled_response();
    }

    let req = PayInvoiceRequest {
        bolt11: payload.bolt11,
        max_fee_sats: payload.max_fee_sats,
    };

    match state.lightning_provider.pay_invoice(req).await {
        Ok(payment) => {
            (StatusCode::OK, Json(serde_json::to_value(payment).unwrap())).into_response()
        }
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "code": "invalid_request",
                "message": "Unable to pay Lightning invoice",
                "error": "Unable to pay Lightning invoice"
            })),
        )
            .into_response(),
    }
}

async fn create_mint_quote(
    _auth: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<MintQuoteDto>,
) -> impl IntoResponse {
    if !state.config.lightning_enabled {
        return lightning_disabled_response();
    }

    match state
        .cashu_bridge
        .create_mint_quote(payload.amount_sats)
        .await
    {
        Ok(quote) => (StatusCode::OK, Json(serde_json::to_value(quote).unwrap())).into_response(),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "code": "provider_unavailable",
                "message": "Unable to create mint quote",
                "error": "Unable to create mint quote"
            })),
        )
            .into_response(),
    }
}

async fn check_mint_quote(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(quote_id): Path<String>,
) -> impl IntoResponse {
    if !state.config.lightning_enabled {
        return lightning_disabled_response();
    }

    match state.cashu_bridge.check_mint_quote(&quote_id).await {
        Ok(quote) => (StatusCode::OK, Json(serde_json::to_value(quote).unwrap())).into_response(),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "code": "provider_unavailable",
                "message": "Unable to retrieve mint quote",
                "error": "Unable to retrieve mint quote"
            })),
        )
            .into_response(),
    }
}

async fn create_melt_quote(
    _auth: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<MeltQuoteDto>,
) -> impl IntoResponse {
    if !state.config.lightning_enabled {
        return lightning_disabled_response();
    }

    match state.cashu_bridge.create_melt_quote(&payload.bolt11).await {
        Ok(quote) => (StatusCode::OK, Json(serde_json::to_value(quote).unwrap())).into_response(),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "code": "provider_unavailable",
                "message": "Unable to create melt quote",
                "error": "Unable to create melt quote"
            })),
        )
            .into_response(),
    }
}
