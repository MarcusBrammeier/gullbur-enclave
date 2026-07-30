//! Chrome/Firefox Native Messaging host.
//!
//! Implements the stdio-based JSON protocol: 4-byte LE length prefix
//! followed by UTF-8 JSON message bytes. Validates extension origins.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A native message received from the browser extension.
#[derive(Debug, Deserialize)]
pub struct NativeMessage {
    /// The extension origin (chrome-extension://<id> or moz-extension://<id>)
    pub origin: String,
    /// The JSON-RPC method from the dApp
    pub method: String,
    /// Optional method parameters
    #[serde(default)]
    pub params: Option<Value>,
}

/// A native message sent back to the browser extension.
#[derive(Debug, Serialize)]
pub struct NativeResponse {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<NativeError>,
}

#[derive(Debug, Serialize)]
pub struct NativeError {
    pub code: i32,
    pub message: String,
}

/// Security: validates an extension origin.
/// Accepts chrome-extension://<id> and moz-extension://<id>.
pub fn validate_origin(origin: &str, allowed_ids: &[String]) -> bool {
    if origin.is_empty() {
        return false;
    }
    for id in allowed_ids {
        let chrome = format!("chrome-extension://{id}");
        let moz = format!("moz-extension://{id}");
        if origin == chrome || origin == moz {
            return true;
        }
    }
    false
}

/// Read a 4-byte length prefix + JSON message from stdin.
/// Returns None on EOF.
pub async fn read_message() -> Option<NativeMessage> {
    let mut stdin = tokio::io::stdin();
    let mut len_buf = [0u8; 4];
    match stdin.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
        Err(e) => {
            tracing::error!("Failed to read length prefix: {e}");
            return None;
        }
    }

    let msg_len = u32::from_le_bytes(len_buf) as usize;
    // Reject unreasonably large messages (>16MB)
    if msg_len > 16 * 1024 * 1024 {
        tracing::error!("Native message too large: {msg_len} bytes");
        return None;
    }

    let mut msg_buf = vec![0u8; msg_len];
    if let Err(e) = stdin.read_exact(&mut msg_buf).await {
        tracing::error!("Failed to read message body: {e}");
        return None;
    }

    match serde_json::from_slice(&msg_buf) {
        Ok(msg) => Some(msg),
        Err(e) => {
            tracing::error!("Failed to parse native message: {e}");
            None
        }
    }
}

/// Write a 4-byte length prefix + JSON response to stdout.
pub async fn write_response(response: &NativeResponse) {
    let json = match serde_json::to_vec(response) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to serialize response: {e}");
            return;
        }
    };
    let len = (json.len() as u32).to_le_bytes();

    let mut stdout = tokio::io::stdout();
    if let Err(e) = stdout.write_all(&len).await {
        tracing::error!("Failed to write length prefix: {e}");
        return;
    }
    if let Err(e) = stdout.write_all(&json).await {
        tracing::error!("Failed to write response body: {e}");
    }
    let _ = stdout.flush().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_origin() {
        assert!(!validate_origin("", &["abc".into()]));
    }

    #[test]
    fn test_validate_chrome_origin() {
        let ids = vec!["abcdefghijklmnop".into()];
        assert!(validate_origin("chrome-extension://abcdefghijklmnop", &ids));
    }

    #[test]
    fn test_validate_moz_origin() {
        let ids = vec!["myext".into()];
        assert!(validate_origin("moz-extension://myext", &ids));
    }

    #[test]
    fn test_reject_unknown_origin() {
        let ids = vec!["trusted".into()];
        assert!(!validate_origin("chrome-extension://evil", &ids));
    }

    #[test]
    fn test_reject_http_origin() {
        let ids = vec!["trusted".into()];
        assert!(!validate_origin("https://evil.com", &ids));
    }
}
