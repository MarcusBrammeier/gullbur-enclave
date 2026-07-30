//! ERC-7715 Session Key Policy Engine — Rust structs.
//!
//! Defines the policy object that binds a Session Key to strict
//! four-dimensional constraints enforced at the blockchain level:
//!
//! 1. **Target Scope** — which contracts/dApps are permitted
//! 2. **Function Scope** — which method selectors may be called
//! 3. **Financial Scope** — maximum token allowance (value + rate-limit)
//! 4. **Temporal Scope** — absolute expiry timestamp
//!
//! ## Architecture
//!
//! ```text
//! User Wallet (Vault)
//!   │
//!   │  wallet_grantPermissions → ERC7715Policy
//!   ▼
//! SessionKeyManager
//!   ├─ issues SessionKeyPair
//!   ├─ binds Policy to key
//!   └─ hands to MCP Server
//! ```

use serde::{Deserialize, Serialize};

// ── Target Scope ──────────────────────────────────────────────────────────

/// Which contracts / addresses the session key is allowed to interact with.
///
/// `None` = any contract (wide-open, use with caution).
/// `Some(vec![])` = no contracts permitted (revoke-only key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetScope {
    /// Whitelist of permitted contract addresses (20-byte hex, 0x-prefixed).
    /// Empty = send-only (no contract calls). None = no restriction.
    pub allowed_contracts: Option<Vec<String>>,
}

// ── Function Scope ────────────────────────────────────────────────────────

/// Which function selectors / ABI methods the session key may invoke on
/// permitted contracts.
///
/// `None` = any function (wide-open). Only meaningful when a contract
/// whitelist is also provided.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionScope {
    /// Allowed 4-byte method selectors (8 hex chars, 0x-prefixed).
    /// e.g. `["0x23b872dd"]` for `transferFrom`.
    pub allowed_selectors: Option<Vec<String>>,
}

// ── Financial Scope ───────────────────────────────────────────────────────

/// Per-token financial cap enforced on every transfer the session key signs.
///
/// ## Rate-limiting
///
/// The `period_seconds` + `max_amount` pair define a sliding-window budget:
/// the key may spend at most `max_amount` of the token per `period_seconds`.
/// This prevents a compromised agent from draining the wallet in one shot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAllowance {
    /// Token address (0x-prefixed hex). Use `0x0000...0000` for native ETH.
    pub token: String,
    /// Maximum spendable amount (decimal string, in the token's smallest unit).
    pub max_amount: String,
    /// Rolling window length in seconds. 0 = no rate limit (single-shot cap only).
    pub period_seconds: u64,
    /// Total lifetime cap across all periods. 0 = unlimited (bounded by max_amount per period).
    pub lifetime_cap: String,
}

/// The full financial constraint set.
///
/// Multiple `TokenAllowance` entries let the policy author grant access to
/// multiple tokens (e.g. 50 USDC/day + 0.1 ETH/day) independently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialScope {
    pub allowances: Vec<TokenAllowance>,
}

// ── Temporal Scope ────────────────────────────────────────────────────────

/// When the session key expires. After this Unix timestamp, the key is
/// invalid at the blockchain level — no transaction signed by it will
/// be accepted by an ERC-7715-compliant bundler or relayer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalScope {
    /// Unix timestamp (seconds). Use `std::time::SystemTime::now()
    /// + Duration::from_secs(3600)` for one hour from now.
    pub expires_at: u64,
    /// Optional "not valid before" timestamp for scheduled delegation.
    pub not_valid_before: Option<u64>,
}

// ── Top-Level Policy Object ───────────────────────────────────────────────

/// The complete ERC-7715 session key policy, as serialized into the
/// `wallet_grantPermissions` response and stored on-chain by the bundler.
///
/// ## Serialization
///
/// Normal serialization to JSON via serde. The Vault stores this alongside
/// the session key public key so it can verify compliance before signing
/// any x402 authorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ERC7715Policy {
    /// Human-readable label for the policy (shown in wallet UIs).
    pub label: String,
    /// Unique policy ID (UUID v4, generated at creation time).
    pub policy_id: String,
    /// Target contracts.
    pub target: TargetScope,
    /// Allowed functions.
    pub functions: FunctionScope,
    /// Token allowances.
    pub financial: FinancialScope,
    /// Expiry.
    pub temporal: TemporalScope,
    /// Optional: which bundler/relayer endpoints are authorised to forward
    /// UserOps signed by this key.
    pub authorised_relayers: Option<Vec<String>>,
    /// Optional: EIP-712 domain separator overrides (default = wallet domain).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_override: Option<serde_json::Value>,
}

impl ERC7715Policy {
    /// Validate the policy is internally consistent.
    /// Returns `Ok(())` on success, `Err` with a description of the problem.
    pub fn validate(&self) -> Result<(), String> {
        // Temporal: expiry must be in the future-ish
        if self.temporal.expires_at == 0 {
            return Err("expires_at must be set".into());
        }

        // Financial: at least one allowance must exist
        if self.financial.allowances.is_empty() {
            return Err("financial scope must have at least one allowance".into());
        }

        // Target: if contracts are listed, each must look like a 20-byte address
        if let Some(ref contracts) = self.target.allowed_contracts {
            for addr in contracts {
                if !addr.starts_with("0x") || addr.len() != 42 {
                    return Err(format!("Invalid contract address: {addr}"));
                }
            }
        }

        // Function: if selectors are listed, each must be 4 bytes (10 hex chars including 0x)
        if let Some(ref selectors) = self.functions.allowed_selectors {
            for sel in selectors {
                if !sel.starts_with("0x") || sel.len() != 10 {
                    return Err(format!("Invalid function selector: {sel}"));
                }
            }
        }

        Ok(())
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_validate_valid() {
        let policy = ERC7715Policy {
            label: "test".into(),
            policy_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            target: TargetScope {
                allowed_contracts: Some(vec!["0x1234567890123456789012345678901234567890".into()]),
            },
            functions: FunctionScope {
                allowed_selectors: Some(vec!["0x23b872dd".into()]),
            },
            financial: FinancialScope {
                allowances: vec![TokenAllowance {
                    token: "0x0000000000000000000000000000000000000000".into(),
                    max_amount: "50000000".into(),
                    period_seconds: 86400,
                    lifetime_cap: "0".into(),
                }],
            },
            temporal: TemporalScope {
                expires_at: 9999999999,
                not_valid_before: None,
            },
            authorised_relayers: None,
            domain_override: None,
        };
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_policy_validate_no_expiry() {
        let policy = ERC7715Policy {
            label: "test".into(),
            policy_id: "x".into(),
            target: TargetScope { allowed_contracts: None },
            functions: FunctionScope { allowed_selectors: None },
            financial: FinancialScope { allowances: vec![TokenAllowance {
                token: "0x0".into(),
                max_amount: "100".into(),
                period_seconds: 3600,
                lifetime_cap: "0".into(),
            }]},
            temporal: TemporalScope { expires_at: 0, not_valid_before: None },
            authorised_relayers: None,
            domain_override: None,
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_policy_validate_invalid_address() {
        let policy = ERC7715Policy {
            label: "test".into(),
            policy_id: "y".into(),
            target: TargetScope {
                allowed_contracts: Some(vec!["not-an-address".into()]),
            },
            functions: FunctionScope { allowed_selectors: None },
            financial: FinancialScope { allowances: vec![TokenAllowance {
                token: "0x0".into(),
                max_amount: "100".into(),
                period_seconds: 3600,
                lifetime_cap: "0".into(),
            }]},
            temporal: TemporalScope { expires_at: 1, not_valid_before: None },
            authorised_relayers: None,
            domain_override: None,
        };
        assert!(policy.validate().is_err());
    }
}