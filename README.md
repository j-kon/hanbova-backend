# Hanbova Backend

> **Send protected.**

The backend infrastructure and cryptographic services for the **Hanbova** payment ecosystem.

---

## Workspace Structure

```text
hanbova-backend/
├── services/
│   └── api/                         # Axum REST API server
├── crates/
│   ├── hanbova-core/                # Pure domain models (PaymentIntent, PaymentStatus, etc.)
│   ├── hanbova-protected-payments/  # Cashu CDK P2PK & timelock conditional escrow
│   └── hanbova-lightning/           # Lightning network traits and adapter boundaries
├── migrations/                      # PostgreSQL SQLx database migrations
├── infrastructure/                  # Deployment & container manifests
├── scripts/                         # Operational helper scripts
└── docker-compose.yml               # Local PostgreSQL and Cashu Nutshell mint
```

---

## Quickstart

### Prerequisites
* Rust >= 1.80 (`cargo`, `rustc`)
* Docker & Docker Compose
* Make

### Commands
```bash
# Start PostgreSQL & Cashu Mint in Docker
make dev-up

# Run API server (http://127.0.0.1:8080)
make api

# Run all tests
make test

# Run linters
make lint
```

---

## Testing

```bash
# Run unit & integration tests across workspace
cargo test --workspace

# Run clippy
cargo clippy --workspace --all-targets -- -D warnings

# Check formatting
cargo fmt --all -- --check
```

### Mint-backed claim/refund checks

The normal workspace suite skips tests requiring a running mint. CI explicitly
runs all four scenarios against a pinned Nutshell 0.20.3 FakeWallet fixture.
For the same local check:

```bash
docker compose -p hanbova-mint-test -f docker-compose.mint-test.yml up --detach --wait --wait-timeout 90
HANBOVA_TEST_MINT_PORT=3339 cargo test -p hanbova-protected-payments cdk_test::tests::test_scenario -- --ignored
# Discard only this disposable mint and its valueless fixtures after testing.
docker compose -p hanbova-mint-test -f docker-compose.mint-test.yml down
```

The fixture binds only localhost, uses a public test-only key, charges zero swap
fees, and mounts no persistent volumes. Never use it for real funds or configure
the regular app to store value there. `HANBOVA_TEST_MINT_PORT` can select another
local port; set it on both Compose and the test command. Without this variable,
the Rust integration tests retain their legacy localhost:3338 target.

The existing development mint remains pinned to 0.16.5 to avoid silently
migrating its database. That version rejects recipient claims after locktime;
it is **not compatible with the full protected-payment flow**. The new isolated
fixture passes this check but does not upgrade the existing environment.
Back up the existing mint data and key, validate migration and rollback on a
copy, and explicitly plan the upgrade before changing its pin or restarting it
on a newer image. [Nutshell 0.20.3 release notes](https://github.com/cashubtc/nutshell/releases/tag/0.20.3)
warn that the release includes database migrations.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
