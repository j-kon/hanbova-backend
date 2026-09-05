use super::*;
use crate::{auth::models::RegisterRequest, config::AppConfig, models::CreatePaymentIntentRequest};
use hanbova_core::PaymentType;

async fn fixture() -> (AppState, AuthUser, AuthUser, AuthUser) {
    let state = AppState::new(
        AppConfig::from_iter([("HANBOVA_ENV", "development")]).unwrap(),
        None,
    );
    let mut users = Vec::new();
    for username in ["alice", "bob", "charlie"] {
        let response = state
            .auth_service
            .register(RegisterRequest {
                username: username.into(),
                email: format!("{username}@example.test"),
                first_name: username.into(),
                last_name: "Test".into(),
                phone: None,
                password: "StrongTestPassword2026!".into(),
            })
            .await
            .unwrap();
        users.push(AuthUser {
            user_id: response.user.id,
            username: response.user.username,
        });
    }
    (state, users[0].clone(), users[1].clone(), users[2].clone())
}

async fn intent(state: &AppState, sender: &AuthUser, recipient: &str, kind: PaymentType) -> Uuid {
    let req: CreatePaymentIntentRequest = serde_json::from_value(serde_json::json!({
        "payment_type": kind, "amount_sats": 100, "recipient_identifier": recipient,
        "sender_id": sender.user_id.to_string()
    }))
    .unwrap();
    state
        .payment_service
        .create_payment_intent(req)
        .await
        .unwrap()
        .id
}

fn payload(payment: Option<Uuid>) -> CreateProtectedMessageRequest {
    CreateProtectedMessageRequest {
        recipient_username: "@bob".into(),
        encrypted_payload: "opaque-encrypted-test-envelope".into(),
        payload_version: 1,
        payment_intent_id: payment,
        recipient_transport_key_fingerprint: None,
        recipient_p2pk_key_fingerprint: None,
        wallet_environment: Some("cashu_test".into()),
    }
}

#[tokio::test]
async fn protected_message_link_rejects_unowned_mismatched_missing_and_instant_intents() {
    let (state, alice, bob, charlie) = fixture().await;
    let unowned = intent(&state, &charlie, "@bob", PaymentType::Protected).await;
    let wrong_recipient = intent(&state, &alice, "@charlie", PaymentType::Protected).await;
    let instant = intent(&state, &alice, "@bob", PaymentType::Instant).await;
    for (id, expected) in [
        (unowned, StatusCode::FORBIDDEN),
        (wrong_recipient, StatusCode::FORBIDDEN),
        (Uuid::new_v4(), StatusCode::NOT_FOUND),
        (instant, StatusCode::BAD_REQUEST),
    ] {
        use axum::response::IntoResponse;
        let result = create_protected_message_handler(
            State(state.clone()),
            alice.clone(),
            Json(payload(Some(id))),
        )
        .await;
        let error = result.expect_err("invalid message link was accepted");
        assert_eq!(error.into_response().status(), expected);
        assert!(state
            .protected_message_repo
            .find_inbox_by_user_id(bob.user_id)
            .await
            .unwrap()
            .is_empty());
        assert!(state
            .protected_message_repo
            .find_outbox_by_user_id(alice.user_id)
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn protected_message_link_accepts_matching_handles_ids_and_unlinked_envelopes() {
    let (state, alice, bob, _) = fixture().await;
    let handle_intent = intent(&state, &alice, "@BOB", PaymentType::Protected).await;
    let id_intent = intent(
        &state,
        &alice,
        &bob.user_id.to_string(),
        PaymentType::Protected,
    )
    .await;
    for linked_id in [Some(handle_intent), Some(id_intent), None] {
        let (status, Json(message)) = create_protected_message_handler(
            State(state.clone()),
            alice.clone(),
            Json(payload(linked_id)),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(message.payment_intent_id, linked_id);
        assert_eq!(message.sender_username, "alice");
        assert_eq!(message.recipient_username, "bob");
    }
    assert_eq!(
        state
            .protected_message_repo
            .find_inbox_by_user_id(bob.user_id)
            .await
            .unwrap()
            .len(),
        3
    );
}

#[tokio::test]
async fn message_status_route_preserves_terminal_report_and_actor_permissions() {
    let (state, alice, bob, charlie) = fixture().await;
    let (_, Json(message)) =
        create_protected_message_handler(State(state.clone()), alice.clone(), Json(payload(None)))
            .await
            .unwrap();
    for (actor, status, expected) in [
        (&charlie, "claimed", StatusCode::FORBIDDEN),
        (&alice, "claimed", StatusCode::FORBIDDEN),
        (&bob, "refunded", StatusCode::FORBIDDEN),
        (&bob, "claimed", StatusCode::OK),
        (&bob, "claimed", StatusCode::OK),
        (&alice, "refunded", StatusCode::CONFLICT),
        (&bob, "acknowledged", StatusCode::CONFLICT),
    ] {
        use axum::response::IntoResponse;
        let response = ack_message_handler(
            State(state.clone()),
            actor.clone(),
            Path(message.id),
            Json(AcknowledgeMessageRequest {
                status: Some(status.into()),
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), expected);
    }
    assert_eq!(
        state
            .protected_message_repo
            .find_message_by_id(message.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "claimed"
    );
}
