# Payment coordination safety implementation plan

**Goal:** Prevent stale payment updates, terminal message-state reversal, and unauthorized payment/message association.

**Architecture:** Keep actor authorization in the service/route layer. Validate state changes while holding a PostgreSQL row lock or the in-memory write lock, so validation and persistence cannot race. Coordination remains client-reported metadata, not mint settlement proof.

**Tech stack:** Rust, Axum, Tokio, SQLx/PostgreSQL; no new dependencies or schema migration.

**Spec:** `hanbova-docs/verification/2026-09-05_PRODUCT_UX_BACKEND_AUDIT.md`, remaining backend ownership/status work, authorized by the user's continuation.

## Constraints

- Preserve existing uncommitted work and mobile API payloads.
- Do not run migrations on existing user databases, deploy, or perform real payments.
- Keep unlinked encrypted messages supported; validate every provided payment-intent ID.
- Do not change Cashu claim/refund eligibility semantics or claim that coordination verifies mint settlement.

## Task 1 — Atomic payment state updates

- [x] In `services/api/src/repositories/payment_intent_repo.rs`, add a shared test that seeds `RefundAvailable`, races `Claimed` and `Refunded`, and asserts exactly one succeeds. Reject backward transitions and missing IDs. Run against memory first, then an isolated PostgreSQL database.
- [x] Replace unconditional repository status assignment with transition validation under `SELECT ... FOR UPDATE` / `RwLock::write`. Return the stored update timestamp; preserve it on idempotent retries.
- [x] In `services/api/src/services/payment_service.rs`, keep authorization, call the atomic status method instead of upserting a stale intent, and use its timestamp in the response.
- [x] Run `cargo test -p hanbova-api payment_status` and the isolated PostgreSQL variant.

## Task 2 — Monotonic protected-message acknowledgements

- [x] In `services/api/src/repositories/protected_message_repo.rs`, test `delivered -> acknowledged -> claimed`, idempotent retry, and rejected `claimed -> refunded/acknowledged/delivered`. Race terminal outcomes and assert one winner.
- [x] Apply the same transition rules atomically to PostgreSQL and memory: delivered may advance, acknowledged cannot become delivered, claimed/refunded may only repeat themselves. Keep existing route-specific actor checks.
- [x] Add a conflict response in `services/api/src/error.rs`; invalid transitions must not mutate status or first-ack timestamp.
- [x] Run `cargo test -p hanbova-api message_status` and isolated PostgreSQL coverage.

## Task 3 — Validate message/payment association

- [x] Add route-level tests in `services/api/src/routes/protected_messages.rs`: third-party intent, wrong recipient, missing intent, instant intent, and valid protected intent; verify rejected requests do not enter inbox/outbox.
- [x] Add `PaymentService::validate_protected_message_link` using the stored intent's sender, recipient and payment type. Call it before `save_message` when an intent ID is supplied, resolving the recipient to its canonical user first.
- [x] Retain the optional unlinked-message path and support UUID/handle recipient identifiers already accepted by the app.
- [x] Run full workspace tests, formatting and Clippy. Extend CI's explicit PostgreSQL test filter to cover all isolated repository regressions. Document results and limitations; request bounded read-only review before handoff.
