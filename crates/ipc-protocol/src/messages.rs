//! JSON-RPC 2.0 message types.

use serde::{Deserialize, Serialize};

/// An RPC error object per the JSON-RPC 2.0 spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcError {
    /// Create a new RPC error.
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create a new RPC error with additional data.
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    // ── Standard JSON-RPC error codes ──

    /// Parse error (-32700).
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    /// Invalid request (-32600).
    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request")
    }

    /// Method not found (-32601).
    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found")
    }

    /// Invalid params (-32602).
    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params")
    }

    /// Internal error (-32603).
    pub fn internal_error() -> Self {
        Self::new(-32603, "Internal error")
    }

    // ── Application error codes (range: -32000 to -32099) ──

    /// Authentication required (-32002).
    ///
    /// The vault's authentication level is insufficient for this operation.
    /// The `data` field contains the required `AuthStatus` level.
    pub fn auth_required(required_status: &str) -> Self {
        Self::with_data(
            -32002,
            format!("Authentication required: {required_status}"),
            serde_json::json!({"required_level": required_status}),
        )
    }
}

/// A JSON-RPC 2.0 request object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// The method name to invoke.
    pub method: String,
    /// Method parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// Request ID (Number, not String, per JSON-RPC 2.0; use u64).
    pub id: u64,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
            id,
        }
    }

    /// Create a notification (no `id` field). Notifications have no response.
    /// We use `id: 0` as sentinel for notifications here — callers should
    /// be aware that 0 is technically a valid ID per the spec.
    pub fn notification(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
            id: 0,
        }
    }
}

/// A successful JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// The result of the method invocation.
    pub result: serde_json::Value,
    /// Matching request ID.
    pub id: u64,
}

impl JsonRpcResponse {
    /// Create a new successful response.
    pub fn new(result: serde_json::Value, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result,
            id,
        }
    }
}

/// A JSON-RPC 2.0 error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// The error details.
    pub error: RpcError,
    /// Matching request ID.
    pub id: u64,
}

impl JsonRpcError {
    /// Create a new error response.
    pub fn new(error: RpcError, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            error,
            id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = JsonRpcRequest::new("eth_getBalance", None, 1);
        let json = serde_json::to_string(&req).expect("test invariant");
        let parsed: JsonRpcRequest = serde_json::from_str(&json).expect("test invariant");
        assert_eq!(parsed.method, "eth_getBalance");
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.jsonrpc, "2.0");
    }

    #[test]
    fn test_response_serialization() {
        let resp = JsonRpcResponse::new(serde_json::json!("0x1a"), 1);
        let json = serde_json::to_string(&resp).expect("test invariant");
        let parsed: JsonRpcResponse = serde_json::from_str(&json).expect("test invariant");
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.result, serde_json::json!("0x1a"));
    }

    #[test]
    fn test_error_response() {
        let err = JsonRpcError::new(RpcError::method_not_found(), 1);
        let json = serde_json::to_string(&err).expect("test invariant");
        let parsed: JsonRpcError = serde_json::from_str(&json).expect("test invariant");
        assert_eq!(parsed.error.code, -32601);
        assert_eq!(parsed.id, 1);
    }
}