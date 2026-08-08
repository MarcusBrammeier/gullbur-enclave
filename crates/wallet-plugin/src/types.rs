use serde::{Deserialize, Serialize};

/// Describes a blockchain network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSpec {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub is_testnet: bool,
}

/// An account derived from a seed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub network: String,
    pub address: String,
    pub path: Option<String>,
    pub label: Option<String>,
    /// BIP-44 account index used for derivation. Serialized so the UI can
    /// compute the next free index; defaults to 0 for legacy persisted data.
    #[serde(default)]
    pub index: u32,
}

/// Balance of an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub confirmed: String,
    pub unconfirmed: String,
    pub unit: String,
}

/// Status of a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TxStatus {
    Pending,
    Confirmed { confirmations: u64 },
    Failed { reason: String },
}

/// A transaction record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxRecord {
    pub txid: String,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub amount: String,
    pub fee: Option<String>,
    pub block_height: Option<u64>,
    pub timestamp: Option<u64>,
    pub status: TxStatus,
}

/// Fee estimate for a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub fast: String,
    pub medium: String,
    pub slow: String,
    pub unit: String,
}

/// Capabilities a plugin can advertise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Capability {
    Signing,
    Broadcast,
    Balance,
    History,
    FeeEstimation,
    AddressValidation,
    AccountCreation,
    PsbtParsing,
    Custom(String),
}

/// Metadata about a wallet plugin for discovery and registry use.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub description: &'static str,
    pub author: &'static str,
    pub homepage: &'static str,
    pub repository: &'static str,
    pub min_wallet_version: &'static str,
    pub tags: &'static [&'static str],
}

/// Key types supported by wallet plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyType {
    Secp256k1,
    Ed25519,
}

/// Handle to a key stored in the keystore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyHandle {
    pub key_id: String,
    pub key_type: KeyType,
    pub public_key: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::PluginError;

    #[test]
    fn test_network_spec_construction() {
        let spec = NetworkSpec {
            id: "bitcoin".into(),
            name: "Bitcoin".into(),
            symbol: "BTC".into(),
            decimals: 8,
            is_testnet: false,
        };
        assert_eq!(spec.id, "bitcoin");
        assert_eq!(spec.name, "Bitcoin");
        assert_eq!(spec.symbol, "BTC");
        assert_eq!(spec.decimals, 8);
        assert!(!spec.is_testnet);
    }

    #[test]
    fn test_account_construction() {
        let account = Account {
            id: "btc-mainnet-0".into(),
            network: "bitcoin".into(),
            address: "bc1qtest".into(),
            path: Some("m/84'/0'/0'/0/0".into()),
            label: None,
            index: 0,
        };
        assert_eq!(account.id, "btc-mainnet-0");
        assert_eq!(account.network, "bitcoin");
        assert_eq!(account.address, "bc1qtest");
        assert_eq!(account.path.as_deref(), Some("m/84'/0'/0'/0/0"));
    }

    #[test]
    fn test_balance_default() {
        let balance = Balance {
            confirmed: "0".into(),
            unconfirmed: "0".into(),
            unit: "BTC".into(),
        };
        assert_eq!(balance.confirmed, "0");
        assert_eq!(balance.unconfirmed, "0");
        assert_eq!(balance.unit, "BTC");
    }

    #[test]
    fn test_fee_estimate() {
        let fee = FeeEstimate {
            fast: "100".into(),
            medium: "50".into(),
            slow: "10".into(),
            unit: "sat/vB".into(),
        };
        assert_eq!(fee.fast, "100");
        assert_eq!(fee.medium, "50");
        assert_eq!(fee.slow, "10");
        assert_eq!(fee.unit, "sat/vB");
    }

    #[test]
    fn test_key_handle() {
        let handle = KeyHandle {
            key_id: "key-1".into(),
            key_type: KeyType::Secp256k1,
            public_key: vec![1, 2, 3],
        };
        assert_eq!(handle.key_id, "key-1");
        assert_eq!(handle.key_type, KeyType::Secp256k1);
        assert_eq!(handle.public_key, vec![1, 2, 3]);
    }

    #[test]
    fn test_tx_status_serialization() {
        let status = TxStatus::Pending;
        let json = serde_json::to_string(&status).expect("test invariant");
        let parsed: TxStatus = serde_json::from_str(&json).expect("test invariant");
        match parsed {
            TxStatus::Pending => {} // expected
            _ => panic!("Expected Pending after roundtrip"),
        }
    }

    #[test]
    fn test_plugin_error_display() {
        let err = PluginError::UnsupportedNetwork("test".into());
        let msg = err.to_string();
        assert!(msg.contains("unsupported network"), "message: {msg}");
        assert!(msg.contains("test"), "message: {msg}");
    }

    // ── Proptest: type serialization roundtrip ──────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_account_roundtrip(
            id in ".{1,32}", network in ".{1,20}", address in ".{1,64}",
            path_present: bool, label_present: bool,
        ) {
            let acct = Account {
                id: id.clone(), network: network.clone(), address: address.clone(),
                path: if path_present { Some("m/44'/0'/0'/0/0".into()) } else { None },
                label: if label_present { Some("My Wallet".into()) } else { None },
                index: 0,
            };
            let json = serde_json::to_string(&acct).expect("test invariant");
            let parsed: Account = serde_json::from_str(&json).expect("test invariant");
            assert_eq!(parsed.id, id);
            assert_eq!(parsed.network, network);
            assert_eq!(parsed.address, address);
            assert_eq!(parsed.path.is_some(), path_present);
            assert_eq!(parsed.label.is_some(), label_present);
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_balance_roundtrip(confirmed: u64, unconfirmed: u64, unit in ".{1,8}") {
            let bal = Balance {
                confirmed: confirmed.to_string(),
                unconfirmed: unconfirmed.to_string(),
                unit,
            };
            let json = serde_json::to_string(&bal).expect("test invariant");
            let parsed: Balance = serde_json::from_str(&json).expect("test invariant");
            assert_eq!(parsed.confirmed.parse::<u64>().expect("test invariant"), confirmed);
            assert_eq!(parsed.unconfirmed.parse::<u64>().expect("test invariant"), unconfirmed);
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_tx_record_roundtrip(txid_hex in "[0-9a-f]{64}", amount: u64) {
            let tx = TxRecord {
                txid: txid_hex.clone(),
                from_address: Some("addr1".into()),
                to_address: Some("addr2".into()),
                amount: amount.to_string(),
                fee: Some("100".into()),
                block_height: Some(12345),
                timestamp: Some(1000000),
                status: TxStatus::Pending,
            };
            let json = serde_json::to_string(&tx).expect("test invariant");
            let parsed: TxRecord = serde_json::from_str(&json).expect("test invariant");
            assert_eq!(parsed.txid, txid_hex);
            assert_eq!(parsed.amount.parse::<u64>().expect("test invariant"), amount);
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_fee_estimate_roundtrip(fast: u64, medium: u64, slow: u64) {
            let fee = FeeEstimate {
                fast: fast.to_string(),
                medium: medium.to_string(),
                slow: slow.to_string(),
                unit: "sat/vB".into(),
            };
            let json = serde_json::to_string(&fee).expect("test invariant");
            let parsed: FeeEstimate = serde_json::from_str(&json).expect("test invariant");
            assert_eq!(parsed.fast.parse::<u64>().expect("test invariant"), fast);
            assert_eq!(parsed.medium.parse::<u64>().expect("test invariant"), medium);
        }
    }
}
