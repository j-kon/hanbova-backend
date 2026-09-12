use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{providers::PurchaseEsimRequest, state::AppState};

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

use crate::providers::CapabilityStatus;

fn esim_disabled_response() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "code": "provider_unavailable",
            "message": "eSIM services are not available in this environment",
            "error": "eSIM services are not available in this environment"
        })),
    )
}

async fn get_packages(
    State(state): State<AppState>,
    Query(q): Query<PackagesQuery>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return esim_disabled_response();
    }
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    match state.esim_provider.get_esim_packages(&country).await {
        Ok(packages) => (StatusCode::OK, Json(json!({ "packages": packages }))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn purchase_esim(
    State(state): State<AppState>,
    Json(req): Json<PurchaseEsimRequest>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return esim_disabled_response();
    }
    match state.esim_provider.purchase_esim(&req).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn list_profiles(State(state): State<AppState>) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return esim_disabled_response();
    }
    let profile = state
        .esim_provider
        .get_esim_status("esim_prof_sample")
        .await;
    match profile {
        Ok(p) => (StatusCode::OK, Json(json!({ "profiles": vec![p] }))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn get_profile(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return esim_disabled_response();
    }
    match state.esim_provider.get_esim_status(&id).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}

async fn topup_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<TopupRequest>,
) -> impl IntoResponse {
    if state.capabilities().dtone_bills == CapabilityStatus::Disabled {
        return esim_disabled_response();
    }
    match state.esim_provider.top_up_esim(&id, &req.package_id).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (e.status_code(), Json(e.to_response_body())),
    }
}
