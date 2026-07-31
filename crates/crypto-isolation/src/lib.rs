/// Minimal AES-256-GCM isolation crypto — no native dependencies.
///
/// This crate exists solely so `crypto-wasm` doesn't pull in `secp256k1-sys`
/// or other native-only crates that fail to compile to wasm32-unknown-unknown.
///
/// `crypto-core` re-exports everything from this crate in its `isolation` module.
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce, aead::Aead};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Encrypted payload format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub iv_hex: String,
    pub data_b64: String,
}

/// Generate a random 256-bit AES-GCM key.
pub fn generate_key() -> [u8; 32] {
    let mut raw = [0u8; 32];
    rand::Rng::fill(&mut rand::rng(), &mut raw);
    raw
}

/// Generate a random 256-bit AES-GCM key as hex.
pub fn generate_key_hex() -> String {
    hex::encode(generate_key())
}

/// Encrypt a JSON value with AES-256-GCM.
pub fn encrypt(key: &[u8; 32], value: &Value) -> Result<EncryptedPayload, String> {
    let aes_key = Key::<Aes256Gcm>::from_iter(key.iter().copied());
    let cipher = Aes256Gcm::new(&aes_key);
    let mut iv_arr = [0u8; 12];
    rand::Rng::fill(&mut rand::rng(), &mut iv_arr);
    let nonce = Nonce::from_iter(iv_arr.iter().copied());
    let plaintext = serde_json::to_vec(value).map_err(|e| format!("Serialize failed: {e}"))?;
    let combined = cipher
        .encrypt(&nonce, plaintext.as_ref())
        .map_err(|e| format!("AES-GCM encrypt failed: {e}"))?;
    Ok(EncryptedPayload {
        iv_hex: hex::encode(iv_arr),
        data_b64: general_purpose::STANDARD.encode(&combined),
    })
}

/// Decrypt an AES-256-GCM encrypted payload.
pub fn decrypt(key: &[u8; 32], payload: &EncryptedPayload) -> Result<Value, String> {
    let aes_key = Key::<Aes256Gcm>::from_iter(key.iter().copied());
    let cipher = Aes256Gcm::new(&aes_key);
    let iv_bytes = hex::decode(&payload.iv_hex).map_err(|e| format!("Bad IV hex: {e}"))?;
    let nonce = Nonce::from_iter(iv_bytes.iter().copied());
    let combined = general_purpose::STANDARD
        .decode(&payload.data_b64)
        .map_err(|e| format!("Bad base64: {e}"))?;
    let plaintext = cipher
        .decrypt(&nonce, combined.as_ref())
        .map_err(|e| format!("AES-GCM decrypt failed: {e}"))?;
    serde_json::from_slice(&plaintext).map_err(|e| format!("JSON parse failed: {e}"))
}

/// Check if a JSON value is an encrypted payload wrapper.
pub fn is_encrypted(args: &Value) -> bool {
    args.as_object()
        .and_then(|o| o.get("__encrypted__"))
        .and_then(|v| v.as_bool())
        == Some(true)
}

/// Extract EncryptedPayload from `{ __encrypted__: true, __payload__: {...} }`.
pub fn extract_encrypted(args: &Value) -> Result<EncryptedPayload, String> {
    let obj = args.as_object().ok_or("Args is not an object")?;
    let payload_val = obj.get("__payload__").ok_or("Missing __payload__ field")?;
    serde_json::from_value(payload_val.clone()).map_err(|e| format!("Bad encrypted payload: {e}"))
}

/// Encode bytes to base64 using the `base64` crate.
pub fn encode_base64(input: &[u8]) -> String {
    general_purpose::STANDARD.encode(input)
}

/// Decode base64 string to bytes using the `base64` crate.
pub fn decode_base64(input: &str) -> Result<Vec<u8>, &'static str> {
    general_purpose::STANDARD
        .decode(input)
        .map_err(|_| "Invalid base64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key_length() {
        let key = generate_key();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let original = serde_json::json!({"method": "test", "params": {"value": 42}});
        let encrypted = encrypt(&key, &original).expect("encrypt");
        let decrypted = decrypt(&key, &encrypted).expect("decrypt");
        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_tampered_data_fails() {
        let key = generate_key();
        let original = serde_json::json!({"test": "value"});
        let encrypted = encrypt(&key, &original).expect("encrypt");
        let mut decrypted_bytes = general_purpose::STANDARD
            .decode(&encrypted.data_b64)
            .expect("test invariant");
        if let Some(b) = decrypted_bytes.last_mut() {
            *b ^= 0x01;
        }
        let mut tampered = encrypted.clone();
        tampered.data_b64 = general_purpose::STANDARD.encode(&decrypted_bytes);
        assert!(decrypt(&key, &tampered).is_err());
    }

    #[test]
    fn test_base64_roundtrip() {
        let data = b"hello world! 123";
        assert_eq!(
            decode_base64(&encode_base64(data)).expect("test invariant"),
            data
        );
    }

    // ── Proptest: AES-GCM roundtrip ───────────────────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_aes_gcm_roundtrip(
            key_byte in proptest::array::uniform32(proptest::num::u8::ANY),
            json_value in proptest::string::string_regex(".{0,64}").expect("test invariant"),
        ) {
            let key: [u8; 32] = key_byte;
            let original = serde_json::json!({"data": json_value});
            let encrypted = encrypt(&key, &original).expect("test invariant");
            let decrypted = decrypt(&key, &encrypted).expect("test invariant");
            assert_eq!(original, decrypted);
        }
    }

    // ── Proptest: AES-GCM tamper detection ────────────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_aes_gcm_tamper_detection(
            key_byte in proptest::array::uniform32(proptest::num::u8::ANY),
            value in proptest::string::string_regex(".{1,32}").expect("test invariant"),
        ) {
            let key: [u8; 32] = key_byte;
            let original = serde_json::json!({"value": value});
            let encrypted = encrypt(&key, &original).expect("test invariant");

            // Corrupt the IV (flip a bit in the raw 12-byte IV, not the hex string)
            let iv_bytes = hex::decode(&encrypted.iv_hex).expect("test invariant");
            let mut corrupted_iv = iv_bytes;
            if let Some(b) = corrupted_iv.last_mut() {
                *b ^= 0xff;
            }
            let tampered_iv = EncryptedPayload {
                iv_hex: hex::encode(corrupted_iv),
                data_b64: encrypted.data_b64.clone(),
            };
            assert!(decrypt(&key, &tampered_iv).is_err());

            // Corrupt the ciphertext
            let mut corrupted_data = general_purpose::STANDARD.decode(&encrypted.data_b64).expect("test invariant");
            if !corrupted_data.is_empty() {
                corrupted_data[0] ^= 0xff;
            }
            let tampered_data = EncryptedPayload {
                iv_hex: encrypted.iv_hex.clone(),
                data_b64: general_purpose::STANDARD.encode(&corrupted_data),
            };
            assert!(decrypt(&key, &tampered_data).is_err());
        }
    }
}
