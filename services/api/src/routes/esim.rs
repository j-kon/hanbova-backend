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
    providers::{dtone::DtOneAdapter, EsimProvider, PurchaseEsimRequest},
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

async fn get_packages(Query(q): Query<PackagesQuery>) -> impl IntoResponse {
    let country = q.country.unwrap_or_else(|| "KE".to_string());
    let adapter = DtOneAdapter::new();
    match adapter.get_esim_packages(&country).await {
        Ok(packages) => (StatusCode::OK, Json(json!({ "packages": packages }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn purchase_esim(Json(req): Json<PurchaseEsimRequest>) -> impl IntoResponse {
    let adapter = DtOneAdapter::new();
    match adapter.purchase_esim(&req).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn list_profiles() -> impl IntoResponse {
    let adapter = DtOneAdapter::new();
    let profile = adapter.get_esim_status("esim_prof_sample").await;
    match profile {
        Ok(p) => (StatusCode::OK, Json(json!({ "profiles": vec![p] }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}

async fn get_profile(Path(id): Path<String>) -> impl IntoResponse {
    let adapter = DtOneAdapter::new();
    match adapter.get_esim_status(&id).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))),
    }
}

async fn topup_profile(Path(id): Path<String>, Json(req): Json<TopupRequest>) -> impl IntoResponse {
    let adapter = DtOneAdapter::new();
    match adapter.top_up_esim(&id, &req.package_id).await {
        Ok(profile) => (StatusCode::OK, Json(json!(profile))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))),
    }
}
