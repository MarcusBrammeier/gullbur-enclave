use async_trait::async_trait;
use std::sync::LazyLock;
use wallet_plugin::{
    Account, Balance, Capability, FeeEstimate, NetworkSpec, PluginError, PluginMetadata, TxRecord,
    TxStatus, WalletPlugin,
};

use reqwest;

/// Bitcoin plugin for Gullbúr Enclave.
///
/// Implements [`WalletPlugin`] for P2WPKH (SegWit v0, BIP-84) accounts, PSBT signing,
/// and Esplora-backed balance/history/fee/broadcast. All network access can be routed
/// through a SOCKS5 proxy (e.g. Tor) via [`BtcPlugin::with_tor`].
pub struct BtcPlugin {
    /// Optional SOCKS5 proxy URL used for all outbound HTTP requests.
    socks5_proxy: Option<String>,
}

impl BtcPlugin {
    /// Create a BTC plugin with no proxy (direct network access).
    pub fn new(socks5_proxy: Option<String>) -> Self {
        Self { socks5_proxy }
    }

    /// Create a BTC plugin that routes all requests through a local Tor SOCKS port.
    pub fn with_tor(socks_port: u16) -> Self {
        Self::new(Some(format!("socks5://127.0.0.1:{socks_port}")))
    }

    /// Build an HTTP client with the configured timeout and optional proxy.
    fn build_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
        if let Some(ref proxy_url) = self.socks5_proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
        }
        builder.build()
    }
}

static BTC_NETWORKS: LazyLock<[NetworkSpec; 3]> = LazyLock::new(|| {
    [
        NetworkSpec {
            id: String::from("bitcoin"),
            name: String::from("Bitcoin"),
            symbol: String::from("BTC"),
            decimals: 8,
            is_testnet: false,
        },
        NetworkSpec {
            id: String::from("bitcoin-testnet"),
            name: String::from("Bitcoin Testnet"),
            symbol: String::from("tBTC"),
            decimals: 8,
            is_testnet: true,
        },
        NetworkSpec {
            id: String::from("bitcoin-signet"),
            name: String::from("Bitcoin Signet"),
            symbol: String::from("sBTC"),
            decimals: 8,
            is_testnet: true,
        },
    ]
});

/// Return the Esplora HTTP API base URL for the given Bitcoin network.
fn esplora_base(network: &str) -> &str {
    match network {
        "bitcoin" => "https://blockstream.info/api",
        "bitcoin-testnet" => "https://blockstream.info/testnet/api",
        "bitcoin-signet" => "https://blockstream.info/signet/api",
        _ => "https://blockstream.info/api",
    }
}

#[async_trait]
impl WalletPlugin for BtcPlugin {
    fn id(&self) -> &'static str {
        "btc"
    }
    fn name(&self) -> &'static str {
        "Bitcoin"
    }
    fn supported_networks(&self) -> &[NetworkSpec] {
        &*BTC_NETWORKS
    }

    fn plugin_metadata(&self) -> PluginMetadata {
        PluginMetadata {
            description: "Bitcoin — P2WPKH (SegWit v0) via BIP-84, PSBT signing, Esplora-backed balance & history",
            author: "Gullbúr Enclave",
            homepage: "https://github.com/MarcusBrammeier/gullbur-enclave",
            repository: "https://github.com/MarcusBrammeier/gullbur-enclave",
            min_wallet_version: "0.1.0",
            tags: &["bitcoin", "btc", "segwit", "bip84", "psbt"],
        }
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::AccountCreation,
            Capability::Signing,
            Capability::Broadcast,
            Capability::Balance,
            Capability::History,
            Capability::FeeEstimation,
            Capability::AddressValidation,
            Capability::PsbtParsing,
        ]
    }

    async fn create_account(
        &self,
        seed: &[u8],
        index: u32,
        network: &str,
    ) -> Result<Account, PluginError> {
        let btc_network = match network {
            "bitcoin" => bitcoin::Network::Bitcoin,
            "bitcoin-testnet" => bitcoin::Network::Testnet,
            "bitcoin-signet" => bitcoin::Network::Signet,
            _ => return Err(PluginError::UnsupportedNetwork(network.into())),
        };
        // BIP-84: m/84'/0'/0'/0/index (mainnet) or m/84'/1'/0'/0/index (testnet)
        let purpose: u32 = 84;
        let coin_type: u32 = match btc_network {
            bitcoin::Network::Bitcoin => 0,
            _ => 1,
        };
        let path = format!("m/{purpose}'/{coin_type}'/0'/0/{index}");

        let secp = bitcoin::secp256k1::Secp256k1::new();
        let master = bitcoin::bip32::Xpriv::new_master(btc_network, seed)
            .map_err(|e| PluginError::Internal(format!("BIP-32 master key error: {e}")))?;
        let derivation_path: bitcoin::bip32::DerivationPath =
            path.parse().map_err(|e: bitcoin::bip32::Error| {
                PluginError::Internal(format!("BIP-32 path error: {e}"))
            })?;
        let child = master
            .derive_priv(&secp, &derivation_path)
            .map_err(|e| PluginError::Internal(format!("BIP-32 derivation error: {e}")))?;

        let compressed = bitcoin::CompressedPublicKey(
            bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &child.private_key),
        );
        let address = bitcoin::Address::p2wpkh(&compressed, btc_network);

        Ok(Account {
            id: format!("btc-{network}-{index}"),
            network: network.into(),
            address: address.to_string(),
            path: Some(path),
            label: None,
            index,
        })
    }

    async fn sign_transaction(
        &self,
        tx: &[u8],
        seed: &[u8],
        account_index: u32,
        network: &str,
    ) -> Result<Vec<u8>, PluginError> {
        use bitcoin::psbt::Psbt;
        use bitcoin::sighash::SighashCache;

        // Parse the PSBT
        let mut psbt = Psbt::deserialize(tx)
            .map_err(|e| PluginError::Internal(format!("PSBT parse error: {e}")))?;

        if psbt.inputs.is_empty() {
            return Err(PluginError::Internal("PSBT has no inputs".into()));
        }
        let acct_index = account_index;

        // Determine BIP-84 network
        let btc_network = match network {
            "bitcoin" => bitcoin::Network::Bitcoin,
            "bitcoin-testnet" => bitcoin::Network::Testnet,
            "bitcoin-signet" => bitcoin::Network::Signet,
            _ => return Err(PluginError::UnsupportedNetwork(network.into())),
        };
        let coin_type: u32 = match btc_network {
            bitcoin::Network::Bitcoin => 0,
            _ => 1,
        };
        let path = format!("m/84'/{coin_type}'/0'/0/{acct_index}");
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let master = bitcoin::bip32::Xpriv::new_master(btc_network, &seed)
            .map_err(|e| PluginError::Internal(format!("BIP-32 master key error: {e}")))?;
        let derivation_path: bitcoin::bip32::DerivationPath =
            path.parse().map_err(|e: bitcoin::bip32::Error| {
                PluginError::Internal(format!("BIP-32 path error: {e}"))
            })?;
        let child = master
            .derive_priv(&secp, &derivation_path)
            .map_err(|e| PluginError::Internal(format!("BIP-32 derivation error: {e}")))?;

        // Get compressed public key matching create_account derivation
        let compressed_pubkey = bitcoin::CompressedPublicKey(
            bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &child.private_key),
        );
        let bitcoin_pubkey: bitcoin::PublicKey = compressed_pubkey.into();

        // Extract secret key bytes for signing
        let secret_bytes = child.private_key.secret_bytes();

        // Shared signing context (same account key for all inputs)
        let sighash_type = bitcoin::sighash::EcdsaSighashType::All;
        let mut sighasher = SighashCache::new(&psbt.unsigned_tx);
        let bitcoin_secp = bitcoin::secp256k1::Secp256k1::signing_only();
        let secret_key = bitcoin::secp256k1::SecretKey::from_slice(&secret_bytes)
            .map_err(|e| PluginError::Internal(format!("invalid secret key: {e}")))?;

        // Sign EVERY input that has a witness UTXO
        for i in 0..psbt.inputs.len() {
            let utxo = psbt.inputs[i].witness_utxo.as_ref().ok_or_else(|| {
                PluginError::Internal(format!("PSBT input {i} missing witness UTXO"))
            })?;

            // Compute BIP-143 sighash for this input at its proper index
            let sighash = sighasher
                .p2wpkh_signature_hash(i, &utxo.script_pubkey, utxo.value, sighash_type)
                .map_err(|e| {
                    PluginError::Internal(format!("sighash compute error for input {i}: {e}"))
                })?;

            // Sign with bitcoin's secp256k1 (handles low-S normalization for P2WPKH)
            let msg = bitcoin::secp256k1::Message::from_digest(*sighash.as_ref());
            let secp_sig = bitcoin_secp.sign_ecdsa(&msg, &secret_key);
            let der_sig = secp_sig.serialize_der().to_vec();

            // Encode signature as DER + sighash type byte
            let mut sig_bytes = der_sig;
            sig_bytes.push(sighash_type.to_u32() as u8);

            // Insert the signature + pubkey into this input's partial_sigs map
            let bitcoin_sig = bitcoin::ecdsa::Signature::from_slice(&sig_bytes)
                .map_err(|e| PluginError::Internal(format!("signature conversion error: {e}")))?;
            psbt.inputs[i]
                .partial_sigs
                .insert(bitcoin_pubkey, bitcoin_sig);
        }

        // Return the serialized signed PSBT bytes
        Ok(psbt.serialize())
    }

    async fn broadcast_transaction(&self, tx: &[u8], network: &str) -> Result<String, PluginError> {
        let base = esplora_base(network);
        let url = format!("{}/tx", base);
        let client = self.build_client().map_err(|e| {
            PluginError::BroadcastFailed(format!("Failed to build HTTP client: {e}"))
        })?;
        let body = hex::encode(tx);
        let resp = client
            .post(&url)
            .header("Content-Type", "text/plain")
            .body(body)
            .send()
            .await
            .map_err(|e| PluginError::BroadcastFailed(format!("HTTP request failed: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| PluginError::BroadcastFailed(format!("Failed to read response: {e}")))?;
        let txid = body.trim();
        // Esplora returns HTTP 200 with error text for invalid txs.
        // A valid txid is a 64-char hex string.
        if !status.is_success() || txid.len() != 64 || txid.chars().any(|c| !c.is_ascii_hexdigit())
        {
            return Err(PluginError::BroadcastFailed(txid.to_string()));
        }
        Ok(txid.to_string())
    }

    async fn get_balance(&self, account: &Account, network: &str) -> Result<Balance, PluginError> {
        let base = esplora_base(network);
        let url = format!("{}/address/{}", base, account.address);
        let client = self
            .build_client()
            .map_err(|e| PluginError::NetworkError(format!("Failed to build HTTP client: {e}")))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| PluginError::NetworkError(format!("HTTP request failed: {e}")))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PluginError::NetworkError(format!("Failed to parse address info: {e}")))?;

        let chain_funded = json["chain_stats"]["funded_txo_sum"].as_u64().unwrap_or(0);
        let chain_spent = json["chain_stats"]["spent_txo_sum"].as_u64().unwrap_or(0);
        let mempool_funded = json["mempool_stats"]["funded_txo_sum"]
            .as_u64()
            .unwrap_or(0);
        let mempool_spent = json["mempool_stats"]["spent_txo_sum"].as_u64().unwrap_or(0);

        let confirmed_sats = chain_funded.saturating_sub(chain_spent);
        let unconfirmed_sats = mempool_funded.saturating_sub(mempool_spent);

        let fmt_btc = |sats: u64| -> String {
            if sats == 0 {
                return "0".into();
            }
            let btc = sats as f64 / 100_000_000.0;
            let s = format!("{:.8}", btc);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        };

        Ok(Balance {
            confirmed: fmt_btc(confirmed_sats),
            unconfirmed: fmt_btc(unconfirmed_sats),
            unit: "BTC".into(),
        })
    }

    async fn get_transaction_history(
        &self,
        account: &Account,
        network: &str,
        limit: u32,
    ) -> Result<Vec<TxRecord>, PluginError> {
        let base = esplora_base(network);
        let url = format!("{}/address/{}/txs", base, account.address);
        let client = self
            .build_client()
            .map_err(|e| PluginError::NetworkError(format!("Failed to build HTTP client: {e}")))?;

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(vec![]),
        };

        let txs: Vec<serde_json::Value> = match resp.json().await {
            Ok(j) => j,
            Err(_) => return Ok(vec![]),
        };

        Ok(txs
            .into_iter()
            .take(limit as usize)
            .map(|tx| {
                let txid = tx["txid"].as_str().unwrap_or("").to_string();
                let from_address = tx["vin"][0]["prevout"]["scriptpubkey_address"]
                    .as_str()
                    .map(|s| s.to_string());
                let to_address = tx["vout"][0]["scriptpubkey_address"]
                    .as_str()
                    .map(|s| s.to_string());
                let amount_sats: u64 = tx["vout"]
                    .as_array()
                    .map(|vouts| vouts.iter().filter_map(|v| v["value"].as_u64()).sum())
                    .unwrap_or(0);
                let fee = tx["fee"].as_u64().map(|f| f.to_string());
                let block_height = tx["status"]["block_height"].as_u64();
                let timestamp = tx["status"]["block_time"].as_u64();
                let confirmed = tx["status"]["confirmed"].as_bool().unwrap_or(false);
                let status = if confirmed {
                    TxStatus::Confirmed {
                        confirmations: block_height.map(|_| 1u64).unwrap_or(1),
                    }
                } else {
                    TxStatus::Pending
                };

                TxRecord {
                    txid,
                    from_address,
                    to_address,
                    amount: amount_sats.to_string(),
                    fee,
                    block_height,
                    timestamp,
                    status,
                }
            })
            .collect())
    }

    async fn estimate_fee(&self, _t: &[u8], network: &str) -> Result<FeeEstimate, PluginError> {
        let base = esplora_base(network);
        let url = format!("{}/fee-estimates", base);
        let client = self
            .build_client()
            .map_err(|e| PluginError::NetworkError(format!("Failed to build HTTP client: {e}")))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| PluginError::NetworkError(format!("HTTP request failed: {e}")))?;
        let json: serde_json::Value = resp.json().await.map_err(|e| {
            PluginError::NetworkError(format!("Failed to parse fee estimates: {e}"))
        })?;

        let fast = json["1"]
            .as_f64()
            .map_or("0".to_string(), |v| format!("{:.1}", v));
        let medium = json["3"]
            .as_f64()
            .map_or("0".to_string(), |v| format!("{:.1}", v));
        let slow = json["6"]
            .as_f64()
            .map_or("0".to_string(), |v| format!("{:.1}", v));

        Ok(FeeEstimate {
            fast,
            medium,
            slow,
            unit: "sat/vB".into(),
        })
    }

    async fn validate_address(&self, addr: &str, network: &str) -> Result<bool, PluginError> {
        // Try full address validation via bitcoin crate (validates bech32 checksum,
        // Base58Check for legacy, and matches the address to its network)
        use bitcoin::address::NetworkUnchecked;
        if let Ok(unchecked) = addr.parse::<bitcoin::Address<NetworkUnchecked>>() {
            let expected = match network {
                "bitcoin" => bitcoin::Network::Bitcoin,
                "bitcoin-testnet" => bitcoin::Network::Testnet,
                "bitcoin-signet" => bitcoin::Network::Signet,
                _ => return Ok(false),
            };
            return Ok(unchecked.require_network(expected).is_ok());
        }

        // Fall back to prefix-only check for backward compatibility
        match network {
            "bitcoin" => {
                Ok(addr.starts_with("bc1") || addr.starts_with("1") || addr.starts_with("3"))
            }
            "bitcoin-testnet" | "bitcoin-signet" => Ok(addr.starts_with("tb1")
                || addr.starts_with("m")
                || addr.starts_with("n")
                || addr.starts_with("2")),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wallet_plugin::WalletPlugin;

    #[test]
    fn test_plugin_id_and_name() {
        let plugin = BtcPlugin::new(None);
        assert_eq!(plugin.id(), "btc");
        assert_eq!(plugin.name(), "Bitcoin");
    }

    #[test]
    fn test_supported_networks_count() {
        let plugin = BtcPlugin::new(None);
        assert_eq!(plugin.supported_networks().len(), 3);
    }

    #[test]
    fn test_supported_networks_data() {
        let plugin = BtcPlugin::new(None);
        let networks = plugin.supported_networks();
        let first = &networks[0];
        assert_eq!(first.id, "bitcoin");
        assert_eq!(first.symbol, "BTC");
        assert_eq!(first.decimals, 8);
        assert!(!first.is_testnet);
    }

    #[tokio::test]
    async fn test_validate_address_valid() {
        let plugin = BtcPlugin::new(None);
        let result = plugin
            .validate_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4", "bitcoin")
            .await;
        assert!(result.expect("test invariant"));
    }

    #[tokio::test]
    async fn test_validate_address_invalid() {
        let plugin = BtcPlugin::new(None);
        let result = plugin.validate_address("not-an-address", "bitcoin").await;
        assert!(!result.expect("test invariant"));
    }

    #[tokio::test]
    async fn test_validate_address_testnet() {
        let plugin = BtcPlugin::new(None);
        let result = plugin
            .validate_address(
                "tb1q7f5gpwcjvspelyu8sj9jlvt40wjlk93t4heqgk",
                "bitcoin-testnet",
            )
            .await;
        assert!(
            result.expect("test invariant"),
            "testnet bech32 address should be valid"
        );
    }

    #[tokio::test]
    async fn test_validate_address_mainnet_rejects_testnet() {
        let plugin = BtcPlugin::new(None);
        let result = plugin
            .validate_address("tb1q7f5gpwcjvspelyu8sj9jlvt40wjlk93t4heqgk", "bitcoin")
            .await;
        assert!(
            !result.expect("test invariant"),
            "testnet address should be rejected on mainnet"
        );
    }

    #[tokio::test]
    async fn test_create_account_returns_account() {
        let plugin = BtcPlugin::new(None);
        let account = plugin
            .create_account(&[0xaa; 32], 0, "bitcoin-testnet")
            .await
            .expect("test invariant");
        eprintln!("BTC_TESTNET_ADDR:{}", account.address);
        assert_eq!(account.network, "bitcoin-testnet");
        assert!(account.id.starts_with("btc-bitcoin-testnet-"));
        assert!(account.address.starts_with("tb1q") || account.address.starts_with("bc1q"));
        assert_eq!(account.index, 0, "account must carry its derivation index");
    }

    /// Regression: distinct account indices MUST yield distinct addresses.
    /// A wallet bug once computed nextIndex wrong so the 3rd+ account reused
    /// index 1; this guards the backend uniqueness invariant.
    #[tokio::test]
    async fn test_distinct_indices_yield_distinct_addresses() {
        let plugin = BtcPlugin::new(None);
        let seed = [0x42u8; 32];
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for idx in 0..8u32 {
            let account = plugin
                .create_account(&seed, idx, "bitcoin-testnet")
                .await
                .expect("test invariant");
            assert_eq!(account.index, idx);
            assert!(
                seen.insert(account.address.clone()),
                "address for index {idx} collided with an earlier account"
            );
        }
        assert_eq!(seen.len(), 8, "all 8 accounts must have unique addresses");
    }

    #[tokio::test]
    async fn test_sign_transaction_with_test_key() {
        use bitcoin::absolute::LockTime;
        use bitcoin::psbt::Psbt;
        use bitcoin::transaction::Version;
        use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness};

        let plugin = BtcPlugin::new(None);

        // Create a minimal transaction
        let unsigned_tx = bitcoin::Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::default(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(10000),
                script_pubkey: ScriptBuf::from_bytes(vec![0u8; 22]),
            }],
        };

        // Build a PSBT with a witness UTXO (P2WPKH: 0x00 0x14 + 20 zero bytes)
        let mut p2wpkh_bytes = vec![0x00, 0x14];
        p2wpkh_bytes.extend_from_slice(&[0u8; 20]);
        let psbt = Psbt {
            unsigned_tx,
            version: 0,
            xpub: Default::default(),
            proprietary: Default::default(),
            unknown: Default::default(),
            inputs: vec![bitcoin::psbt::Input {
                witness_utxo: Some(TxOut {
                    value: Amount::from_sat(50000),
                    script_pubkey: ScriptBuf::from_bytes(p2wpkh_bytes),
                }),
                ..Default::default()
            }],
            outputs: vec![Default::default()],
        };

        let psbt_bytes = psbt.serialize();
        let seed_bytes = vec![0xdeu8; 32];

        let signed = plugin
            .sign_transaction(&psbt_bytes, &seed_bytes, 0, "bitcoin")
            .await
            .expect("sign_transaction should succeed");

        // Deserialize the signed PSBT and verify it has a partial signature
        let signed_psbt = Psbt::deserialize(&signed).expect("signed PSBT should deserialize");
        assert!(
            !signed_psbt.inputs[0].partial_sigs.is_empty(),
            "partial_sigs should contain at least one signature"
        );
    }

    #[tokio::test]
    async fn test_sign_transaction_multi_input_all_inputs_signed() {
        // Validates the fix: sign_transaction must loop over EVERY input and
        // produce a partial sig for each — not just inputs[0]. Regression test
        // for the "multi-input PSBT only signed first input" bug.
        use bitcoin::absolute::LockTime;
        use bitcoin::hashes::Hash;
        use bitcoin::psbt::Psbt;
        use bitcoin::transaction::Version;
        use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness};

        let plugin = BtcPlugin::new(None);

        // Deterministic 32-byte seed so derivation is stable.
        let seed_hex = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

        // A single unsigned tx that spends TWO different UTXOs (2 inputs).
        let unsigned_tx = bitcoin::Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![
                TxIn {
                    previous_output: OutPoint::new(bitcoin::Txid::from_byte_array([1u8; 32]), 0),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::new(),
                },
                TxIn {
                    previous_output: OutPoint::new(bitcoin::Txid::from_byte_array([2u8; 32]), 1),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::new(),
                },
            ],
            output: vec![TxOut {
                value: Amount::from_sat(90000),
                script_pubkey: ScriptBuf::from_bytes(vec![0u8; 22]),
            }],
        };

        // Same P2WPKH script for both inputs.
        let mut p2wpkh_bytes = vec![0x00, 0x14];
        p2wpkh_bytes.extend_from_slice(&[0u8; 20]);

        let psbt = Psbt {
            unsigned_tx,
            version: 0,
            xpub: Default::default(),
            proprietary: Default::default(),
            unknown: Default::default(),
            inputs: vec![
                bitcoin::psbt::Input {
                    witness_utxo: Some(TxOut {
                        value: Amount::from_sat(50000),
                        script_pubkey: ScriptBuf::from_bytes(p2wpkh_bytes.clone()),
                    }),
                    ..Default::default()
                },
                bitcoin::psbt::Input {
                    witness_utxo: Some(TxOut {
                        value: Amount::from_sat(50000),
                        script_pubkey: ScriptBuf::from_bytes(p2wpkh_bytes.clone()),
                    }),
                    ..Default::default()
                },
            ],
            outputs: vec![Default::default()],
        };

        let psbt_bytes = psbt.serialize();
        let seed_bytes = hex::decode(seed_hex).expect("test invariant");
        let key = wallet_plugin::KeyHandle {
            key_id: seed_hex.into(),
            key_type: wallet_plugin::KeyType::Secp256k1,
            public_key: vec![],
        };

        let signed = plugin
            .sign_transaction(&psbt_bytes, &seed_bytes, 0, "bitcoin")
            .await
            .expect("sign_transaction should succeed");

        let signed_psbt = Psbt::deserialize(&signed).expect("signed PSBT should deserialize");

        // Critical regression assertion: BOTH inputs must carry a partial sig.
        assert!(
            !signed_psbt.inputs[0].partial_sigs.is_empty(),
            "input[0] must have a partial sig"
        );
        assert!(
            !signed_psbt.inputs[1].partial_sigs.is_empty(),
            "input[1] must have a partial sig (multi-input regressions) — was only signing inputs[0]"
        );

        // Both inputs must be signed by the SAME key (single account key).
        let key0: &bitcoin::PublicKey = signed_psbt.inputs[0]
            .partial_sigs
            .keys()
            .next()
            .expect("input[0] pubkey");
        let key1: &bitcoin::PublicKey = signed_psbt.inputs[1]
            .partial_sigs
            .keys()
            .next()
            .expect("input[1] pubkey");
        assert_eq!(
            key0, key1,
            "both inputs must be signed by the same account key"
        );
    }

    #[tokio::test]
    async fn test_sign_transaction_invalid_psbt() {
        let plugin = BtcPlugin::new(None);
        let seed_bytes = vec![0xdeu8; 32];
        let result = plugin
            .sign_transaction(b"not a valid PSBT", &seed_bytes, 0, "bitcoin")
            .await;
        assert!(result.is_err(), "passing garbage bytes should return Err");
    }
}
