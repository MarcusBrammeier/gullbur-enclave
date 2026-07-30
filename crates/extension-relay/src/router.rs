//! dApp → vault method router.
//!
//! Translates Ethereum JSON-RPC methods (eth_*, personal_sign) into
//! vault-core IPC method calls. Enforces the security invariant that
//! state-changing methods (eth_sendTransaction, personal_sign) always
//! require explicit user approval before execution.

use serde_json::Value;

/// Routing result: the vault method to call + approval metadata.
pub struct RoutedRequest {
    /// The vault IPC method name (e.g., "vault.create_account")
    pub vault_method: String,
    /// Parameters to pass to the vault method
    pub params: Value,
    /// Whether this request requires explicit user approval before execution
    pub requires_approval: bool,
    /// Human-readable description for the approval prompt
    pub approval_description: Option<String>,
}

/// Routes a dApp JSON-RPC method to the corresponding vault operation.
///
/// # Security
///
/// `eth_sendTransaction` and `personal_sign` always set `requires_approval: true`.
/// The caller MUST gate these behind a user confirmation before forwarding to vault-core.
pub fn route(method: &str, params: Value) -> Option<RoutedRequest> {
    match method {
        "eth_requestAccounts" => Some(RoutedRequest {
            vault_method: "vault.create_account".into(),
            params,
            requires_approval: true,
            approval_description: Some(
                "dApp is requesting access to your Ethereum accounts".into(),
            ),
        }),
        "eth_accounts" => Some(RoutedRequest {
            vault_method: "vault.status".into(),
            params,
            requires_approval: false,
            approval_description: None,
        }),
        "eth_chainId" => Some(RoutedRequest {
            vault_method: "vault.list_networks".into(),
            params,
            requires_approval: false,
            approval_description: None,
        }),
        "eth_getBalance" => Some(RoutedRequest {
            vault_method: "vault.get_balance".into(),
            params,
            requires_approval: false,
            approval_description: None,
        }),
        "eth_estimateGas" => Some(RoutedRequest {
            vault_method: "vault.estimate_fee".into(),
            params,
            requires_approval: false,
            approval_description: None,
        }),
        "eth_sendTransaction" => Some(RoutedRequest {
            vault_method: "vault.sign_transaction".into(),
            params,
            requires_approval: true,
            approval_description: Some(
                "Confirm transaction in Gullbúr Enclave".into(),
            ),
        }),
        "personal_sign" => Some(RoutedRequest {
            vault_method: "vault.sign_transaction".into(),
            params,
            requires_approval: true,
            approval_description: Some("Sign message with your wallet".into()),
        }),
        "wallet_switchEthereumChain" => Some(RoutedRequest {
            vault_method: "vault.list_networks".into(),
            params,
            requires_approval: false,
            approval_description: None,
        }),
        // ── Channel B: Next-gen vault_* API (Phase 2 stubs) ──────────────
        "vault_executeBatch" => Some(RoutedRequest {
            vault_method: "vault_executeBatch".into(),
            params,
            requires_approval: true,
            approval_description: Some("Execute batch of transactions via ERC-4337 bundler".into()),
        }),
        "vault_requestSessionKey" => Some(RoutedRequest {
            vault_method: "vault_requestSessionKey".into(),
            params,
            requires_approval: true,
            approval_description: Some("Generate ERC-7579 session key for dApp".into()),
        }),
        "vault_simulateAndSend" => Some(RoutedRequest {
            vault_method: "vault_simulateAndSend".into(),
            params,
            requires_approval: true,
            approval_description: Some("Simulate transaction and send with biometric signature".into()),
        }),
        _ => None, // unsupported method — caller should return error to dApp
    }
}

/// Returns whether a method always requires user approval.
///
/// # Security
///
/// This is the canonical list of gated methods. Any new state-changing
/// method added to `route()` MUST also be added here.
pub fn requires_approval(method: &str) -> bool {
    matches!(
        method,
        "eth_sendTransaction" | "personal_sign" | "eth_requestAccounts"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_route_all_8_methods() {
        let methods = [
            "eth_requestAccounts",
            "eth_accounts",
            "eth_chainId",
            "eth_getBalance",
            "eth_estimateGas",
            "eth_sendTransaction",
            "personal_sign",
            "wallet_switchEthereumChain",
            "vault_executeBatch",
            "vault_requestSessionKey",
            "vault_simulateAndSend",
        ];
        for m in &methods {
            let r = route(m, json!({}));
            assert!(r.is_some(), "method {m} should be routable");
        }
    }

    #[test]
    fn test_unknown_method_returns_none() {
        assert!(route("eth_unknownMethod", json!({})).is_none());
    }

    #[test]
    fn test_send_transaction_requires_approval() {
        let r = route("eth_sendTransaction", json!({"from": "0x123"})).expect("test invariant");
        assert!(r.requires_approval);
        assert!(r.approval_description.is_some());
        assert_eq!(r.vault_method, "vault.sign_transaction");
    }

    #[test]
    fn test_personal_sign_requires_approval() {
        let r = route("personal_sign", json!({"data": "0xdeadbeef"})).expect("test invariant");
        assert!(r.requires_approval);
        assert_eq!(r.vault_method, "vault.sign_transaction");
    }

    #[test]
    fn test_get_balance_no_approval() {
        let r = route("eth_getBalance", json!({"address": "0x123"})).expect("test invariant");
        assert!(!r.requires_approval);
        assert_eq!(r.vault_method, "vault.get_balance");
    }

    #[test]
    fn test_requires_approval_function() {
        assert!(requires_approval("eth_sendTransaction"));
        assert!(requires_approval("personal_sign"));
        assert!(requires_approval("eth_requestAccounts"));
        assert!(!requires_approval("eth_getBalance"));
        assert!(!requires_approval("eth_chainId"));
    }

    #[test]
    fn test_request_accounts_approval() {
        let r = route("eth_requestAccounts", json!({})).expect("test invariant");
        assert!(r.requires_approval);
        assert_eq!(r.vault_method, "vault.create_account");
    }

    #[test]
    fn test_chain_id_routing() {
        let r = route("eth_chainId", json!({})).expect("test invariant");
        assert_eq!(r.vault_method, "vault.list_networks");
        assert!(!r.requires_approval);
    }
}
