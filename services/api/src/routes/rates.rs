use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct RateQuery {
    pub market: Option<String>,
    pub asset: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RateErrorResponse {
    pub error: String,
    pub code: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rates/hanbova", get(get_hanbova_rate))
}

async fn get_hanbova_rate(
    State(state): State<AppState>,
    Query(query): Query<RateQuery>,
) -> Response {
    let market = query.market.as_deref().unwrap_or("NG");
    let asset = query.asset.as_deref().unwrap_or("USDT");
    let currency = query.currency.as_deref().unwrap_or("NGN");

    match state.rate_service.get_rate(market, asset, currency).await {
        Ok(rate) => (StatusCode::OK, Json(rate)).into_response(),
        Err(err) => {
            tracing::warn!(error = %err, "Failed to get Hanbova platform rate");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(RateErrorResponse {
                    error: "Rate temporarily unavailable".to_string(),
                    code: "rate_unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}
