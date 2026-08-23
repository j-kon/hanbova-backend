use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub environment: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub database: String,
}

#[derive(Debug, Serialize)]
pub struct VersionResponse {
    pub name: String,
    pub version: String,
    pub environment: String,
}

pub async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let db_status = match &state.db_pool {
        Some(pool) => match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => "connected".to_string(),
            Err(e) => format!("unhealthy: {e}"),
        },
        None => "in_memory".to_string(),
    };

    let status_code = if db_status.starts_with("unhealthy") {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };

    (
        status_code,
        Json(HealthResponse {
            status: "ok".to_string(),
            environment: state.config.env.clone(),
            timestamp: Utc::now(),
            database: db_status,
        }),
    )
}

pub async fn version_info(State(state): State<AppState>) -> Json<VersionResponse> {
    Json(VersionResponse {
        name: "hanbova-api".to_string(),
        version: state.config.app_version.clone(),
        environment: state.config.env.clone(),
    })
}
