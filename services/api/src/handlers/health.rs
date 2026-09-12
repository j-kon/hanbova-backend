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

#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub status: String,
    pub environment: String,
    pub database: String,
    pub provider_mode: String,
}

#[derive(Debug, Serialize)]
pub struct CapabilitiesResponse {
    pub environment: String,
    pub provider_mode: String,
    pub database: String,
    pub capabilities: crate::providers::ProviderCapabilities,
}

pub async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let (db_status, is_healthy) = match &state.db_pool {
        Some(pool) => match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => ("connected".to_string(), true),
            Err(e) => {
                tracing::error!("Database health check query failed: {e}");
                ("unhealthy".to_string(), false)
            }
        },
        None => ("in_memory".to_string(), true),
    };

    let status_code = if is_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(HealthResponse {
            status: if is_healthy {
                "ok".to_string()
            } else {
                "unhealthy".to_string()
            },
            environment: state.config.environment.to_string(),
            timestamp: Utc::now(),
            database: db_status,
        }),
    )
}

pub async fn readiness_check(
    State(state): State<AppState>,
) -> (StatusCode, Json<ReadinessResponse>) {
    let (db_status, db_ok) = match &state.db_pool {
        Some(pool) => match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => ("connected".to_string(), true),
            Err(e) => {
                tracing::error!("Database readiness check query failed: {e}");
                ("unhealthy".to_string(), false)
            }
        },
        None => {
            if state.config.is_pilot() || state.config.is_production() {
                ("unhealthy".to_string(), false)
            } else {
                ("in_memory".to_string(), true)
            }
        }
    };

    let mut is_ready = db_ok;
    if state.config.is_pilot() {
        if state.config.provider_mode != crate::config::ProviderMode::Sandbox {
            is_ready = false;
        }
        if state.db_pool.is_none() {
            is_ready = false;
        }
    }

    let status_code = if is_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(ReadinessResponse {
            status: if is_ready {
                "ready".to_string()
            } else {
                "not_ready".to_string()
            },
            environment: state.config.environment.to_string(),
            database: db_status,
            provider_mode: state.config.provider_mode.to_string(),
        }),
    )
}

pub async fn get_capabilities(State(state): State<AppState>) -> Json<CapabilitiesResponse> {
    let database = match &state.db_pool {
        Some(_) => "postgres".to_string(),
        None => "in_memory".to_string(),
    };

    Json(CapabilitiesResponse {
        environment: state.config.environment.to_string(),
        provider_mode: state.config.provider_mode.to_string(),
        database,
        capabilities: state.capabilities(),
    })
}

pub async fn version_info(State(state): State<AppState>) -> Json<VersionResponse> {
    Json(VersionResponse {
        name: "hanbova-api".to_string(),
        version: state.config.app_version.clone(),
        environment: state.config.environment.to_string(),
    })
}
