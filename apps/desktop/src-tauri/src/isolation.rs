/// Gullbúr Enclave — Desktop Isolation Pattern State
///
/// Manages the per-session AES-256-GCM key lifecycle for the Tauri v2
/// Isolation Pattern. The actual encrypt/decrypt logic lives in
/// `crypto-core::isolation` — this module owns the state management
/// (key generation, key_sent flag, IV counter) and the Tauri-specific
/// helper functions (maybe_decrypt_args, maybe_encrypt_response).
///
/// The sandboxed isolation iframe receives the key via __get_isolation_key.
/// The main window's Svelte code NEVER has access to the key.
use crypto_core::isolation;
pub use crypto_core::isolation::EncryptedPayload;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct IsolationState {
    pub key_hex: String,
    key_bytes: [u8; 32],
    pub key_sent: Arc<AtomicBool>,
}

impl IsolationState {
    /// Generate a fresh per-session AES-256-GCM key.
    pub fn new() -> Self {
        let key = isolation::generate_key();
        let key_hex = hex::encode(key);
        Self {
            key_hex,
            key_bytes: key,
            key_sent: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn decrypt(&self, payload: &EncryptedPayload) -> Result<Value, String> {
        isolation::decrypt(&self.key_bytes, payload)
    }

    pub async fn encrypt(&self, value: &Value) -> Result<EncryptedPayload, String> {
        isolation::encrypt(&self.key_bytes, value)
    }
}

/// Encrypt a response for the isolation iframe.
pub async fn maybe_encrypt_response(isolation: &IsolationState, response: &Value) -> Value {
    if !isolation.key_sent.load(Ordering::Relaxed) {
        return response.clone();
    }
    match isolation.encrypt(response).await {
        Ok(encrypted) => serde_json::json!({
            "__encrypted__": true,
            "__payload__": encrypted,
        }),
        Err(e) => {
            tracing::error!("[isolation] Response encrypt failed: {e}");
            response.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_generation() {
        let state = IsolationState::new();
        assert_eq!(state.key_hex.len(), 64);
        assert!(hex::decode(&state.key_hex).is_ok());
    }

    #[tokio::test]
    async fn test_encrypt_decrypt_roundtrip() {
        let state = IsolationState::new();
        let original = serde_json::json!({"method": "test", "params": {"value": 42}});
        let encrypted = state.encrypt(&original).await.expect("test invariant");
        let decrypted = state.decrypt(&encrypted).await.expect("test invariant");
        assert_eq!(original, decrypted);
    }

    #[tokio::test]
    async fn test_tampered_data_fails() {
        let state = IsolationState::new();
        let original = serde_json::json!({"test": "value"});
        let mut encrypted = state.encrypt(&original).await.expect("test invariant");
        let mut bytes = isolation::decode_base64(&encrypted.data_b64).expect("test invariant");
        if let Some(b) = bytes.last_mut() {
            *b ^= 0x01;
        }
        encrypted.data_b64 = isolation::encode_base64(&bytes);
        assert!(state.decrypt(&encrypted).await.is_err());
    }
}
