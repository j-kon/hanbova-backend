use axum::{routing::get, Router};

use crate::{
    handlers::{health_check, version_info},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/version", get(version_info))
}
