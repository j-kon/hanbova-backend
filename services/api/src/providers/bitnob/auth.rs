use hex::ToHex;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

use crate::providers::{ProviderError, ProviderResult};

type HmacSha256 = Hmac<Sha256>;

/// Generates a cryptographically random 16-byte hex-encoded nonce (32 hex characters).
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.encode_hex::<String>()
}

/// Computes the official Bitnob HMAC-SHA256 signature over the canonical message string:
/// `CLIENT_ID:TIMESTAMP:NONCE:PAYLOAD`
///
/// Returns the lower-case hex-encoded HMAC digest (64 characters).
pub fn generate_signature(
    client_id: &str,
    client_secret: &str,
    timestamp: u64,
    nonce: &str,
    payload: &str,
) -> ProviderResult<String> {
    let canonical_message = format!("{client_id}:{timestamp}:{nonce}:{payload}");
    let mut mac = HmacSha256::new_from_slice(client_secret.as_bytes())
        .map_err(|e| ProviderError::Internal(format!("HMAC initialization failed: {e}")))?;
    mac.update(canonical_message.as_bytes());
    let result = mac.finalize();
    Ok(result.into_bytes().as_slice().encode_hex::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_hmac_signature() {
        let client_id = "test-client-id-12345";
        let client_secret = "test-secret-abcdef-67890";
        let timestamp = 1719236465;
        let nonce = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
        let payload = r#"{"from_asset":"USDT","to_currency":"NGN","country":"NG","source":"offchain","amount":"1"}"#;

        let sig = generate_signature(client_id, client_secret, timestamp, nonce, payload)
            .expect("HMAC signature generation");

        assert_eq!(sig.len(), 64, "HMAC-SHA256 hex must be 64 characters");

        // Verify identical output given identical canonical message
        let sig2 = generate_signature(client_id, client_secret, timestamp, nonce, payload)
            .expect("HMAC signature generation repeat");
        assert_eq!(sig, sig2, "HMAC signature must be strictly deterministic");

        // Canonical message format check: CLIENT_ID:TIMESTAMP:NONCE:PAYLOAD
        let expected_canonical = format!("{client_id}:{timestamp}:{nonce}:{payload}");
        let mut mac = HmacSha256::new_from_slice(client_secret.as_bytes()).unwrap();
        mac.update(expected_canonical.as_bytes());
        let expected_hex = mac
            .finalize()
            .into_bytes()
            .as_slice()
            .encode_hex::<String>();
        assert_eq!(sig, expected_hex);
    }

    #[test]
    fn test_generate_nonce_length_and_randomness() {
        let nonce1 = generate_nonce();
        let nonce2 = generate_nonce();
        assert_eq!(
            nonce1.len(),
            32,
            "Nonce must be 32 hex characters (16 bytes)"
        );
        assert_ne!(nonce1, nonce2, "Two generated nonces must not be equal");
    }
}
