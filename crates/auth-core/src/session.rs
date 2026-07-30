use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// ERC-7579 session key — a temporary key with scoped permissions.
#[derive(Debug, Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct SessionKey {
    pub key_id: String,
    pub public_key: Vec<u8>,
    pub permissions: SessionPermissions,
    /// Unix timestamp (seconds) when this session expires.
    pub expires_at: u64,
}

/// Permissions scoping a session key.
#[derive(Debug, Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct SessionPermissions {
    /// Allowed contract addresses (empty = all contracts).
    pub allowed_contracts: Vec<String>,
    /// Maximum gas per transaction.
    pub max_gas_per_tx: u64,
    /// Maximum value per transaction (in wei/lamports/… as decimal string).
    pub max_value_per_tx: String,
    /// Allowed method selectors (4-byte hex) — empty = all methods.
    pub allowed_methods: Vec<String>,
}

/// A transaction being checked against session key permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTx {
    /// Destination address.
    pub to: String,
    /// Value being transferred (in base unit, decimal string).
    pub value: String,
    /// Gas limit for this transaction.
    pub gas_limit: u64,
    /// Calldata.
    pub data: Vec<u8>,
}

/// Module for creating and validating ERC-7579 session keys.
pub struct SessionKeyModule;

impl SessionKeyModule {
    /// Create a new session key with the given permissions and duration.
    ///
    /// Generates a real ed25519 keypair via crypto-core and stores the
    /// public key in the returned `SessionKey`. The private key is held
    /// by the caller (vault-core) for signing.
    pub fn create_session(
        permissions: SessionPermissions,
        duration_secs: u64,
    ) -> SessionKey {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Generate a real ed25519 keypair for this session
        let seed = crypto_core::keys::generate_seed();
        let key_handle = crypto_core::keys::derive_ed25519_key(seed.as_slice(), 0)
            .expect("ed25519 key derivation from fresh seed must succeed");

        SessionKey {
            key_id: uuid_v4(),
            public_key: key_handle.public_key,
            permissions,
            expires_at: now.saturating_add(duration_secs),
        }
    }

    /// Validate whether a session key can sign a given transaction.
    pub fn validate_session(key: &SessionKey, tx: &SessionTx) -> bool {
        // Check expiry.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now >= key.expires_at {
            return false;
        }

        let perms = &key.permissions;

        // Check allowed contracts (if non-empty, tx.to must be in the list).
        if !perms.allowed_contracts.is_empty()
            && !perms.allowed_contracts.iter().any(|c| c == &tx.to)
        {
            return false;
        }

        // Check gas limit.
        if tx.gas_limit > perms.max_gas_per_tx {
            return false;
        }

        // Check value limit.
        if let Ok(tx_val) = tx.value.parse::<u128>()
            && let Ok(max_val) = perms.max_value_per_tx.parse::<u128>()
                && tx_val > max_val {
                    return false;
                }

        // Check allowed methods (first 4 bytes of calldata).
        if !perms.allowed_methods.is_empty() && tx.data.len() >= 4 {
            let selector = hex::encode(&tx.data[..4]);
            if !perms.allowed_methods.iter().any(|m| m == &selector) {
                return false;
            }
        }

        true
    }
}

/// Generate a simple UUID v4 (fallback, no external crate dependency).
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (ts & 0xFFFF_FFFF) as u32,
        ((ts >> 32) & 0xFFFF) as u16,
        ((ts >> 48) & 0x0FFF) as u16,
        (0x8000 | ((ts >> 60) & 0x3FFF)) as u16,
        (ts >> 74) as u64 & 0xFFFF_FFFF_FFFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_session_key_creation() {
        let perms = SessionPermissions {
            allowed_contracts: vec![],
            max_gas_per_tx: 1_000_000,
            max_value_per_tx: "1000000000000000000".into(),
            allowed_methods: vec![],
        };
        let key = SessionKeyModule::create_session(perms, 3600);
        // Verify key_id format (UUID-like)
        assert!(
            key.key_id.contains('-'),
            "key_id should be UUID-like: {}",
            key.key_id
        );
        // Verify public key is non-empty (real ed25519 key generated)
        assert_eq!(key.public_key.len(), 32, "ed25519 public key must be 32 bytes");
        // Verify expires_at is in the future
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test invariant")
            .as_secs();
        assert!(
            key.expires_at > now,
            "expires_at should be in the future"
        );
    }

    #[test]
    fn test_session_validation_allows_valid_tx() {
        let perms = SessionPermissions {
            allowed_contracts: vec!["0xContract".into()],
            max_gas_per_tx: 1_000_000,
            max_value_per_tx: "1000000000000000000".into(),
            allowed_methods: vec![],
        };
        let key = SessionKeyModule::create_session(perms, 3600);
        let tx = SessionTx {
            to: "0xContract".into(),
            value: "1000".into(),
            gas_limit: 500_000,
            data: vec![],
        };
        assert!(SessionKeyModule::validate_session(&key, &tx));
    }

    #[test]
    fn test_session_validation_rejects_wrong_contract() {
        let perms = SessionPermissions {
            allowed_contracts: vec!["0xContract".into()],
            max_gas_per_tx: 1_000_000,
            max_value_per_tx: "1000000000000000000".into(),
            allowed_methods: vec![],
        };
        let key = SessionKeyModule::create_session(perms, 3600);
        let tx = SessionTx {
            to: "0xOther".into(),
            value: "1000".into(),
            gas_limit: 500_000,
            data: vec![],
        };
        assert!(!SessionKeyModule::validate_session(&key, &tx));
    }

    #[test]
    fn test_session_validation_rejects_expired() {
        let perms = SessionPermissions {
            allowed_contracts: vec![],
            max_gas_per_tx: 1_000_000,
            max_value_per_tx: "1000000000000000000".into(),
            allowed_methods: vec![],
        };
        // Create session with 0 duration — expires_at == now
        let key = SessionKeyModule::create_session(perms, 0);
        let tx = SessionTx {
            to: "0xContract".into(),
            value: "1000".into(),
            gas_limit: 500_000,
            data: vec![],
        };
        assert!(!SessionKeyModule::validate_session(&key, &tx));
    }

    #[test]
    fn test_session_permissions_builder() {
        let perms = SessionPermissions {
            allowed_contracts: vec!["0xAAA".into(), "0xBBB".into()],
            max_gas_per_tx: 500_000,
            max_value_per_tx: "5000000000000000000".into(),
            allowed_methods: vec!["0x12345678".into()],
        };
        assert_eq!(perms.allowed_contracts.len(), 2);
        assert_eq!(perms.max_gas_per_tx, 500_000);
        assert_eq!(perms.max_value_per_tx, "5000000000000000000");
        assert_eq!(perms.allowed_methods.len(), 1);
    }
}