use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::ApiError,
    models::{CreatePaymentIntentRequest, PaymentIntentResponse, UpdatePaymentStatusRequest},
    state::AppState,
};

pub async fn create_payment_intent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(mut payload): Json<CreatePaymentIntentRequest>,
) -> Result<(StatusCode, Json<PaymentIntentResponse>), ApiError> {
    if payload.sender_id.is_none() {
        payload.sender_id = Some(auth_user.user_id.to_string());
    }
    let response = state.payment_service.create_payment_intent(payload).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn get_payment_intent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PaymentIntentResponse>, ApiError> {
    let user_id_str = auth_user.user_id.to_string();
    let response = state
        .payment_service
        .get_payment_intent(id, Some(&user_id_str), Some(&auth_user.username))
        .await?;
    Ok(Json(response))
}

pub async fn list_payment_intents(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<PaymentIntentResponse>>, ApiError> {
    let user_id_str = auth_user.user_id.to_string();
    let response = state
        .payment_service
        .list_user_payment_intents(&user_id_str, Some(&auth_user.username))
        .await?;
    Ok(Json(response))
}

pub async fn update_payment_intent_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePaymentStatusRequest>,
) -> Result<Json<PaymentIntentResponse>, ApiError> {
    let actor_id_str = auth_user.user_id.to_string();
    let response = state
        .payment_service
        .update_payment_status(
            id,
            payload.status,
            &actor_id_str,
            Some(&auth_user.username),
        )
        .await?;
    Ok(Json(response))
}
