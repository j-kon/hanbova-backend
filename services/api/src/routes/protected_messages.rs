use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::Utc;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    auth::handlers::AuthUser,
    error::{ApiError, Result},
    models::{
        AcknowledgeMessageRequest, CreateProtectedMessageRequest, ProtectedMessageResponse,
        ProtectedMessageRow, UpdatePaymentKeysRequest, UserPaymentProfileResponse,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/users/:username/payment-profile",
            get(get_payment_profile_handler),
        )
        .route("/me/payment-keys", put(update_payment_keys_handler))
        .route(
            "/protected-messages",
            post(create_protected_message_handler),
        )
        .route("/protected-messages/inbox", get(get_inbox_handler))
        .route("/protected-messages/outbox", get(get_outbox_handler))
        .route("/protected-messages/:id", get(get_message_by_id_handler))
        .route("/protected-messages/:id/ack", post(ack_message_handler))
}

/// Lookup a user's public payment profile (secp256k1 P2PK key + X25519 transport key).
pub async fn get_payment_profile_handler(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Json<UserPaymentProfileResponse>> {
    let profile = state
        .protected_message_repo
        .find_payment_profile_by_username(&username)
        .await?;

    match profile {
        Some(p) => Ok(Json(p)),
        None => Err(ApiError::NotFound(format!(
            "User '@{}' not found or has not published payment keys",
            username.strip_prefix('@').unwrap_or(&username)
        ))),
    }
}

/// Update the authenticated user's public payment & transport keys.
pub async fn update_payment_keys_handler(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<UpdatePaymentKeysRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let clean_p2pk = payload.protected_payment_pubkey.trim();
    if clean_p2pk.len() != 66 || (!clean_p2pk.starts_with("02") && !clean_p2pk.starts_with("03")) {
        return Err(ApiError::BadRequest(
            "Protected payment public key must be a 33-byte compressed secp256k1 hex string (66 chars, 02/03 prefix)".into(),
        ));
    }
    if secp256k1::PublicKey::from_str(clean_p2pk).is_err() {
        return Err(ApiError::BadRequest(
            "Invalid secp256k1 elliptic curve public key point".into(),
        ));
    }

    let clean_transport = payload.transport_encryption_pubkey.trim();
    if clean_transport.len() != 64 || hex::decode(clean_transport).is_err() {
        return Err(ApiError::BadRequest(
            "Transport encryption public key must be a 32-byte X25519 hex string (64 chars)".into(),
        ));
    }

    state
        .protected_message_repo
        .upsert_user_payment_keys(
            auth_user.user_id,
            &payload.protected_payment_pubkey,
            &payload.transport_encryption_pubkey,
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "Payment keys updated successfully",
            "username": auth_user.username,
        })),
    ))
}

/// Send an end-to-end encrypted protected message to a recipient.
pub async fn create_protected_message_handler(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<CreateProtectedMessageRequest>,
) -> Result<(StatusCode, Json<ProtectedMessageResponse>)> {
    let clean_recipient = payload
        .recipient_username
        .strip_prefix('@')
        .unwrap_or(&payload.recipient_username);

    // Look up recipient user ID
    let recipient_user = state
        .auth_service
        .find_user_by_username(clean_recipient)
        .await?;

    let recipient = recipient_user.ok_or_else(|| {
        ApiError::NotFound(format!(
            "Recipient user '@{}' does not exist",
            clean_recipient
        ))
    })?;

    let message_id = Uuid::new_v4();
    let row = ProtectedMessageRow {
        id: message_id,
        payment_intent_id: payload.payment_intent_id,
        sender_user_id: auth_user.user_id,
        recipient_user_id: recipient.id,
        sender_username: auth_user.username.clone(),
        recipient_username: clean_recipient.to_string(),
        encrypted_payload: payload.encrypted_payload,
        payload_version: payload.payload_version,
        status: "delivered".to_string(),
        created_at: Utc::now(),
        acknowledged_at: None,
    };

    state.protected_message_repo.save_message(&row).await?;

    Ok((StatusCode::CREATED, Json(row.into())))
}

/// Fetch incoming encrypted messages for the authenticated user.
pub async fn get_inbox_handler(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<ProtectedMessageResponse>>> {
    let messages = state
        .protected_message_repo
        .find_inbox_by_user_id(auth_user.user_id)
        .await?;

    let responses: Vec<ProtectedMessageResponse> = messages.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

/// Fetch outgoing encrypted messages sent by the authenticated user.
pub async fn get_outbox_handler(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<ProtectedMessageResponse>>> {
    let messages = state
        .protected_message_repo
        .find_outbox_by_user_id(auth_user.user_id)
        .await?;

    let responses: Vec<ProtectedMessageResponse> = messages.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

/// Fetch a single protected message by ID with object-level authorization (only sender or recipient).
pub async fn get_message_by_id_handler(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProtectedMessageResponse>> {
    let message = state
        .protected_message_repo
        .find_message_by_id(id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Protected message '{id}' not found")))?;

    // Object-level authorization check: user must be sender or recipient
    if message.sender_user_id != auth_user.user_id && message.recipient_user_id != auth_user.user_id
    {
        return Err(ApiError::BadRequest(
            "Unauthorized: You do not have permission to view this message".into(),
        ));
    }

    Ok(Json(message.into()))
}

/// Acknowledge a message delivery / update status (e.g. "claimed", "refunded").
pub async fn ack_message_handler(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<AcknowledgeMessageRequest>,
) -> Result<Json<serde_json::Value>> {
    let message = state
        .protected_message_repo
        .find_message_by_id(id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Protected message '{id}' not found")))?;

    // Only sender or recipient can acknowledge/update status
    if message.sender_user_id != auth_user.user_id && message.recipient_user_id != auth_user.user_id
    {
        return Err(ApiError::BadRequest(
            "Unauthorized: You do not have permission to update this message".into(),
        ));
    }

    let new_status = payload.status.unwrap_or_else(|| "acknowledged".to_string());
    state
        .protected_message_repo
        .update_message_status(id, &new_status)
        .await?;

    Ok(Json(serde_json::json!({
        "id": id,
        "status": new_status,
        "acknowledged": true,
    })))
}
