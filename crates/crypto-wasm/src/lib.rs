/// Gullbúr Enclave — WASM Crypto Wrapper
///
/// Compiles the shared AES-256-GCM crypto from crypto-core::isolation
/// into a WASM blob loadable by:
/// - The Tauri v2 isolation iframe (desktop)
/// - The IpcClient.ts WebSocket client (browser extension)
///
/// ## Building
/// ```bash
/// cd crates/crypto-wasm
/// wasm-pack build --target web --out-name crypto_wasm
/// # Outputs: pkg/crypto_wasm.js + crypto_wasm_bg.wasm
/// ```

use crypto_isolation as isolation;
use wasm_bindgen::prelude::*;

/// Encrypt a JSON string with AES-256-GCM.
///
/// - `key_hex`: 64-char hex string (32 bytes raw)
/// - `json_data`: any JSON-serializable string
///
/// Returns a JSON string: `{"iv_hex": "...", "data_b64": "..."}`
#[wasm_bindgen]
pub fn encrypt(key_hex: &str, json_data: &str) -> Result<String, JsValue> {
    let key_bytes = hex::decode(key_hex)
        .map_err(|e| JsValue::from_str(&format!("Bad key hex: {e}")))?;
    if key_bytes.len() != 32 {
        return Err(JsValue::from_str("Key must be 32 bytes"));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes);

    let value: serde_json::Value = serde_json::from_str(json_data)
        .map_err(|e| JsValue::from_str(&format!("Bad JSON: {e}")))?;

    let payload = isolation::encrypt(&key, &value)
        .map_err(|e| JsValue::from_str(&e))?;

    serde_json::to_string(&payload)
        .map_err(|e| JsValue::from_str(&format!("Serialize failed: {e}")))
}

/// Decrypt an AES-256-GCM encrypted payload.
///
/// - `key_hex`: 64-char hex string (32 bytes raw)
/// - `payload_json`: JSON string `{"iv_hex": "...", "data_b64": "..."}`
///
/// Returns the original JSON string.
#[wasm_bindgen]
pub fn decrypt(key_hex: &str, payload_json: &str) -> Result<String, JsValue> {
    let key_bytes = hex::decode(key_hex)
        .map_err(|e| JsValue::from_str(&format!("Bad key hex: {e}")))?;
    if key_bytes.len() != 32 {
        return Err(JsValue::from_str("Key must be 32 bytes"));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes);

    let payload: isolation::EncryptedPayload = serde_json::from_str(payload_json)
        .map_err(|e| JsValue::from_str(&format!("Bad payload JSON: {e}")))?;

    let value = isolation::decrypt(&key, &payload)
        .map_err(|e| JsValue::from_str(&e))?;

    serde_json::to_string(&value)
        .map_err(|e| JsValue::from_str(&format!("Serialize failed: {e}")))
}

/// Generate a random AES-256-GCM key.
/// Returns a 64-char hex string.
#[wasm_bindgen]
pub fn generate_key() -> String {
    isolation::generate_key_hex()
}