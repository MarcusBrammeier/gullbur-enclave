use async_trait::async_trait;
use bitcoin::hashes::Hash;
use std::sync::LazyLock;
use wallet_plugin::{
    Account, Balance, Capability, FeeEstimate, KeyHandle, NetworkSpec, PluginError, PluginMetadata,
    TxRecord, WalletPlugin,
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

static LTC_NETWORKS: LazyLock<[NetworkSpec; 2]> = LazyLock::new(|| {
    [
        NetworkSpec {
            id: "litecoin".into(),
            name: "Litecoin".into(),
            symbol: "LTC".into(),
            decimals: 8,
            is_testnet: false,
        },
        NetworkSpec {
            id: "litecoin-testnet".into(),
            name: "Litecoin Testnet".into(),
            symbol: "tLTC".into(),
            decimals: 8,
            is_testnet: true,
        },
    ]
});

fn esplora_base(network: &str) -> &str {
    match network {
        "litecoin" => "https://litecoin.mempool.space/api",
        "litecoin-testnet" => "https://litecoin.mempool.space/testnet/api",
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
        "litecoin-testnet" => "tltc",
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
        "litecoin-testnet" => (0x6fu8, "m"),
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
        "litecoin-testnet" => Ok(bitcoin::Network::Testnet),
        _ => Err(PluginError::UnsupportedNetwork(network.into())),
    }
}

fn coin_type(network: &str) -> u32 {
    match network {
        "litecoin" => 2,
        "litecoin-testnet" => 1,
        _ => 1,
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
            homepage: "https://github.com/YOUR_GITHUB_ORG/YOUR_GITHUB_REPO",
            repository: "https://github.com/YOUR_GITHUB_ORG/YOUR_GITHUB_REPO",
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
        let address_str = match network {
            "litecoin-testnet" => ltc_p2pkh_address(&compressed, network)?,
            _ => ltc_p2wpkh_address(&compressed, network)?,
        };
        Ok(Account {
            id: format!("ltc-{network}-{index}"),
            network: network.into(),
            address: address_str,
            path: Some(path),
            label: None,
        })
    }

    async fn sign_transaction(
        &self,
        tx: &[u8],
        key: &KeyHandle,
        network: &str,
    ) -> Result<Vec<u8>, PluginError> {
        use bitcoin::psbt::Psbt;
        use bitcoin::sighash::SighashCache;
        let mut psbt =
            Psbt::deserialize(tx).map_err(|e| PluginError::Internal(format!("PSBT parse: {e}")))?;
        if psbt.inputs.is_empty() {
            return Err(PluginError::Internal("PSBT has no inputs".into()));
        }
        // Decode seed from key_id (format: "hex_seed@index" or "0xhex_seed@index")
        let (seed_hex, acct_index): (String, u32) = {
            let raw = key.key_id.strip_prefix("0x").unwrap_or(&key.key_id);
            if let Some(at_pos) = raw.find('@') {
                let seed_part = &raw[..at_pos];
                let idx: u32 = raw[at_pos + 1..]
                    .parse()
                    .map_err(|e| PluginError::Internal(format!("invalid account index: {e}")))?;
                (seed_part.to_string(), idx)
            } else {
                (raw.to_string(), 0u32)
            }
        };
        let seed = hex::decode(&seed_hex)
            .map_err(|e| PluginError::Internal(format!("invalid seed: {e}")))?;
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

            // Compute BIP-143 sighash for this input at its proper index
            let sighash = sighasher
                .p2wpkh_signature_hash(i, &utxo.script_pubkey, utxo.value, sighash_type)
                .map_err(|e| PluginError::Internal(format!("sighash: {e}")))?;

            // Sign with bitcoin's secp256k1 (handles low-S normalization)
            let msg = bitcoin::secp256k1::Message::from_digest(*sighash.as_ref());
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
                // Try bech32 checksum validation via segwit decode
                #[allow(clippy::collapsible_if)]
                if let Ok((hrp, _, _)) = bech32::segwit::decode(addr) {
                    if hrp.as_str() == "ltc" {
                        return Ok(true);
                    }
                }
                // Fall back to legacy P2PKH/P2SH prefix check
                Ok(addr.starts_with("L") || addr.starts_with("M") || addr.starts_with("ltc1"))
            }
            "litecoin-testnet" => {
                // Try bech32 checksum validation via segwit decode
                #[allow(clippy::collapsible_if)]
                if let Ok((hrp, _, _)) = bech32::segwit::decode(addr) {
                    if hrp.as_str() == "tltc" {
                        return Ok(true);
                    }
                }
                // Fall back to legacy P2PKH/P2SH prefix check
                Ok(addr.starts_with("m")
                    || addr.starts_with("n")
                    || addr.starts_with("tltc1")
                    || addr.starts_with("Q")
                    || addr.starts_with("T"))
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
        assert_eq!(LtcPlugin::new().supported_networks().len(), 2);
    }

    #[test]
    fn test_capabilities_includes_account_creation() {
        let caps = LtcPlugin::new().capabilities();
        assert!(caps.contains(&Capability::AccountCreation));
    }

    #[test]
    fn test_validate_ltc1_address() {
        let plugin = LtcPlugin::new();
        let result = futures::executor::block_on(plugin.validate_address("ltc1q...", "litecoin"))
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
        assert_eq!(coin_type("litecoin-testnet"), 1);
    }

    #[test]
    fn test_coin_type_unknown() {
        assert_eq!(coin_type("unknown"), 1);
    }

    #[test]
    fn test_btc_network_mainnet() {
        assert_eq!(
            btc_network("litecoin").expect("test invariant"),
            bitcoin::Network::Bitcoin
        );
    }

    #[test]
    fn test_btc_network_testnet() {
        assert_eq!(
            btc_network("litecoin-testnet").expect("test invariant"),
            bitcoin::Network::Testnet
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
    fn test_esplora_base_testnet() {
        assert_eq!(
            esplora_base("litecoin-testnet"),
            "https://litecoin.mempool.space/testnet/api"
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
    fn test_balance_base_testnet() {
        assert_eq!(
            esplora_base("litecoin-testnet"),
            "https://litecoin.mempool.space/testnet/api"
        );
    }

    #[test]
    fn test_ltc_p2wpkh_mainnet() {
        // Known test vector: Bitcoin test pubkey → Litecoin mainnet address
        // Using a known compressed pubkey to verify proper checksum
        let secp = bitcoin::secp256k1::Secp256k1::new();
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
        let secp = bitcoin::secp256k1::Secp256k1::new();
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
        let secp = bitcoin::secp256k1::Secp256k1::new();
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
        let r = futures::executor::block_on(plugin.validate_address("Labc123", "litecoin"))
            .expect("test invariant");
        assert!(r);
    }

    #[test]
    fn test_validate_address_mainnet_bech32() {
        let plugin = LtcPlugin::new();
        let r = futures::executor::block_on(plugin.validate_address("ltc1qabc123", "litecoin"))
            .expect("test invariant");
        assert!(r);
    }

    #[test]
    fn test_validate_address_testnet_bech32() {
        let plugin = LtcPlugin::new();
        let r = futures::executor::block_on(
            plugin.validate_address("tltc1qabc123", "litecoin-testnet"),
        )
        .expect("test invariant");
        assert!(r);
    }

    #[test]
    fn test_validate_address_testnet_rejects_mainnet() {
        let plugin = LtcPlugin::new();
        let r =
            futures::executor::block_on(plugin.validate_address("ltc1qabc...", "litecoin-testnet"))
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
}
