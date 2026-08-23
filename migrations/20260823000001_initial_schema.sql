-- Hanbova Initial Database Schema
-- Version: 20260823000001

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    identifier VARCHAR(255) NOT NULL UNIQUE,
    display_name VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Payment Intents table
CREATE TABLE IF NOT EXISTS payment_intents (
    id UUID PRIMARY KEY,
    payment_type VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL,
    amount_sats BIGINT NOT NULL,
    sender_id VARCHAR(255),
    recipient_identifier VARCHAR(255) NOT NULL,
    description TEXT,
    expires_at TIMESTAMPTZ,
    claim_reference VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_payment_intents_status ON payment_intents(status);
CREATE INDEX IF NOT EXISTS idx_payment_intents_recipient ON payment_intents(recipient_identifier);
CREATE INDEX IF NOT EXISTS idx_payment_intents_created_at ON payment_intents(created_at DESC);

-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY,
    payment_intent_id UUID REFERENCES payment_intents(id) ON DELETE SET NULL,
    transaction_type VARCHAR(32) NOT NULL,
    amount_sats BIGINT NOT NULL,
    fee_sats BIGINT NOT NULL DEFAULT 0,
    status VARCHAR(32) NOT NULL,
    tx_hash VARCHAR(255),
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_transactions_payment_intent ON transactions(payment_intent_id);
CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON transactions(created_at DESC);
