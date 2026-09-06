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
    providers::{ProviderError, PurchaseEsimRequest},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/esim/packages", get(get_packages))
        .route("/esim/purchase", post(purchase_esim))
        .route("/esim/profiles", get(list_profiles))
        .route("/esim/profiles/:id", get(get_profile))
        .route("/esim/profiles/:id/topup", post(topup_profile))
}

#[derive(Debug, Deserialize)]
struct PackagesQuery {
    country: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TopupRequest {
    package_id: String,
}

fn status_from_provider_error(err: &ProviderError) -> StatusCode {
    match err {
        ProviderError::NotConfigured(_) | ProviderError::Unavailable(_) => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        ProviderError::RateLimit(_) => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::BAD_REQUEST,
    }
}

async fn get_packages(
    State(state): State<AppState>,
    Query(q): Query<PackagesQuery>,
) -> impl IntoResponse {
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    match state.esim_provider.get_esim_packages(&country).await {
        Ok(packages) => (StatusCode::OK, Json(json!({ "packages": packages }))),
        Err(e) => (
            status_from_provider_error(&e),
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn purchase_esim(
    State(state): State<AppState>,
    Json(req): Json<PurchaseEsimRequest>,
) -> impl IntoResponse {
    match state.esim_provider.purchase_esim(&req).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (
            status_from_provider_error(&e),
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn list_profiles(State(state): State<AppState>) -> impl IntoResponse {
    let profile = state
        .esim_provider
        .get_esim_status("esim_prof_sample")
        .await;
    match profile {
        Ok(p) => (StatusCode::OK, Json(json!({ "profiles": vec![p] }))),
        Err(e) => (
            status_from_provider_error(&e),
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn get_profile(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.esim_provider.get_esim_status(&id).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn topup_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<TopupRequest>,
) -> impl IntoResponse {
    match state.esim_provider.top_up_esim(&id, &req.package_id).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (
            status_from_provider_error(&e),
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
