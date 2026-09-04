-- Create user_payment_keys table for public wallet keys
CREATE TABLE IF NOT EXISTS user_payment_keys (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    protected_payment_pubkey VARCHAR(128) NOT NULL,
    transport_encryption_pubkey VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create protected_messages table for end-to-end encrypted envelope storage
CREATE TABLE IF NOT EXISTS protected_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_intent_id UUID,
    sender_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sender_username VARCHAR(50) NOT NULL,
    recipient_username VARCHAR(50) NOT NULL,
    encrypted_payload TEXT NOT NULL,
    payload_version INT NOT NULL DEFAULT 1,
    status VARCHAR(30) NOT NULL DEFAULT 'delivered',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at TIMESTAMPTZ
);

-- Indexes for performance and secure lookups
CREATE INDEX IF NOT EXISTS idx_user_payment_keys_user_id ON user_payment_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_protected_messages_recipient ON protected_messages(recipient_user_id, status);
CREATE INDEX IF NOT EXISTS idx_protected_messages_sender ON protected_messages(sender_user_id);
CREATE INDEX IF NOT EXISTS idx_protected_messages_created ON protected_messages(created_at DESC);
