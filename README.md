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

---

## License

MIT License. See [LICENSE](LICENSE) for details.
