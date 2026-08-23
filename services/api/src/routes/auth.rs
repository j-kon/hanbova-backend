use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    auth::handlers::{
        forgot_password_handler, get_me_handler, login_handler, logout_handler, refresh_handler,
        register_handler, reset_password_handler,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler))
        .route("/auth/refresh", post(refresh_handler))
        .route("/auth/logout", post(logout_handler))
        .route("/auth/forgot-password", post(forgot_password_handler))
        .route("/auth/reset-password", post(reset_password_handler))
        .route("/me", get(get_me_handler))
}
