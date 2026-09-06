-- Add wallet_environment to user_payment_keys
ALTER TABLE user_payment_keys ADD COLUMN IF NOT EXISTS wallet_environment VARCHAR(50) NOT NULL DEFAULT 'cashu_test';

-- Alter primary key to composite (user_id, wallet_environment)
ALTER TABLE user_payment_keys DROP CONSTRAINT IF EXISTS user_payment_keys_pkey;
ALTER TABLE user_payment_keys ADD PRIMARY KEY (user_id, wallet_environment);

-- Add fingerprints and wallet_environment to protected_messages
ALTER TABLE protected_messages ADD COLUMN IF NOT EXISTS recipient_transport_key_fingerprint VARCHAR(64);
ALTER TABLE protected_messages ADD COLUMN IF NOT EXISTS recipient_p2pk_key_fingerprint VARCHAR(64);
ALTER TABLE protected_messages ADD COLUMN IF NOT EXISTS wallet_environment VARCHAR(50);
