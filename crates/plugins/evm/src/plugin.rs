use async_trait::async_trait;
use std::sync::LazyLock;
use wallet_plugin::{
    Account, Balance, Capability, FeeEstimate, NetworkSpec, PluginError, PluginMetadata,
    TxRecord, TxStatus, WalletPlugin,
};

pub struct EvmPlugin {
    socks5_proxy: Option<String>,
}

impl EvmPlugin {
    pub fn new() -> Self {
        Self { socks5_proxy: None }
    }

    pub fn with_tor(socks_port: u16) -> Self {
        Self::new_with_proxy(Some(format!("socks5://127.0.0.1:{socks_port}")))
    }

    pub fn with_proxy(socks5_proxy: Option<String>) -> Self {
        Self { socks5_proxy }
    }

    pub fn new_with_proxy(socks5_proxy: Option<String>) -> Self {
        Self { socks5_proxy }
    }

    fn build_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
        if let Some(ref proxy_url) = self.socks5_proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
        }
        builder.build()
    }
}

impl Default for EvmPlugin {
    fn default() -> Self {
        Self::new()
    }
}

static EVM_NETWORKS: LazyLock<[NetworkSpec; 7]> = LazyLock::new(|| {
    [
        NetworkSpec {
            id: String::from("ethereum"),
            name: String::from("Ethereum"),
            symbol: String::from("ETH"),
            decimals: 18,
            is_testnet: false,
        },
        NetworkSpec {
            id: String::from("arbitrum"),
            name: String::from("Arbitrum One"),
            symbol: String::from("ETH"),
            decimals: 18,
            is_testnet: false,
        },
        NetworkSpec {
            id: String::from("optimism"),
            name: String::from("OP Mainnet"),
            symbol: String::from("ETH"),
            decimals: 18,
            is_testnet: false,
        },
        NetworkSpec {
            id: String::from("base"),
            name: String::from("Base"),
            symbol: String::from("ETH"),
            decimals: 18,
            is_testnet: false,
        },
        NetworkSpec {
            id: String::from("polygon"),
            name: String::from("Polygon"),
            symbol: String::from("POL"),
            decimals: 18,
            is_testnet: false,
        },
        NetworkSpec {
            id: String::from("bnb"),
            name: String::from("BNB Chain"),
            symbol: String::from("BNB"),
            decimals: 18,
            is_testnet: false,
        },
        NetworkSpec {
            id: String::from("sepolia"),
            name: String::from("Sepolia"),
            symbol: String::from("ETH"),
            decimals: 18,
            is_testnet: true,
        },
    ]
});

/// Derive a k256 secret key from seed bytes and account index.
/// This ensures sign_transaction produces the same address as create_account.
fn derive_key_from_seed(seed: &[u8], account_index: u32) -> Result<k256::ecdsa::SigningKey, PluginError> {
    let index = account_index;
    // Prefer full BIP-44 derivation (64-byte BIP-39 seed), fallback to raw key bytes
    if seed.len() >= 64 {
        let seed_512: [u8; 64] = {
            let mut arr = [0u8; 64];
            arr.copy_from_slice(&seed[..64]);
            arr
        };
        let secret = crypto_core::keys::derive_bip44_eth_key(&seed_512, index)
            .map_err(|e| PluginError::Internal(format!("BIP-44 derivation failed: {e}")))?;
        Ok(k256::ecdsa::SigningKey::from(&secret))
    } else if seed.len() >= 32 {
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&seed[..32]);
        let secret = k256::SecretKey::from_slice(&key_bytes)
            .map_err(|e| PluginError::Internal(format!("invalid k256 secret key: {e}")))?;
        Ok(k256::ecdsa::SigningKey::from(&secret))
    } else {
        Err(PluginError::Internal(
            "seed too short for key derivation".into(),
        ))
    }
}

// ── JSON-RPC helpers ─────────────────────────────────────────────────────────

/// Map a network id to its public JSON-RPC endpoint.
pub fn rpc_endpoint(network: &str) -> Option<&'static str> {
    match network {
        "ethereum" => Some("https://ethereum-rpc.publicnode.com"),
        "polygon" => Some("https://polygon-bor-rpc.publicnode.com"),
        "arbitrum" => Some("https://arbitrum-one-rpc.publicnode.com"),
        "optimism" => Some("https://optimism-rpc.publicnode.com"),
        "bnb" => Some("https://bsc-rpc.publicnode.com"),
        "base" => Some("https://base-rpc.publicnode.com"),
        "sepolia" => Some("https://ethereum-sepolia-rpc.publicnode.com"),
        _ => None,
    }
}

impl EvmPlugin {
    /// Perform a JSON-RPC POST request and return the deserialized result field.
    /// Uses the plugin's SOCKS5 proxy if configured.
    async fn json_rpc_call(
        &self,
        endpoint: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, PluginError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1,
        });

        let client = self
            .build_client()
            .map_err(|e| PluginError::NetworkError(format!("Failed to build HTTP client: {e}")))?;
        let resp = client
            .post(endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| PluginError::NetworkError(format!("HTTP request failed: {e}")))?;

        let text = resp
            .text()
            .await
            .map_err(|e| PluginError::NetworkError(format!("Failed to read response body: {e}")))?;

        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| PluginError::NetworkError(format!("Invalid JSON response: {e}")))?;

        // Check for JSON-RPC error object
        if let Some(err) = json.get("error") {
            let msg = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown JSON-RPC error");
            return Err(PluginError::NetworkError(format!("JSON-RPC error: {msg}")));
        }

        json.get("result")
            .ok_or_else(|| {
                PluginError::NetworkError("JSON-RPC response missing 'result' field".into())
            })
            .cloned()
    }
}

/// Parse a hex-encoded string from a JSON value (strips 0x prefix).
fn hex_str_to_u128(val: &serde_json::Value) -> Result<u128, PluginError> {
    let s = val
        .as_str()
        .ok_or_else(|| PluginError::NetworkError("expected a string in JSON-RPC result".into()))?;
    let hex = s.strip_prefix("0x").unwrap_or(s);
    u128::from_str_radix(hex, 16)
        .map_err(|e| PluginError::NetworkError(format!("failed to parse hex value '{s}': {e}")))
}

// ── Minimal RLP helpers for EIP-1559 transaction encoding/decoding ──────────

/// RLP-encode a single byte array (string item).
fn rlp_encode_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        vec![data[0]]
    } else if data.len() <= 55 {
        let mut out = Vec::with_capacity(1 + data.len());
        out.push(0x80 + data.len() as u8);
        out.extend_from_slice(data);
        out
    } else {
        let len_bytes = rlp_length_bytes(data.len());
        let mut out = Vec::with_capacity(1 + len_bytes.len() + data.len());
        out.push(0xb7 + len_bytes.len() as u8);
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(data);
        out
    }
}

/// RLP-encode a list of byte arrays.
fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let total_len: usize = items.iter().map(|i| i.len()).sum();
    if total_len <= 55 {
        let mut out = Vec::with_capacity(1 + total_len);
        out.push(0xc0 + total_len as u8);
        for item in items {
            out.extend_from_slice(item);
        }
        out
    } else {
        let len_bytes = rlp_length_bytes(total_len);
        let mut out = Vec::with_capacity(1 + len_bytes.len() + total_len);
        out.push(0xf7 + len_bytes.len() as u8);
        out.extend_from_slice(&len_bytes);
        for item in items {
            out.extend_from_slice(item);
        }
        out
    }
}

fn rlp_length_bytes(len: usize) -> Vec<u8> {
    let be = len.to_be_bytes();
    let start = be.iter().position(|&b| b != 0).unwrap_or(be.len() - 1);
    be[start..].to_vec()
}

/// RLP-decode a single item from bytes, returning the item bytes and remaining bytes.
fn rlp_decode_item(data: &[u8]) -> Result<(Vec<u8>, &[u8]), PluginError> {
    if data.is_empty() {
        return Err(PluginError::Internal("RLP decode: empty input".into()));
    }
    let prefix = data[0];
    if prefix <= 0x7f {
        Ok((vec![prefix], &data[1..]))
    } else if prefix <= 0xb7 {
        let len = (prefix - 0x80) as usize;
        if data.len() < 1 + len {
            return Err(PluginError::Internal("RLP decode: string too short".into()));
        }
        Ok((data[1..1 + len].to_vec(), &data[1 + len..]))
    } else if prefix <= 0xbf {
        let len_len = (prefix - 0xb7) as usize;
        if data.len() < 1 + len_len {
            return Err(PluginError::Internal(
                "RLP decode: long string header too short".into(),
            ));
        }
        let len_bytes = &data[1..1 + len_len];
        let len = rlp_bytes_to_usize(len_bytes)?;
        if data.len() < 1 + len_len + len {
            return Err(PluginError::Internal(
                "RLP decode: long string payload too short".into(),
            ));
        }
        Ok((
            data[1 + len_len..1 + len_len + len].to_vec(),
            &data[1 + len_len + len..],
        ))
    } else if prefix <= 0xf7 {
        let len = (prefix - 0xc0) as usize;
        if data.len() < 1 + len {
            return Err(PluginError::Internal("RLP decode: list too short".into()));
        }
        Ok((data[..1 + len].to_vec(), &data[1 + len..]))
    } else {
        let len_len = (prefix - 0xf7) as usize;
        if data.len() < 1 + len_len {
            return Err(PluginError::Internal(
                "RLP decode: long list header too short".into(),
            ));
        }
        let len_bytes = &data[1..1 + len_len];
        let len = rlp_bytes_to_usize(len_bytes)?;
        if data.len() < 1 + len_len + len {
            return Err(PluginError::Internal(
                "RLP decode: long list payload too short".into(),
            ));
        }
        Ok((
            data[..1 + len_len + len].to_vec(),
            &data[1 + len_len + len..],
        ))
    }
}

fn rlp_bytes_to_usize(bytes: &[u8]) -> Result<usize, PluginError> {
    if bytes.len() > 8 {
        return Err(PluginError::Internal(
            "RLP: length too large for usize".into(),
        ));
    }
    let mut buf = [0u8; 8];
    buf[8 - bytes.len()..].copy_from_slice(bytes);
    Ok(u64::from_be_bytes(buf) as usize)
}

// ── WalletPlugin Implementation ─────────────────────────────────────────────

#[async_trait]
impl WalletPlugin for EvmPlugin {
    fn id(&self) -> &'static str {
        "evm"
    }
    fn name(&self) -> &'static str {
        "EVM (Ethereum + L2s)"
    }

    fn supported_networks(&self) -> &[NetworkSpec] {
        &*EVM_NETWORKS
    }

    fn plugin_metadata(&self) -> PluginMetadata {
        PluginMetadata {
            description: "Ethereum Virtual Machine — EIP-1559 (type-2) tx signing, balance via eth_getBalance, 7+ L2s",
            author: "Gullbúr Enclave",
            homepage: "https://github.com/MarcusBrammeier/gullbur-enclave",
            repository: "https://github.com/MarcusBrammeier/gullbur-enclave",
            min_wallet_version: "0.1.0",
            tags: &[
                "evm", "ethereum", "arbitrum", "optimism", "base", "polygon", "bnb", "eip1559",
            ],
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
        // BIP-44 derivation from a 64-byte BIP-39 seed
        let seed_512: [u8; 64] = seed
            .try_into()
            .map_err(|_| PluginError::Internal("expected 64-byte BIP-39 seed".into()))?;
        let key = crypto_core::keys::derive_bip44_eth_key(&seed_512, index)
            .map_err(|e| PluginError::Internal(format!("Key derivation failed: {e}")))?;
        let public_key = key.public_key();
        let addr = crypto_core::hash::keccak256(&public_key.to_sec1_bytes()[1..]);
        let address = format!("0x{}", hex::encode(&addr[12..]));
        Ok(Account {
            id: format!("{network}-{index}"),
            network: network.to_string(),
            address,
            path: Some(format!("m/44'/60'/{index}'/0/0")),
            label: None,
            index,
        })
    }

    async fn sign_transaction(
        &self,
        tx: &[u8],
        seed: &[u8],
        account_index: u32,
        _network: &str,
    ) -> Result<Vec<u8>, PluginError> {
        if tx.is_empty() || tx[0] != 0x02 {
            return Err(PluginError::Internal(
                "Not a valid EIP-1559 transaction: missing 0x02 envelope byte".into(),
            ));
        }

        let (mut list, _rem) = rlp_decode_item(&tx[1..])?;
        if list.is_empty() || list[0] < 0xc0 {
            return Err(PluginError::Internal(
                "Invalid EIP-1559 RLP: not a list".into(),
            ));
        }
        list.remove(0); // Remove list prefix byte

        let mut fields: Vec<Vec<u8>> = Vec::with_capacity(9);
        let mut remaining: &[u8] = &list;
        for i in 0..9 {
            let (item, rem) = rlp_decode_item(remaining).map_err(|e| {
                PluginError::Internal(format!("Failed to decode RLP field {i}: {e}"))
            })?;
            fields.push(item);
            remaining = rem;
        }

        let chain_id: u64 = rlp_bytes_to_u64(&fields[0])
            .ok_or_else(|| PluginError::Internal("Invalid chain_id in tx".into()))?;
        let _ = chain_id;

        let signing_key = derive_key_from_seed(seed, account_index)?;

        // Build EIP-1559 signing hash: keccak256(0x02 || full_rlp_list)
        // Use the original RLP-encoded list (with prefix) from tx input
        let rlp_encoded_list = &tx[1..]; // The full RLP list (prefix + payload)
        let mut hash_input = vec![0x02u8];
        hash_input.extend_from_slice(rlp_encoded_list);
        let hash = crypto_core::hash::keccak256(&hash_input);

        let (sig, recid) = signing_key.sign_prehash_recoverable(&hash);
        let r = sig.r().to_bytes();
        let s = sig.s().to_bytes();
        let r = r.to_vec();
        let s = s.to_vec();

        let y_parity = recid.is_y_odd();

        let mut encoded_fields: Vec<Vec<u8>> = Vec::with_capacity(12);
        for (i, f) in fields.iter().enumerate() {
            if i == 8 {
                // Access list — encode as RLP list, not bytes
                let items = if f.is_empty() || f[0] == 0xc0 {
                    // Empty access list
                    vec![]
                } else if f[0] <= 0xf7 {
                    // Non-empty, extract sub-items
                    let payload_len = (f[0] - 0xc0) as usize;
                    let mut sub_items = Vec::new();
                    let mut rem = &f[1..1 + payload_len];
                    while !rem.is_empty() {
                        let (item, rest) = rlp_decode_item(rem).map_err(|e| {
                            PluginError::Internal(format!("Failed to decode access list: {e}"))
                        })?;
                        sub_items.push(item);
                        rem = rest;
                    }
                    sub_items
                } else {
                    // Long list — not expected for access lists
                    vec![]
                };
                let encoded = if items.is_empty() {
                    vec![0xc0]
                } else {
                    let owned: Vec<Vec<u8>> = items.into_iter().map(|v| v.to_vec()).collect();
                    rlp_encode_list(&owned)
                };
                encoded_fields.push(encoded);
            } else {
                encoded_fields.push(rlp_encode_bytes(f));
            }
        }
        if y_parity {
            encoded_fields.push(rlp_encode_bytes(&[0x01]));
        } else {
            encoded_fields.push(rlp_encode_bytes(&[]));
        }
        encoded_fields.push(rlp_encode_bytes(&r));
        encoded_fields.push(rlp_encode_bytes(&s));

        let list_rlp = rlp_encode_list(&encoded_fields);
        let mut signed_tx = vec![0x02];
        signed_tx.extend_from_slice(&list_rlp);

        Ok(signed_tx)
    }

    async fn broadcast_transaction(&self, tx: &[u8], network: &str) -> Result<String, PluginError> {
        let endpoint = rpc_endpoint(network)
            .ok_or_else(|| PluginError::UnsupportedNetwork(network.to_string()))?;

        let hex_tx = format!("0x{}", hex::encode(tx));

        let result = self
            .json_rpc_call(
                endpoint,
                "eth_sendRawTransaction",
                serde_json::json!([hex_tx]),
            )
            .await?;

        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| PluginError::BroadcastFailed("JSON-RPC result is not a string".into()))
    }

    async fn get_balance(&self, account: &Account, network: &str) -> Result<Balance, PluginError> {
        let endpoint = rpc_endpoint(network)
            .ok_or_else(|| PluginError::UnsupportedNetwork(network.to_string()))?;

        let result = self
            .json_rpc_call(
                endpoint,
                "eth_getBalance",
                serde_json::json!([account.address, "latest"]),
            )
            .await?;

        let wei = hex_str_to_u128(&result)?;
        let eth = wei as f64 / 1_000_000_000_000_000_000.0;
        let balance_str = format!("{:.18}", eth)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();

        Ok(Balance {
            confirmed: balance_str,
            unconfirmed: "0".into(),
            unit: "ETH".into(),
        })
    }

    async fn get_transaction_history(
        &self,
        account: &Account,
        network: &str,
        _limit: u32,
    ) -> Result<Vec<TxRecord>, PluginError> {
        let endpoint = rpc_endpoint(network)
            .ok_or_else(|| PluginError::UnsupportedNetwork(network.to_string()))?;

        let result = self
            .json_rpc_call(
                endpoint,
                "eth_getTransactionCount",
                serde_json::json!([account.address, "latest"]),
            )
            .await?;

        let count = hex_str_to_u128(&result)?;

        if count > 0 {
            Ok(vec![TxRecord {
                txid: "N/A (JSON-RPC only shows count)".into(),
                from_address: None,
                to_address: None,
                amount: "N/A".into(),
                fee: None,
                block_height: None,
                timestamp: None,
                status: TxStatus::Pending,
            }])
        } else {
            Ok(vec![])
        }
    }

    async fn estimate_fee(&self, _tx: &[u8], network: &str) -> Result<FeeEstimate, PluginError> {
        let endpoint = rpc_endpoint(network)
            .ok_or_else(|| PluginError::UnsupportedNetwork(network.to_string()))?;

        let result = self
            .json_rpc_call(endpoint, "eth_gasPrice", serde_json::json!([]))
            .await?;

        let wei = hex_str_to_u128(&result)?;
        let gwei = wei as f64 / 1_000_000_000.0;
        let gwei_str = format!("{:.9}", gwei)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();

        Ok(FeeEstimate {
            fast: gwei_str.clone(),
            medium: gwei_str.clone(),
            slow: gwei_str,
            unit: "gwei".into(),
        })
    }

    async fn validate_address(&self, addr: &str, _network: &str) -> Result<bool, PluginError> {
        // Basic checks: 0x prefix, 42 chars total, all hex after 0x
        if !addr.starts_with("0x") || addr.len() != 42 {
            return Ok(false);
        }
        let hex_chars = &addr[2..];
        if !hex_chars.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(false);
        }

        // EIP-55 mixed-case checksum validation
        let lower = hex_chars.to_ascii_lowercase();
        let hash = crypto_core::hash::keccak256(lower.as_bytes());

        let expected: String = lower
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if c.is_ascii_digit() {
                    c
                } else {
                    let hash_byte = hash[i / 2];
                    let nybble = if i % 2 == 0 {
                        hash_byte >> 4
                    } else {
                        hash_byte & 0x0f
                    };
                    if nybble >= 8 {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    }
                }
            })
            .collect();

        // EIP-55: all-lowercase, all-uppercase, and correct mixed-case are all
        // accepted (all-lower/all-upper carry no checksum protection; only a
        // non-uniform casing must match the keccak checksum exactly).
        let upper = lower.to_ascii_uppercase();
        if addr[2..] == *lower || addr[2..] == upper || addr[2..] == expected {
            return Ok(true);
        }
        Ok(false)
    }
}

fn rlp_bytes_to_u64(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return Some(0);
    }
    if bytes.len() > 8 {
        return None;
    }
    let mut buf = [0u8; 8];
    buf[8 - bytes.len()..].copy_from_slice(bytes);
    Some(u64::from_be_bytes(buf))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wallet_plugin::WalletPlugin;

    #[test]
    fn test_rpc_endpoint_ethereum() {
        assert_eq!(
            rpc_endpoint("ethereum"),
            Some("https://ethereum-rpc.publicnode.com")
        );
    }

    #[test]
    fn test_rpc_endpoint_polygon() {
        assert_eq!(
            rpc_endpoint("polygon"),
            Some("https://polygon-bor-rpc.publicnode.com")
        );
    }

    #[test]
    fn test_rpc_endpoint_bnb() {
        assert_eq!(rpc_endpoint("bnb"), Some("https://bsc-rpc.publicnode.com"));
    }

    #[test]
    fn test_rpc_endpoint_unknown() {
        assert_eq!(rpc_endpoint("unknown-network"), None);
    }

    #[test]
    fn test_plugin_id_and_name() {
        let plugin = EvmPlugin::new();
        assert_eq!(plugin.id(), "evm");
        assert!(plugin.name().contains("EVM"));
    }

    #[test]
    fn test_supported_networks_count() {
        let plugin = EvmPlugin::new();
        assert_eq!(plugin.supported_networks().len(), 7);
    }

    #[test]
    fn test_supported_networks_has_ethereum() {
        let plugin = EvmPlugin::new();
        let networks = plugin.supported_networks();
        let eth = networks
            .iter()
            .find(|n| n.id == "ethereum")
            .expect("Should have ethereum network");
        assert_eq!(eth.symbol, "ETH");
        assert_eq!(eth.decimals, 18);
        assert!(!eth.is_testnet);
    }

    #[tokio::test]
    async fn test_validate_address_valid() {
        let plugin = EvmPlugin::new();
        let result = plugin
            .validate_address("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045", "ethereum")
            .await;
        assert!(result.expect("test invariant"));
    }

    #[tokio::test]
    async fn test_validate_address_rejects_bad_checksum_case() {
        let plugin = EvmPlugin::new();
        // The correct EIP-55 checksum for vitalik.eth is 0xd8dA6BF...aA96045.
        // Flipping a single letter's case in the mixed-case portion must be
        // rejected — this is the EIP-55 false-acceptance case.
        let bad = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA9604F";
        let result = plugin.validate_address(bad, "ethereum").await;
        assert!(!result.expect("test invariant"));
    }

    #[tokio::test]
    async fn test_validate_address_accepts_all_lowercase() {
        let plugin = EvmPlugin::new();
        // All-lowercase is accepted by EIP-55 (no checksum protection).
        let result = plugin
            .validate_address("0xd8da6bf26964af9d7eed9e03e53415d37aa96045", "ethereum")
            .await;
        assert!(result.expect("test invariant"));
    }

    #[tokio::test]
    async fn test_validate_address_accepts_all_uppercase() {
        let plugin = EvmPlugin::new();
        // All-uppercase hex body (with lowercase 0x prefix) is accepted by
        // EIP-55 as a no-checksum form.
        let result = plugin
            .validate_address("0xD8DA6BF26964AF9D7EED9E03E53415D37AA96045", "ethereum")
            .await;
        assert!(result.expect("test invariant"));
    }

    #[tokio::test]
    async fn test_validate_address_invalid_short() {
        let plugin = EvmPlugin::new();
        let result = plugin.validate_address("0x123", "ethereum").await;
        assert!(!result.expect("test invariant"));
    }

    #[tokio::test]
    async fn test_validate_address_invalid_no_prefix() {
        let plugin = EvmPlugin::new();
        let result = plugin
            .validate_address("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045", "ethereum")
            .await;
        assert!(!result.expect("test invariant"));
    }

    #[tokio::test]
    async fn test_create_account_format() {
        let plugin = EvmPlugin::new();
        // Use a 64-byte test seed (simulating BIP-39 512-bit seed)
        let seed_64 = [42u8; 64];
        let account = plugin
            .create_account(&seed_64, 0, "ethereum")
            .await
            .expect("test invariant");
        assert!(account.address.starts_with("0x"));
        assert_eq!(account.address.len(), 42);
        assert_eq!(account.network, "ethereum");
    }

    #[tokio::test]
    async fn test_sign_simple_eip1559() {
        let plugin = EvmPlugin::new();

        let chain_id = vec![0x01u8];
        let zero = vec![0x80u8];
        let fee21000 = rlp_encode_bytes(&[0x52, 0x08]);
        let to = rlp_encode_bytes(&[0u8; 20]);
        let empty = vec![0x80u8];
        let access_list = vec![0xc0u8];

        let list_rlp = rlp_encode_list(&[
            chain_id,
            zero.clone(),
            zero.clone(),
            fee21000.clone(),
            fee21000,
            to,
            zero,
            empty,
            access_list,
        ]);
        let mut tx_bytes = vec![0x02];
        tx_bytes.extend_from_slice(&list_rlp);

        let seed_64 = {
            let mut s = [42u8; 64];
            // Fill the second half with a different pattern so first-32-bytes
            // fallback would produce different keys
            for (i, slot) in s.iter_mut().enumerate().skip(32) {
                *slot = 42 + (i - 32) as u8;
            }
            s
        };

        let signed = plugin
            .sign_transaction(&tx_bytes, &seed_64, 1, "ethereum")
            .await
            .expect("sign_transaction should succeed for valid EIP-1559 tx");

        assert_eq!(signed.first(), Some(&0x02));
        assert!(signed.len() > tx_bytes.len());

        // Verify that index 1 produces a different signature than index 0
        let signed0 = plugin
            .sign_transaction(&tx_bytes, &seed_64, 0, "ethereum")
            .await
            .expect("sign_transaction should succeed for index 0");
        assert_ne!(
            signed, signed0,
            "different BIP-44 indices must produce different signatures"
        );
    }

    #[tokio::test]
    async fn test_sign_transaction_invalid_rlp() {
        let plugin = EvmPlugin::new();
        let seed_bytes = [42u8; 32];
        let result = plugin
            .sign_transaction(b"not a valid tx", &seed_bytes, 0, "ethereum")
            .await;
        assert!(result.is_err(), "passing garbage bytes should return Err");
    }

    // ── Tor / SOCKS5 proxy tests ────────────────────────────────────────────

    #[test]
    fn test_evm_plugin_default_no_proxy() {
        let plugin = EvmPlugin::new();
        assert!(plugin.socks5_proxy.is_none());
    }

    #[test]
    fn test_evm_plugin_with_tor_proxy() {
        let plugin = EvmPlugin::with_tor(9050);
        assert_eq!(plugin.socks5_proxy, Some("socks5://127.0.0.1:9050".into()));
    }

    #[test]
    fn test_evm_plugin_with_proxy_custom() {
        let plugin = EvmPlugin::with_proxy(Some("socks5://tor-proxy:9150".into()));
        assert_eq!(plugin.socks5_proxy, Some("socks5://tor-proxy:9150".into()));
    }

    #[test]
    fn test_evm_plugin_with_proxy_none() {
        let plugin = EvmPlugin::with_proxy(None);
        assert!(plugin.socks5_proxy.is_none());
    }

    #[test]
    fn test_build_client_with_proxy() {
        let plugin = EvmPlugin::with_tor(9050);
        let client = plugin.build_client();
        assert!(
            client.is_ok(),
            "build_client should succeed with valid proxy URL"
        );
    }

    #[test]
    fn test_build_client_no_proxy() {
        let plugin = EvmPlugin::new();
        let client = plugin.build_client();
        assert!(client.is_ok(), "build_client should succeed without proxy");
    }
}
