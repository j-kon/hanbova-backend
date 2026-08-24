use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use hanbova_core::CoreError;
use hanbova_protected_payments::ProtectedPaymentError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Core domain error: {0}")]
    Core(#[from] CoreError),

    #[error("Protected payment error: {0}")]
    ProtectedPayment(#[from] ProtectedPaymentError),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Validation error: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            ApiError::Core(e) => (
                StatusCode::BAD_REQUEST,
                "CORE_VALIDATION_ERROR",
                e.to_string(),
            ),
            ApiError::ProtectedPayment(e) => match e {
                ProtectedPaymentError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg),
                ProtectedPaymentError::PaymentExpired(exp) => (
                    StatusCode::GONE,
                    "PAYMENT_EXPIRED",
                    format!("Payment expired at {exp}"),
                ),
                ProtectedPaymentError::InvalidClaimProof(msg) => {
                    (StatusCode::UNAUTHORIZED, "INVALID_CLAIM_PROOF", msg)
                }
                other => (
                    StatusCode::BAD_REQUEST,
                    "PROTECTED_PAYMENT_ERROR",
                    other.to_string(),
                ),
            },
            ApiError::Database(e) => {
                tracing::error!("Database query failure: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DATABASE_ERROR",
                    "A database error occurred".to_string(),
                )
            }
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg),
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_SERVER_ERROR",
                    "An unexpected error occurred".to_string(),
                )
            }
        };

        let body = Json(ApiErrorResponse {
            error: error_type.to_string(),
            message,
            details: None,
        });

        (status, body).into_response()
    }
}
