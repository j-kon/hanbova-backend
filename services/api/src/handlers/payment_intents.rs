use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::{
        ClaimIntentRequest, CreatePaymentIntentRequest, PaymentIntentResponse, RefundIntentRequest,
    },
    state::AppState,
};

pub async fn create_payment_intent(
    State(state): State<AppState>,
    Json(payload): Json<CreatePaymentIntentRequest>,
) -> Result<(StatusCode, Json<PaymentIntentResponse>), ApiError> {
    let response = state.payment_service.create_payment_intent(payload).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn get_payment_intent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PaymentIntentResponse>, ApiError> {
    let response = state.payment_service.get_payment_intent(id).await?;
    Ok(Json(response))
}

pub async fn list_payment_intents(
    State(state): State<AppState>,
) -> Result<Json<Vec<PaymentIntentResponse>>, ApiError> {
    let response = state.payment_service.list_payment_intents().await?;
    Ok(Json(response))
}

pub async fn claim_payment_intent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ClaimIntentRequest>,
) -> Result<Json<PaymentIntentResponse>, ApiError> {
    let response = state
        .payment_service
        .claim_payment_intent(id, payload)
        .await?;
    Ok(Json(response))
}

pub async fn refund_payment_intent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RefundIntentRequest>,
) -> Result<Json<PaymentIntentResponse>, ApiError> {
    let response = state
        .payment_service
        .refund_payment_intent(id, payload)
        .await?;
    Ok(Json(response))
}
