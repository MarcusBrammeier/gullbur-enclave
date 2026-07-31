//! EIP-6963: Multi Injected Provider Discovery types.
//!
//! These types implement the schemas defined in EIP-6963 for announcing
//! and requesting injected wallet providers in browser environments.

use serde::{Deserialize, Serialize};

/// Information about an injected wallet provider (per EIP-6963).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eip6963ProviderInfo {
    /// A globally unique identifier for the wallet provider.
    pub uuid: String,
    /// Human-readable name of the wallet.
    pub name: String,
    /// URI pointing to the wallet's icon (data URI or URL).
    pub icon: String,
    /// Reverse DNS identifier (e.g., "io.gullbur.wallet").
    pub rdns: String,
}

impl Eip6963ProviderInfo {
    /// Create a new provider info instance.
    pub fn new(
        uuid: impl Into<String>,
        name: impl Into<String>,
        icon: impl Into<String>,
        rdns: impl Into<String>,
    ) -> Self {
        Self {
            uuid: uuid.into(),
            name: name.into(),
            icon: icon.into(),
            rdns: rdns.into(),
        }
    }
}

/// A request dispatched via EIP-6963.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eip6963Request {
    /// The JSON-RPC method to invoke.
    pub method: String,
    /// Method parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl Eip6963Request {
    /// Create a new EIP-6963 request.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            method: method.into(),
            params,
        }
    }
}

/// EIP-6963 announce event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eip6963AnnounceEvent {
    /// The provider info for the announcing wallet.
    pub info: Eip6963ProviderInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_info_roundtrip() {
        let info = Eip6963ProviderInfo::new(
            "abc-123-def",
            "Gullbúr Enclave",
            "data:image/svg+xml,...",
            "io.gullbur.wallet",
        );

        let json = serde_json::to_string(&info).expect("test invariant");
        let parsed: Eip6963ProviderInfo = serde_json::from_str(&json).expect("test invariant");

        assert_eq!(parsed.uuid, "abc-123-def");
        assert_eq!(parsed.name, "Gullbúr Enclave");
        assert_eq!(parsed.rdns, "io.gullbur.wallet");
    }

    #[test]
    fn test_request_roundtrip() {
        let req = Eip6963Request::new(
            "eth_requestAccounts",
            Some(serde_json::json!({"chainId": "0x1"})),
        );

        let json = serde_json::to_string(&req).expect("test invariant");
        let parsed: Eip6963Request = serde_json::from_str(&json).expect("test invariant");

        assert_eq!(parsed.method, "eth_requestAccounts");
        assert!(parsed.params.is_some());
    }
}
