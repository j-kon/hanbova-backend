# ========================================================
# Hanbova Backend API - Production Multi-Stage Dockerfile
# ========================================================

# --- Builder Stage ---
FROM rust:1.84-bookworm AS builder

WORKDIR /app

# Install system build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY services ./services

# Build production release binary for hanbova-api
RUN cargo build --release --bin hanbova-api

# --- Runtime Stage ---
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime SSL certificates
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Copy compiled binary from builder stage
COPY --from=builder /app/target/release/hanbova-api /usr/local/bin/hanbova-api

# Expose API HTTP port
EXPOSE 8080

# Set environment defaults
ENV RUST_LOG=info \
    PORT=8080 \
    HOST=0.0.0.0

CMD ["hanbova-api"]
