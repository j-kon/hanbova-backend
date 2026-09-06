use axum::{routing::get, Router};

use crate::{
    handlers::{get_capabilities, health_check, readiness_check, version_info},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/readiness", get(readiness_check))
        .route("/capabilities", get(get_capabilities))
        .route("/version", get(version_info))
}
