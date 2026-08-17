use async_trait::async_trait;
use bitcoin::hashes::Hash;
use std::sync::LazyLock;
use wallet_plugin::{
    Account, Balance, Capability, FeeEstimate, NetworkSpec, PluginError, PluginMetadata, TxRecord,
    WalletPlugin,
};

pub struct LtcPlugin;

impl LtcPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LtcPlugin {
    fn default() -> Self {
        Self::new()
    }
}

static LTC_NETWORKS: LazyLock<[NetworkSpec; 4]> = LazyLock::new(|| {
    [
        NetworkSpec {
            id: "litecoin".into(),
            name: "Litecoin".into(),
            symbol: "LTC".into(),
            decimals: 8,
            is_testnet: false,
        },
        NetworkSpec {
            id: "litecoin-testnet3".into(),
            name: "Litecoin Testnet3".into(),
            symbol: "tLTC".into(),
            decimals: 8,
            is_testnet: true,
        },
        NetworkSpec {
            id: "litecoin-testnet4".into(),
            name: "Litecoin Testnet4".into(),
            symbol: "tLTC".into(),
            decimals: 8,
            is_testnet: true,
        },
        // Backward-compat alias: "litecoin-testnet" resolves to testnet3.
        NetworkSpec {
            id: "litecoin-testnet".into(),
            name: "Litecoin Testnet (alias → testnet3)".into(),
            symbol: "tLTC".into(),
            decimals: 8,
            is_testnet: true,
        },
    ]
});

fn esplora_base(network: &str) -> &str {
    match network {
        "litecoin" => "https://litecoin.mempool.space/api",
        // testnet3 JSON-RPC (the network the key-index 0 testnet address lives on).
        "litecoin-testnet3" | "litecoin-testnet" => "https://litecoin.mempool.space/testnet/api",
        "litecoin-testnet4" => "https://litecoin.mempool.space/testnet4/api",
        _ => "https://litecoin.mempool.space/api",
    }
}

/// Generate a properly checksummed Litecoin segwit address from a compressed public key.
///
/// # Why this exists
///
/// Bitcoin and Litecoin share the same segwit witness program (P2WPKH = HASH160 of compressed
/// pubkey = 20 bytes, witness version 0), but differ in the bech32 HRP:
/// - Bitcoin mainnet: `bc`, testnet: `tb`
/// - Litecoin mainnet: `ltc`, testnet: `tltc`
///
/// Simply doing a string replacement of `bc1`→`ltc1` or `tb1`→`tltc1` produces an *invalid*
/// checksum because the bech32 checksum includes the HRP. This function computes the correct
/// checksum by encoding the witness program with the proper Litecoin HRP from scratch.
fn ltc_p2wpkh_address(
    compressed: &bitcoin::CompressedPublicKey,
    network: &str,
) -> Result<String, PluginError> {
    let hrp_str = match network {
        "litecoin" => "ltc",
        "litecoin-testnet3" | "litecoin-testnet4" | "litecoin-testnet" => "tltc",
        _ => return Err(PluginError::UnsupportedNetwork(network.into())),
    };
    let hrp = bech32::Hrp::parse_unchecked(hrp_str);

    // P2WPKH witness program: version 0, HASH160 of the compressed pub key
    // bitcoin::PubkeyHash is the hash160 of the compressed public key
    let pubkey_hash = compressed.pubkey_hash();
    let witness_program: Vec<u8> = pubkey_hash.as_byte_array().to_vec();

    let address = bech32::segwit::encode_v0(hrp, &witness_program)
        .map_err(|e| PluginError::Internal(format!("bech32 encode: {e}")))?;
    Ok(address)
}

/// Generate a legacy P2PKH (Pay-to-PubKey-Hash) address for Litecoin.
///
/// This produces addresses with the legacy prefix:
/// - Mainnet: `L...` (starting with L or M)
/// - Testnet: `m...` or `n...`
///
/// Many LTC testnet faucets only accept legacy P2PKH addresses (not bech32 `tltc1`),
/// so this function exists as a fallback when bech32 addresses are rejected.
fn ltc_p2pkh_address(
    compressed: &bitcoin::CompressedPublicKey,
    network: &str,
) -> Result<String, PluginError> {
    // Build the address payload: [version_byte] ++ HASH160(compressed_pubkey)
    let pubkey_hash = compressed.pubkey_hash();
    let (version_byte, _) = match network {
        // Litecoin mainnet P2PKH: 0x30 ('L')
        // Litecoin mainnet P2SH:  0x32 ('M')
        "litecoin" => (0x30u8, "L"),
        // Litecoin testnet P2PKH: 0x6f ('m'/'n')
        // Litecoin testnet P2SH:  0xc4 ('2')
        "litecoin-testnet3" | "litecoin-testnet4" | "litecoin-testnet" => (0x6fu8, "m"),
        _ => return Err(PluginError::UnsupportedNetwork(network.into())),
    };

    let mut payload = Vec::with_capacity(1 + 20);
    payload.push(version_byte);
    payload.extend_from_slice(pubkey_hash.as_byte_array());

    let address = bitcoin::base58::encode_check(&payload);
    Ok(address)
}

/// Map an LTC network string to a `bitcoin::Network` for BIP-32 / address derivation.
///
/// ⚠️ **Litecoin FPI reference:** This uses Bitcoin's `Network` enum because Litecoin
/// shares the same BIP-32 HD derivation structure (secp256k1, BIP-84 key paths).
/// A future version should switch to proper Litecoin network parameters
/// (`litecoin::Network` when such a crate stabilises) for correct address prefix
/// validation and p2p message magic bytes. The current mapping works for:
///   - Key derivation (identical BIP-32 algorithm)
///   - Address generation (Bech32/Bech32m via P2WPKH)
///   - PSBT signing (identical ECDSA secp256k1)
fn btc_network(network: &str) -> Result<bitcoin::Network, PluginError> {
    match network {
        "litecoin" => Ok(bitcoin::Network::Bitcoin),
        // testnet3 (the network the key-index 0 testnet address lives on).
        "litecoin-testnet3" | "litecoin-testnet" => Ok(bitcoin::Network::Testnet),
        "litecoin-testnet4" => Ok(bitcoin::Network::Testnet4),
        _ => Err(PluginError::UnsupportedNetwork(network.into())),
    }
}

fn coin_type(network: &str) -> u32 {
    // BIP-44 / SLIP-44: Litecoin is registered coin type 2, used for both
    // mainnet and testnet. (Coin type 1 is Bitcoin testnet only.)
    match network {
        "litecoin" => 2,
        "litecoin-testnet3" | "litecoin-testnet4" | "litecoin-testnet" => 2,
        _ => 2,
    }
}

#[async_trait]
impl WalletPlugin for LtcPlugin {
    fn id(&self) -> &'static str {
        "ltc"
    }
    fn name(&self) -> &'static str {
        "Litecoin"
    }
    fn supported_networks(&self) -> &[NetworkSpec] {
        &*LTC_NETWORKS
    }

    fn plugin_metadata(&self) -> PluginMetadata {
        PluginMetadata {
            description: "Litecoin — Scrypt-based cryptocurrency, FPI reference implementation. BIP-84 key derivation, PSBT signing, Esplora-backed queries.",
            author: "Gullbúr Enclave",
            homepage: "https://github.com/MarcusBrammeier/gullbur-enclave",
            repository: "https://github.com/MarcusBrammeier/gullbur-enclave",
            min_wallet_version: "0.1.0",
            tags: &["litecoin", "ltc", "reference", "scrypt", "bip84"],
        }
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::AccountCreation,
            Capability::Signing,
            Capability::Broadcast,
            Capability::Balance,
            Capability::FeeEstimation,
            Capability::AddressValidation,
        ]
    }

    async fn create_account(
        &self,
        seed: &[u8],
        index: u32,
        network: &str,
    ) -> Result<Account, PluginError> {
        let btc_net = btc_network(network)?;
        let ct = coin_type(network);
        let path = format!("m/84'/{ct}'/0'/0/{index}");
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let master = bitcoin::bip32::Xpriv::new_master(btc_net, seed)
            .map_err(|e| PluginError::Internal(format!("BIP-32 master key error: {e}")))?;
        let derivation_path: bitcoin::bip32::DerivationPath =
            path.parse().map_err(|e: bitcoin::bip32::Error| {
                PluginError::Internal(format!("BIP-32 path: {e}"))
            })?;
        let child = master
            .derive_priv(&secp, &derivation_path)
            .map_err(|e| PluginError::Internal(format!("derivation: {e}")))?;
        let compressed = bitcoin::CompressedPublicKey(
            bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &child.private_key),
        );
        // Use legacy P2PKH address for testnet (most faucets only accept m/n prefix)
        // and Bech32 segwit for mainnet (modern wallets prefer ltc1).
        // Both testnet3 and testnet4 use the same legacy m/n P2PKH prefix (0x6f).
        let address_str = match network {
            "litecoin-testnet3" | "litecoin-testnet4" | "litecoin-testnet" => {
                ltc_p2pkh_address(&compressed, network)?
            }
            _ => ltc_p2wpkh_address(&compressed, network)?,
        };
        Ok(Account {
            id: format!("ltc-{network}-{index}"),
            network: network.into(),
            address: address_str,
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
        let mut psbt =
            Psbt::deserialize(tx).map_err(|e| PluginError::Internal(format!("PSBT parse: {e}")))?;
        if psbt.inputs.is_empty() {
            return Err(PluginError::Internal("PSBT has no inputs".into()));
        }
        let acct_index = account_index;
        let btc_net = btc_network(network)?;
        let ct = coin_type(network);
        let path = format!("m/84'/{ct}'/0'/0/{acct_index}");
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let master = bitcoin::bip32::Xpriv::new_master(btc_net, &seed)
            .map_err(|e| PluginError::Internal(format!("master key: {e}")))?;
        let derivation_path: bitcoin::bip32::DerivationPath = path
            .parse()
            .map_err(|e: bitcoin::bip32::Error| PluginError::Internal(format!("path: {e}")))?;
        let child = master
            .derive_priv(&secp, &derivation_path)
            .map_err(|e| PluginError::Internal(format!("derive: {e}")))?;
        let compressed_pubkey = bitcoin::CompressedPublicKey(
            bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &child.private_key),
        );
        let bitcoin_pubkey: bitcoin::PublicKey = compressed_pubkey.into();
        let secret_bytes = child.private_key.secret_bytes();

        // Shared signing context (same account key for all inputs)
        let sighash_type = bitcoin::sighash::EcdsaSighashType::All;
        let mut sighasher = SighashCache::new(&psbt.unsigned_tx);
        let secp_signer = bitcoin::secp256k1::Secp256k1::signing_only();
        let secret_key = bitcoin::secp256k1::SecretKey::from_slice(&secret_bytes)
            .map_err(|e| PluginError::Internal(format!("invalid key: {e}")))?;

        // Sign EVERY input that has a witness UTXO
        for i in 0..psbt.inputs.len() {
            let utxo = psbt.inputs[i].witness_utxo.as_ref().ok_or_else(|| {
                PluginError::Internal(format!("PSBT input {i} missing witness UTXO"))
            })?;

            // Compute the correct signature hash for the UTXO's script type.
            //
            // create_account emits *legacy P2PKH* addresses (m/n...) for testnet,
            // so a funded UTXO's script_pubkey is P2PKH and must be signed with the
            // legacy signature hash (script code = the P2PKH script itself). Segwit
            // UTXOs (ltc1/tltc1) use the BIP-143 P2WPKH hash. Signing with the wrong
            // algorithm for the script type yields a signature the node rejects.
            let digest: [u8; 32] = if utxo.script_pubkey.is_p2wpkh() {
                let h = sighasher
                    .p2wpkh_signature_hash(i, &utxo.script_pubkey, utxo.value, sighash_type)
                    .map_err(|e| PluginError::Internal(format!("p2wpkh sighash: {e}")))?;
                *h.as_ref()
            } else if utxo.script_pubkey.is_p2pkh() {
                let h = sighasher
                    .legacy_signature_hash(i, &utxo.script_pubkey, sighash_type.to_u32())
                    .map_err(|e| PluginError::Internal(format!("p2pkh sighash: {e}")))?;
                *h.as_ref()
            } else {
                return Err(PluginError::Internal(format!(
                    "unsupported UTXO script type for input {i}: {}",
                    utxo.script_pubkey
                )));
            };

            // Sign with bitcoin's secp256k1 (handles low-S normalization)
            let msg = bitcoin::secp256k1::Message::from_digest(digest);
            let secp_sig = secp_signer.sign_ecdsa(&msg, &secret_key);
            let mut sig_bytes = secp_sig.serialize_der().to_vec();
            sig_bytes.push(sighash_type.to_u32() as u8);
            let bitcoin_sig = bitcoin::ecdsa::Signature::from_slice(&sig_bytes)
                .map_err(|e| PluginError::Internal(format!("sig conversion: {e}")))?;
            psbt.inputs[i]
                .partial_sigs
                .insert(bitcoin_pubkey, bitcoin_sig);
        }
        Ok(psbt.serialize())
    }

    async fn broadcast_transaction(&self, tx: &[u8], network: &str) -> Result<String, PluginError> {
        let base = esplora_base(network);
        let url = format!("{base}/tx");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PluginError::BroadcastFailed(format!("client: {e}")))?;
        let body = hex::encode(tx);
        let resp = client
            .post(&url)
            .header("Content-Type", "text/plain")
            .body(body)
            .send()
            .await
            .map_err(|e| PluginError::BroadcastFailed(format!("HTTP: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| PluginError::BroadcastFailed(format!("read: {e}")))?;
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
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PluginError::NetworkError(format!("client: {e}")))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| PluginError::NetworkError(format!("HTTP: {e}")))?;
        // Surface the actual HTTP status/body on failure instead of letting
        // .json() throw a generic parse error — so the UI can show "LTC API
        // unreachable (429)" rather than a vague "network failed".
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let snippet: String = body.chars().take(200).collect();
            return Err(PluginError::NetworkError(format!(
                "LTC Esplora {status} for balance: {snippet}"
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PluginError::NetworkError(format!("JSON: {e}")))?;

        let chain_funded = json["chain_stats"]["funded_txo_sum"].as_u64().unwrap_or(0);
        let chain_spent = json["chain_stats"]["spent_txo_sum"].as_u64().unwrap_or(0);
        let mempool_funded = json["mempool_stats"]["funded_txo_sum"]
            .as_u64()
            .unwrap_or(0);
        let mempool_spent = json["mempool_stats"]["spent_txo_sum"].as_u64().unwrap_or(0);

        let confirmed = chain_funded.saturating_sub(chain_spent);
        let unconfirmed = mempool_funded.saturating_sub(mempool_spent);

        let fmt = |v: u64| -> String {
            if v == 0 {
                return "0".into();
            }
            let s = v as f64 / 100_000_000.0;
            format!("{:.8}", s)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        };
        Ok(Balance {
            confirmed: fmt(confirmed),
            unconfirmed: fmt(unconfirmed),
            unit: "LTC".into(),
        })
    }

    async fn get_transaction_history(
        &self,
        _a: &Account,
        _n: &str,
        _l: u32,
    ) -> Result<Vec<TxRecord>, PluginError> {
        Ok(vec![])
    }

    async fn estimate_fee(&self, _t: &[u8], network: &str) -> Result<FeeEstimate, PluginError> {
        let base = esplora_base(network);
        let url = format!("{base}/v1/fees/recommended");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PluginError::NetworkError(format!("client: {e}")))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| PluginError::NetworkError(format!("HTTP: {e}")))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PluginError::NetworkError(format!("JSON: {e}")))?;

        let fast = json["fastestFee"]
            .as_u64()
            .map_or("0".into(), |v| format!("{v}"));
        let medium = json["halfHourFee"]
            .as_u64()
            .map_or("0".into(), |v| format!("{v}"));
        let slow = json["hourFee"]
            .as_u64()
            .map_or("0".into(), |v| format!("{v}"));

        Ok(FeeEstimate {
            fast,
            medium,
            slow,
            unit: "sat/vB".into(),
        })
    }

    async fn validate_address(&self, addr: &str, network: &str) -> Result<bool, PluginError> {
        match network {
            "litecoin" => {
                // Bech32 segwit (ltc1...): decode validates the checksum.
                if let Ok((hrp, _, _)) = bech32::segwit::decode(addr) {
                    if hrp.as_str() == "ltc" {
                        return Ok(true);
                    }
                    // fall through to legacy check below
                } else if base58ck::decode_check(addr).is_ok()
                    && (addr.starts_with('L') || addr.starts_with('M'))
                {
                    // Legacy P2PKH/P2SH (L/M...): require a valid base58check checksum,
                    // not just the prefix. This rejects corrupted/garbage addresses.
                    return Ok(true);
                }
                Ok(false)
            }
            "litecoin-testnet3" | "litecoin-testnet4" | "litecoin-testnet" => {
                // Bech32 segwit (tltc1...): decode validates the checksum.
                if let Ok((hrp, _, _)) = bech32::segwit::decode(addr) {
                    if hrp.as_str() == "tltc" {
                        return Ok(true);
                    }
                    // fall through to legacy check below
                } else if base58ck::decode_check(addr).is_ok()
                    && (addr.starts_with('m')
                        || addr.starts_with('n')
                        || addr.starts_with('Q')
                        || addr.starts_with('T'))
                {
                    // Legacy P2PKH/P2SH (m/n/Q/T...): require a valid base58check checksum.
                    return Ok(true);
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures;

    #[test]
    fn test_plugin_id() {
        assert_eq!(LtcPlugin::new().id(), "ltc");
    }

    #[test]
    fn test_supported_networks() {
        let plugin = LtcPlugin::new();
        let ids: Vec<&str> = plugin.supported_networks().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), 4);
        assert!(ids.contains(&"litecoin"));
        assert!(ids.contains(&"litecoin-testnet3"));
        assert!(ids.contains(&"litecoin-testnet4"));
        assert!(ids.contains(&"litecoin-testnet"), "backward-compat alias");
    }

    #[test]
    fn test_capabilities_includes_account_creation() {
        let caps = LtcPlugin::new().capabilities();
        assert!(caps.contains(&Capability::AccountCreation));
    }

    #[test]
    fn test_validate_ltc1_address() {
        let plugin = LtcPlugin::new();
        // Valid LTC mainnet bech32 segwit (real checksum), starts with 'ltc1'
        let result = futures::executor::block_on(
            plugin.validate_address("ltc1qqqqsyqcyq5rqwzqfpg9scrgwpugpzysn3s44dy", "litecoin"),
        )
        .expect("test invariant");
        assert!(result);
    }

    #[test]
    fn test_validate_invalid_address() {
        let plugin = LtcPlugin::new();
        let result = futures::executor::block_on(plugin.validate_address("bad", "litecoin"))
            .expect("test invariant");
        assert!(!result);
    }

    #[test]
    fn test_coin_type_mainnet() {
        assert_eq!(coin_type("litecoin"), 2);
    }

    #[test]
    fn test_coin_type_testnet() {
        assert_eq!(coin_type("litecoin-testnet"), 2);
    }

    #[test]
    fn test_coin_type_unknown() {
        assert_eq!(coin_type("unknown"), 2);
    }

    #[test]
    fn test_btc_network_mainnet() {
        assert_eq!(
            btc_network("litecoin").expect("test invariant"),
            bitcoin::Network::Bitcoin
        );
    }

    #[test]
    fn test_btc_network_testnet3() {
        assert_eq!(
            btc_network("litecoin-testnet3").expect("test invariant"),
            bitcoin::Network::Testnet
        );
    }

    #[test]
    fn test_btc_network_testnet_alias() {
        assert_eq!(
            btc_network("litecoin-testnet").expect("test invariant"),
            bitcoin::Network::Testnet
        );
    }

    #[test]
    fn test_btc_network_testnet4() {
        assert_eq!(
            btc_network("litecoin-testnet4").expect("test invariant"),
            bitcoin::Network::Testnet4
        );
    }

    #[test]
    fn test_btc_network_unsupported() {
        assert!(btc_network("dogecoin").is_err());
    }

    #[test]
    fn test_esplora_base_mainnet() {
        assert_eq!(
            esplora_base("litecoin"),
            "https://litecoin.mempool.space/api"
        );
    }

    #[test]
    fn test_esplora_base_testnet3() {
        assert_eq!(
            esplora_base("litecoin-testnet3"),
            "https://litecoin.mempool.space/testnet/api"
        );
        assert_eq!(
            esplora_base("litecoin-testnet"),
            "https://litecoin.mempool.space/testnet/api"
        );
    }

    #[test]
    fn test_esplora_base_testnet4() {
        assert_eq!(
            esplora_base("litecoin-testnet4"),
            "https://litecoin.mempool.space/testnet4/api"
        );
    }

    #[test]
    fn test_balance_base_mainnet() {
        assert_eq!(
            esplora_base("litecoin"),
            "https://litecoin.mempool.space/api"
        );
    }

    #[test]
    fn test_balance_base_testnet4() {
        assert_eq!(
            esplora_base("litecoin-testnet4"),
            "https://litecoin.mempool.space/testnet4/api"
        );
    }

    #[test]
    fn test_ltc_p2wpkh_mainnet() {
        // Known test vector: Bitcoin test pubkey → Litecoin mainnet address
        // Using a known compressed pubkey to verify proper checksum
        let pk = bitcoin::secp256k1::PublicKey::from_slice(
            &hex::decode("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
                .unwrap(),
        )
        .unwrap();
        let compressed = bitcoin::CompressedPublicKey(pk);
        let addr = ltc_p2wpkh_address(&compressed, "litecoin").unwrap();
        assert!(
            addr.starts_with("ltc1"),
            "LTC mainnet should start with ltc1, got {addr}"
        );
        // Verify it has a valid bech32 checksum (at least 52 chars for length 20 witness program)
        assert_eq!(
            addr.len(),
            43,
            "P2WPKH address should be 43 chars, got {addr}"
        );
    }

    #[test]
    fn test_ltc_p2wpkh_testnet() {
        let pk = bitcoin::secp256k1::PublicKey::from_slice(
            &hex::decode("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
                .unwrap(),
        )
        .unwrap();
        let compressed = bitcoin::CompressedPublicKey(pk);
        let addr = ltc_p2wpkh_address(&compressed, "litecoin-testnet").unwrap();
        assert!(
            addr.starts_with("tltc1"),
            "LTC testnet should start with tltc1, got {addr}"
        );
        assert_eq!(
            addr.len(),
            44,
            "P2WPKH testnet address should be 44 chars, got {addr}"
        );
    }

    #[test]
    fn test_ltc_p2wpkh_unsupported_network() {
        let pk = bitcoin::secp256k1::PublicKey::from_slice(
            &hex::decode("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
                .unwrap(),
        )
        .unwrap();
        let compressed = bitcoin::CompressedPublicKey(pk);
        let result = ltc_p2wpkh_address(&compressed, "dogecoin");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_address_mainnet_legacy() {
        let plugin = LtcPlugin::new();
        // Valid LTC mainnet P2PKH (starts with 'L', correct base58check checksum)
        let r = futures::executor::block_on(
            plugin.validate_address("LKDyUEtTR1HXamkiEphisSiBJu6o3ZPE34", "litecoin"),
        )
        .expect("test invariant");
        assert!(r);
    }

    #[test]
    fn test_validate_address_mainnet_legacy_rejects_bad_checksum() {
        let plugin = LtcPlugin::new();
        // Same prefix and length, but corrupted base58check checksum → must reject.
        // This is the false-positive case: a garbage 'L...' string used to pass.
        let r = futures::executor::block_on(plugin.validate_address("Labc123", "litecoin"))
            .expect("test invariant");
        assert!(!r);
    }

    #[test]
    fn test_validate_address_mainnet_bech32() {
        let plugin = LtcPlugin::new();
        // Valid LTC mainnet bech32 segwit, starts with 'ltc1'
        let r = futures::executor::block_on(
            plugin.validate_address("ltc1qqqqsyqcyq5rqwzqfpg9scrgwpugpzysn3s44dy", "litecoin"),
        )
        .expect("test invariant");
        assert!(r);
    }

    #[test]
    fn test_validate_address_testnet_bech32() {
        let plugin = LtcPlugin::new();
        // Valid LTC testnet bech32 segwit, starts with 'tltc1'
        let r = futures::executor::block_on(plugin.validate_address(
            "tltc1qqqqsyqcyq5rqwzqfpg9scrgwpugpzysnxzku7w",
            "litecoin-testnet",
        ))
        .expect("test invariant");
        assert!(r);
    }

    #[test]
    fn test_validate_address_testnet_legacy() {
        let plugin = LtcPlugin::new();
        // Valid LTC testnet P2PKH (starts with 'm', correct checksum)
        let r = futures::executor::block_on(
            plugin.validate_address("mfWyW5fc9NUj75YAnFgoRLrjxgLDn2MMth", "litecoin-testnet"),
        )
        .expect("test invariant");
        assert!(r);
    }

    #[test]
    fn test_validate_address_testnet_legacy_rejects_bad_checksum() {
        let plugin = LtcPlugin::new();
        let r =
            futures::executor::block_on(plugin.validate_address("mabc12345", "litecoin-testnet"))
                .expect("test invariant");
        assert!(!r);
    }

    #[test]
    fn test_validate_address_testnet_rejects_mainnet() {
        let plugin = LtcPlugin::new();
        // A valid mainnet LTC address must not validate as testnet
        let r = futures::executor::block_on(
            plugin.validate_address("LKDyUEtTR1HXamkiEphisSiBJu6o3ZPE34", "litecoin-testnet"),
        )
        .expect("test invariant");
        assert!(!r);
    }

    #[test]
    fn test_validate_address_unknown_network_returns_false() {
        let plugin = LtcPlugin::new();
        let r = futures::executor::block_on(plugin.validate_address("Labc", "unknown"))
            .expect("test invariant");
        assert!(!r);
    }

    #[tokio::test]
    async fn test_sign_transaction_multi_input_all_inputs_signed() {
        // Regression test for the "multi-input PSBT only signs inputs[0]" bug.
        // sign_transaction must produce a partial sig for EVERY input that has
        // a witness UTXO — mirror of the BTC plugin's test, but for LTC.
        use bitcoin::absolute::LockTime;
        use bitcoin::hashes::Hash;
        use bitcoin::psbt::Psbt;
        use bitcoin::transaction::Version;
        use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness};

        let plugin = LtcPlugin::new();

        // Deterministic 32-byte seed.
        let seed_hex = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let seed_bytes = hex::decode(seed_hex).expect("test invariant");

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

        // Same P2WPKH script for both inputs (LTC mainnet bech32 path).
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

        // LTC's sign_transaction parses key_id as "hexseed@index".
        let key = wallet_plugin::KeyHandle {
            key_id: format!("{seed_hex}@0"),
            key_type: wallet_plugin::KeyType::Secp256k1,
            public_key: vec![],
        };

        let signed = plugin
            .sign_transaction(&psbt_bytes, &seed_bytes, 0, "litecoin")
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
            "input[1] must have a partial sig (multi-input regression) — was only signing inputs[0]"
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
        let plugin = LtcPlugin::new();
        let seed_bytes = vec![0xdeu8; 32];
        let result = plugin
            .sign_transaction(b"not a valid PSBT", &seed_bytes, 0, "litecoin")
            .await;
        assert!(result.is_err(), "passing garbage bytes should return Err");
    }

    #[tokio::test]
    async fn test_sign_transaction_p2pkh_legacy_input() {
        // Regression test for the P2PKH (legacy, m/n...) signing path.
        // create_account emits *legacy P2PKH* addresses for testnet, but sign_transaction
        // previously only handled P2WPKH (BIP-143). That mismatch made the plugin sign a
        // P2PKH UTXO with the wrong sighash algorithm, so any broadcast of a funded
        // legacy testnet address was rejected by the node. This test proves a P2PKH
        // witness_utxo now signs via the legacy (pre-segwit) signature hash.
        use bitcoin::absolute::LockTime;
        use bitcoin::hashes::Hash;
        use bitcoin::psbt::Psbt;
        use bitcoin::transaction::Version;
        use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness};

        let plugin = LtcPlugin::new();

        let seed_hex = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let seed_bytes = hex::decode(seed_hex).expect("test invariant");

        let unsigned_tx = bitcoin::Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::new(bitcoin::Txid::from_byte_array([1u8; 32]), 0),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(90000),
                script_pubkey: ScriptBuf::from_bytes(vec![0u8; 25]),
            }],
        };

        // Legacy P2PKH script: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
        // = 0x76 0xa9 0x14 <hash160> 0x88 0xac
        let mut p2pkh_bytes = vec![0x76u8, 0xa9, 0x14];
        p2pkh_bytes.extend_from_slice(&[0u8; 20]);
        p2pkh_bytes.push(0x88);
        p2pkh_bytes.push(0xac);
        let p2pkh_script = ScriptBuf::from_bytes(p2pkh_bytes);
        assert!(p2pkh_script.is_p2pkh(), "test invariant: script must be P2PKH");

        let psbt = Psbt {
            unsigned_tx,
            version: 0,
            xpub: Default::default(),
            proprietary: Default::default(),
            unknown: Default::default(),
            inputs: vec![bitcoin::psbt::Input {
                witness_utxo: Some(TxOut {
                    value: Amount::from_sat(50000),
                    script_pubkey: p2pkh_script,
                }),
                ..Default::default()
            }],
            outputs: vec![Default::default()],
        };

        let psbt_bytes = psbt.serialize();
        let signed = plugin
            .sign_transaction(&psbt_bytes, &seed_bytes, 0, "litecoin-testnet")
            .await
            .expect("sign_transaction should succeed on a P2PKH legacy input");

        let signed_psbt = Psbt::deserialize(&signed).expect("signed PSBT should deserialize");
        assert!(
            !signed_psbt.inputs[0].partial_sigs.is_empty(),
            "P2PKH input must carry a partial sig"
        );

        // Confirm the UTXO script we signed stayed P2PKH (not rewritten to a witness
        // program), proving the legacy sighash path was used for THIS script.
        assert!(
            signed_psbt.inputs[0]
                .witness_utxo
                .as_ref()
                .expect("witness_utxo preserved")
                .script_pubkey
                .is_p2pkh(),
            "signed input's UTXO script must remain P2PKH"
        );
    }
}
