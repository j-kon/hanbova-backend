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

/// A single entry in the all-rates response. `rate` is null when that market is unavailable.
#[derive(Debug, Serialize)]
pub struct MarketRateEntry {
    pub market: String,
    pub currency: String,
    pub available: bool,
    pub rate: Option<serde_json::Value>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rates/hanbova", get(get_hanbova_rate))
        .route("/rates/hanbova/all", get(get_all_hanbova_rates))
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

async fn get_all_hanbova_rates(State(state): State<AppState>) -> Response {
    let rates = state.rate_service.get_all_rates().await;

    // Map Vec<Option<HanbovaRate>> to a structured list with availability flags
    let entries: Vec<MarketRateEntry> = rates
        .into_iter()
        .zip(crate::providers::ALL_MARKETS.iter())
        .map(|(maybe_rate, (market, _asset, currency))| {
            let available = maybe_rate.is_some();
            let rate_json = maybe_rate
                .as_ref()
                .and_then(|r| serde_json::to_value(r).ok());
            MarketRateEntry {
                market: market.to_string(),
                currency: currency.to_string(),
                available,
                rate: rate_json,
            }
        })
        .collect();

    (StatusCode::OK, Json(entries)).into_response()
}
