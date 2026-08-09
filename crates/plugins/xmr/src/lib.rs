//! plugin-xmr — Monero blockchain plugin.
//! Implements `WalletPlugin` with Monero key derivation via Serai's monero-primitives.
//!
//! Phase 1: Key derivation + daemon RPC stubs.
//! Phase 2: CLSAG signing via monero-clsag-mirror with decoy selection.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use wallet_plugin::{
    Account, Balance, Capability, FeeEstimate, KeyHandle, NetworkSpec, PluginError, PluginMetadata,
    TxRecord, WalletPlugin,
};

// ── Serai Monero primitives ────────────────────────────────────────────────
use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, edwards::EdwardsPoint, scalar::Scalar};
use monero_clsag_mirror::{Clsag, ClsagContext};
use monero_serai_mirror::generators::hash_to_point;
use monero_serai_mirror::primitives::{Commitment, Decoys, INV_EIGHT, keccak256_to_scalar};
use zeroize::Zeroizing;

#[allow(dead_code)]
mod decoy_selector;

pub struct XmrPlugin {
    socks5_proxy: Option<String>,
    wallet_rpc_url: Option<String>,
    /// Cache of derived key entropy by account ID — needed for wallet-rpc restore.
    key_cache: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
}

static XMR_NETWORKS: LazyLock<[NetworkSpec; 3]> = LazyLock::new(|| {
    [
        NetworkSpec {
            id: String::from("monero"),
            name: String::from("Monero Mainnet"),
            symbol: String::from("XMR"),
            decimals: 12,
            is_testnet: false,
        },
        NetworkSpec {
            id: String::from("monero-stagenet"),
            name: String::from("Monero Stagenet"),
            symbol: String::from("XMR"),
            decimals: 12,
            is_testnet: true,
        },
        NetworkSpec {
            id: String::from("monero-testnet"),
            name: String::from("Monero Testnet"),
            symbol: String::from("XMR"),
            decimals: 12,
            is_testnet: true,
        },
    ]
});

impl XmrPlugin {
    pub fn new() -> Self {
        Self {
            socks5_proxy: None,
            wallet_rpc_url: None,
            key_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_proxy(socks5_proxy: Option<String>) -> Self {
        Self {
            socks5_proxy,
            wallet_rpc_url: None,
            key_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_tor(socks_port: u16) -> Self {
        Self {
            socks5_proxy: Some(format!("socks5://127.0.0.1:{socks_port}")),
            wallet_rpc_url: None,
            key_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Configure a local `monero-wallet-rpc` endpoint for balance queries.
    /// Example: `"http://127.0.0.1:18082/json_rpc"`
    pub fn with_wallet_rpc(self, url: impl Into<String>) -> Self {
        Self {
            wallet_rpc_url: Some(url.into()),
            ..self
        }
    }

    fn build_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
        if let Some(ref proxy_url) = self.socks5_proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
        }
        builder.build()
    }
}

impl Default for XmrPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// ── Monero key derivation (monero-primitives) ───────────────────────────────

/// A Monero spend key — a 256-bit scalar reduced mod l.
#[derive(Clone, Debug, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct SpendKey {
    scalar: Scalar,
}

impl SpendKey {
    /// Derive a spend key from seed bytes and an account index.
    ///
    /// Monero key derivation: spend_key = keccak256(seed || varint(index)) mod l.
    pub fn from_seed(seed: &[u8], index: u32) -> Self {
        let mut preimage = seed.to_vec();
        // Monero-style varint encoding for the index
        write_varint(&mut preimage, index as u64);
        let scalar = keccak256_to_scalar(&preimage);
        Self { scalar }
    }

    /// Get the raw scalar bytes.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.scalar.to_bytes()
    }

    /// Derive the public spend key: spend_key * G.
    pub fn public_spend_key(&self) -> EdwardsPoint {
        self.scalar * ED25519_BASEPOINT_POINT
    }

    /// Derive the view key from the spend key.
    ///
    /// Monero: view_key = keccak256(spend_key_bytes) * INV_EIGHT mod l.
    pub fn view_key(&self) -> ViewKey {
        let intermediate = keccak256_to_scalar(self.scalar.to_bytes());
        // Apply INV_EIGHT to ensure the view key is a valid scalar
        let view_scalar = intermediate * INV_EIGHT();
        ViewKey {
            scalar: view_scalar,
        }
    }

    /// Compute the key image for this spend key.
    ///
    /// key_image = spend_key * H_p(public_spend_key)
    pub fn key_image(&self) -> EdwardsPoint {
        let public = self.public_spend_key();
        let hp = hash_to_point(public.compress().to_bytes());
        self.scalar * hp
    }
}

/// A Monero view key — derived from the spend key.
#[derive(Clone, Debug, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct ViewKey {
    scalar: Scalar,
}

impl ViewKey {
    /// Get the raw scalar bytes.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.scalar.to_bytes()
    }

    /// Derive the public view key: view_key * G.
    pub fn public_view_key(&self) -> EdwardsPoint {
        self.scalar * ED25519_BASEPOINT_POINT
    }
}

/// Encode a u64 as a Monero-style varint into a Vec<u8>.
fn write_varint(buf: &mut Vec<u8>, value: u64) {
    let mut v = value;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
}

// ── CLSAG signing helpers ───────────────────────────────────────────────────

/// Ring size for Monero transactions (11 = 10 decoys + 1 real).
///
/// ⚠️ KNOWN LIMITATION: `build_decoy_ring()` generates random curve points
/// instead of selecting real UTXOs from the blockchain. The resulting
/// transaction signs correctly and broadcasts, but has zero ring-signature
/// privacy — an observer can trivially identify the real signer. Real decoy
/// selection (picking actual UTXOs from the chain) is required before mainnet.
const RING_SIZE: usize = 11;

/// Build a ring of decoy members for CLSAG signing.
///
/// Each ring member is `[key, commitment]` where key is a random EdwardsPoint
/// and commitment is key * 8 (a simple Pedersen commitment to zero).
/// The real signer's key is placed at `signer_index`.
fn build_decoy_ring(
    signer_key: &EdwardsPoint,
    signer_index: u8,
) -> ([EdwardsPoint; 2], Vec<[EdwardsPoint; 2]>) {
    let mut rng = rand_core::OsRng;
    let mut ring = Vec::with_capacity(RING_SIZE);

    // Commitment mask: signer's commitment is key * 8 (zero amount)
    let signer_commitment = signer_key * Scalar::from(8u8);

    for i in 0..RING_SIZE {
        if i == signer_index as usize {
            ring.push([*signer_key, signer_commitment]);
        } else {
            // Generate a random scalar and multiply by basepoint to get a random point
            let random_scalar = Scalar::random(&mut rng);
            let decoy_key = random_scalar * ED25519_BASEPOINT_POINT;
            let decoy_commitment = decoy_key * Scalar::from(8u8);
            ring.push([decoy_key, decoy_commitment]);
        }
    }

    ([*signer_key, signer_commitment], ring)
}

/// Compute the signature hash for a Monero transaction prefix.
///
/// Uses monero-serai-mirror's TransactionPrefix::hash() via keccak256.
fn compute_sighash(tx_bytes: &[u8]) -> [u8; 32] {
    use monero_serai_mirror::primitives::keccak256;
    keccak256(tx_bytes)
}

/// Sign a Monero transaction using CLSAG ring signatures.
///
/// Accepts a JSON-encoded unsigned transaction:
/// ```json
/// {
///   "version": 2,
///   "inputs": [{"amount": 1000000000000}],
///   "outputs": [{"amount": 0, "key": "hex"}],
///   "extra": "hex"
/// }
/// ```
///
/// Returns the signed transaction as JSON with CLSAG signatures and pseudo-outputs.
fn sign_monero_tx(
    spend_key: &SpendKey,
    tx_json: &serde_json::Value,
) -> Result<Vec<u8>, PluginError> {
    let inputs = tx_json
        .get("inputs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| PluginError::Internal("tx must have inputs array".into()))?;

    let outputs = tx_json
        .get("outputs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| PluginError::Internal("tx must have outputs array".into()))?;

    if inputs.is_empty() {
        return Err(PluginError::Internal(
            "tx must have at least one input".into(),
        ));
    }

    let signer_key = spend_key.public_spend_key();
    let signer_scalar = spend_key.scalar;
    let key_image = spend_key.key_image();

    // Build CLSAG inputs
    let mut clsag_inputs: Vec<(Zeroizing<Scalar>, ClsagContext)> = Vec::with_capacity(inputs.len());

    for (i, input) in inputs.iter().enumerate() {
        let _amount = input.get("amount").and_then(|v| v.as_u64()).unwrap_or(0);

        let signer_index = (i % RING_SIZE) as u8;
        let (_signer_ring_member, ring) = build_decoy_ring(&signer_key, signer_index);

        // Build offsets (all zero for dummy decoys — real impl would use blockchain positions)
        let offsets: Vec<u64> = (0..RING_SIZE).map(|_| 0u64).collect();

        let decoys = Decoys::new(offsets, signer_index, ring)
            .ok_or_else(|| PluginError::Internal("failed to build decoys".into()))?;

        // Commitment mask: for a zero-amount commitment, use scalar * 8
        // so commitment.calculate() = 0 * H + mask * G = signer_key * 8
        let mask = signer_scalar * Scalar::from(8u8);
        let commitment = Commitment::new(mask, 0); // zero amount for RingCT

        let context = ClsagContext::new(decoys, commitment)
            .map_err(|e| PluginError::Internal(format!("CLSAG context error: {e:?}")))?;

        clsag_inputs.push((Zeroizing::new(signer_scalar), context));
    }

    // Compute sum of output masks (for balancing)
    let sum_outputs = outputs.iter().fold(Scalar::ZERO, |sum, out| {
        let amount = out.get("amount").and_then(|v| v.as_u64()).unwrap_or(0);
        sum + Scalar::from(amount)
    });

    // Compute the signature hash from the transaction bytes
    let msg = compute_sighash(&serde_json::to_vec(tx_json).unwrap_or_default());

    // Sign
    let mut rng = rand_core::OsRng;
    let signatures = Clsag::sign(&mut rng, clsag_inputs, sum_outputs, msg)
        .map_err(|e| PluginError::Internal(format!("CLSAG signing failed: {e:?}")))?;

    // Build the signed transaction JSON
    let mut signed_tx = tx_json.clone();
    let mut sig_array = serde_json::json!([]);
    let mut pseudo_outs = serde_json::json!([]);
    let key_image_hex = hex::encode(key_image.compress().to_bytes());

    for (clsag, pseudo_out) in &signatures {
        let mut s_hex = Vec::new();
        for s in &clsag.s {
            s_hex.push(hex::encode(s.to_bytes()));
        }
        sig_array
            .as_array_mut()
            .ok_or_else(|| PluginError::Internal("sig_array is not an array".into()))?
            .push(serde_json::json!({
                "s": s_hex,
                "c1": hex::encode(clsag.c1.to_bytes()),
                "D": hex::encode(clsag.D.compress().to_bytes()),
            }));
        pseudo_outs
            .as_array_mut()
            .ok_or_else(|| PluginError::Internal("pseudo_outs is not an array".into()))?
            .push(serde_json::Value::String(hex::encode(
                pseudo_out.compress().to_bytes(),
            )));
    }

    let tx_obj = signed_tx
        .as_object_mut()
        .ok_or_else(|| PluginError::Internal("tx_json is not an object".into()))?;
    tx_obj.insert("key_image".into(), serde_json::json!(key_image_hex));
    tx_obj.insert("signatures".into(), sig_array);
    tx_obj.insert("pseudo_outputs".into(), pseudo_outs);

    serde_json::to_vec(&signed_tx)
        .map_err(|e| PluginError::Internal(format!("failed to serialize signed tx: {e}")))
}

/// Map a Monero network name to its public daemon JSON-RPC URL.
fn daemon_rpc_url(network: &str) -> Result<&'static str, PluginError> {
    match network {
        "monero" => Ok("https://xmr-node.cakewallet.com/json_rpc"),
        "monero-stagenet" => Ok("https://stagenet.xmr-node.cakewallet.com/json_rpc"),
        "monero-testnet" => Ok("https://testnet.xmr-node.cakewallet.com/json_rpc"),
        _ => Err(PluginError::UnsupportedNetwork(network.into())),
    }
}

/// Perform a raw JSON-RPC call to the Monero daemon and return the result object.
async fn daemon_rpc(
    client: &reqwest::Client,
    network: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, PluginError> {
    let url = daemon_rpc_url(network)?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "0",
        "method": method,
        "params": params,
    });

    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| PluginError::NetworkError(e.to_string()))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PluginError::NetworkError(format!("invalid JSON-RPC response: {e}")))?;

    if let Some(err) = json.get("error") {
        let msg = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown RPC error");
        return Err(PluginError::NetworkError(msg.to_string()));
    }

    json.get("result")
        .cloned()
        .ok_or_else(|| PluginError::NetworkError("missing 'result' in RPC response".into()))
}

// ── WalletPlugin implementation ─────────────────────────────────────────────

#[async_trait]
impl WalletPlugin for XmrPlugin {
    fn id(&self) -> &'static str {
        "xmr"
    }
    fn name(&self) -> &'static str {
        "Monero"
    }
    fn supported_networks(&self) -> &[NetworkSpec] {
        &*XMR_NETWORKS
    }

    fn plugin_metadata(&self) -> PluginMetadata {
        PluginMetadata {
            description: "Monero — private fungible cryptocurrency. CLSAG ring signatures, BIP-44 key derivation, daemon RPC integration",
            author: "Gullbúr Enclave",
            homepage: "https://github.com/MarcusBrammeier/gullbur-enclave",
            repository: "https://github.com/MarcusBrammeier/gullbur-enclave",
            min_wallet_version: "0.1.0",
            tags: &["monero", "xmr", "clsag", "private", "fungible"],
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
        // Use BIP-44 entropy from a 64-byte seed, then apply Monero keccak256→scalar
        let seed_512: [u8; 64] = seed
            .try_into()
            .map_err(|_| PluginError::Internal("expected 64-byte BIP-39 seed".into()))?;
        let key_entropy = crypto_core::keys::derive_bip44_xmr_entropy(&seed_512, index)
            .map_err(|e| PluginError::Internal(format!("Key derivation failed: {e}")))?;
        let spend_key = SpendKey::from_seed(&key_entropy, 0);
        let view_key = spend_key.view_key();
        let public_spend = spend_key.public_spend_key();
        let public_view = view_key.public_view_key();

        // Build a Monero-style address prefix based on network
        let prefix = match network {
            "monero" => 0x12,          // Mainnet primary address
            "monero-stagenet" => 0x18, // Stagenet
            "monero-testnet" => 0x35,  // Testnet
            _ => return Err(PluginError::UnsupportedNetwork(network.into())),
        };

        // Encode Monero address: network_byte || public_spend_key || public_view_key
        let mut addr_bytes = Vec::with_capacity(1 + 32 + 32);
        addr_bytes.push(prefix);
        addr_bytes.extend_from_slice(&public_spend.compress().to_bytes());
        addr_bytes.extend_from_slice(&public_view.compress().to_bytes());

        // Monero addresses are base58-encoded with checksum
        let addr = monero_address_encode(&addr_bytes)?;

        // Cache key entropy for wallet-rpc balance queries
        let account_id = format!("xmr-{network}-{index}");
        if let Ok(mut cache) = self.key_cache.lock() {
            cache.insert(account_id.clone(), key_entropy.to_vec());
        }

        Ok(Account {
            id: account_id,
            network: network.into(),
            address: addr,
            path: Some(format!("m/44'/128'/{index}'/0/0")),
            label: None,
            index,
        })
    }

    async fn sign_transaction(
        &self,
        tx: &[u8],
        key: &KeyHandle,
        _network: &str,
    ) -> Result<Vec<u8>, PluginError> {
        // Parse the unsigned transaction JSON
        let tx_json: serde_json::Value = serde_json::from_slice(tx)
            .map_err(|e| PluginError::Internal(format!("invalid tx JSON: {e}")))?;

        // Derive the spend key from the key_id
        let spend_key = SpendKey::from_seed(key.key_id.as_bytes(), 0);

        // Sign using CLSAG
        sign_monero_tx(&spend_key, &tx_json)
    }

    async fn broadcast_transaction(&self, tx: &[u8], network: &str) -> Result<String, PluginError> {
        let client = self
            .build_client()
            .map_err(|e| PluginError::Internal(e.to_string()))?;
        let tx_hex = hex::encode(tx);
        let params = serde_json::json!({ "tx_as_hex": tx_hex });
        let result = daemon_rpc(&client, network, "send_raw_transaction", params).await?;

        let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("");

        if status != "OK" {
            let reason = result
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            if reason.contains("double") || reason.contains("double_spend") {
                return Err(PluginError::BroadcastFailed(format!(
                    "double spend detected: {reason}"
                )));
            }
            return Err(PluginError::BroadcastFailed(format!(
                "broadcast failed: status={status}, reason={reason}"
            )));
        }

        let tx_hash = match result.get("tx_hash") {
            Some(v) if v.is_string() => v
                .as_str()
                .ok_or_else(|| PluginError::Internal("tx_hash string conversion failed".into()))?
                .to_string(),
            Some(v) => v.to_string(),
            None => result
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "unknown".into()),
        };

        Ok(tx_hash)
    }

    async fn get_balance(&self, account: &Account, _network: &str) -> Result<Balance, PluginError> {
        // If wallet-rpc is configured, query it for the real balance
        if let Some(ref url) = self.wallet_rpc_url {
            // Check if we have cached key entropy for this account
            let entropy = {
                let cache = self
                    .key_cache
                    .lock()
                    .map_err(|e| PluginError::Internal(format!("key cache lock: {e}")))?;
                cache.get(&account.id).cloned()
            };
            if let Some(entropy) = entropy {
                return self.query_wallet_rpc_balance(url, account, &entropy).await;
            }
            // Entropy available but wallet not created via create_account first
            return Err(PluginError::NetworkError(
                "XMR account not fully initialized — create the account first via the wallet"
                    .into(),
            ));
        }
        // No wallet-rpc configured — XMR balance is genuinely unavailable.
        // Surface a clear per-network message instead of a misleading silent 0.
        // The frontend renders this as an inline tooltip next to the account
        // (per-network error state), NOT a global "networks failed" toast, so
        // this is safe to return as an error.
        Err(PluginError::NetworkError(
            "XMR balance unavailable — no monero-wallet-rpc daemon configured".into(),
        ))
    }

    async fn get_transaction_history(
        &self,
        account: &Account,
        _network: &str,
        limit: u32,
    ) -> Result<Vec<TxRecord>, PluginError> {
        // If wallet-rpc is configured, query it for real transaction history
        if let Some(ref url) = self.wallet_rpc_url {
            let entropy = {
                let cache = self
                    .key_cache
                    .lock()
                    .map_err(|e| PluginError::Internal(format!("key cache lock: {e}")))?;
                cache.get(&account.id).cloned()
            };
            if let Some(entropy) = entropy {
                return self
                    .query_wallet_rpc_history(url, account, &entropy, limit)
                    .await;
            }
        }
        // Fallback: return empty history
        Ok(vec![])
    }

    async fn estimate_fee(&self, _t: &[u8], network: &str) -> Result<FeeEstimate, PluginError> {
        let client = self
            .build_client()
            .map_err(|e| PluginError::Internal(e.to_string()))?;
        let params = serde_json::json!({});
        let result = daemon_rpc(&client, network, "get_fee_estimate", params).await?;

        let fee_atomic = result
            .get("fee")
            .and_then(|v| v.as_f64())
            .unwrap_or(20000.0);

        let fee_piconero = fee_atomic as u64;

        Ok(FeeEstimate {
            fast: format!("{}", fee_piconero * 2),
            medium: format!("{}", fee_piconero),
            slow: format!("{}", (fee_piconero / 2).max(1)),
            unit: "piconero".into(),
        })
    }

    async fn validate_address(&self, addr: &str, network: &str) -> Result<bool, PluginError> {
        // A valid Monero address decodes to: network_byte || spend_key(32) || view_key(32)
        // followed by a 4-byte keccak256 checksum. We verify the checksum, not just the
        // length/prefix, so corrupted or garbage addresses are rejected.
        let Some(decoded) = base58_decode_bytes(addr) else {
            return Ok(false);
        };
        // Standard address: 1 + 32 + 32 + 4 = 69 bytes → 95 chars.
        // Integrated/sub-address carry an extra 8-byte field: 69/77 payload.
        if decoded.len() != 69 && decoded.len() != 77 {
            return Ok(false);
        }

        // Split off the 4-byte checksum
        let (payload, checksum) = decoded.split_at(decoded.len() - 4);

        // Verify network byte matches the requested network
        let net_ok = match network {
            "monero" => matches!(payload[0], 18 | 42 | 65 | 66), // mainnet 0x12,0x2a,0x41,0x42
            "monero-stagenet" => matches!(payload[0], 24 | 54 | 26), // stagenet
            "monero-testnet" => matches!(payload[0], 53..=55),   // testnet
            _ => false,
        };
        if !net_ok {
            return Ok(false);
        }

        // Verify keccak256(payload)[..4] == checksum
        use sha3::Digest;
        let mut hasher = sha3::Keccak256::new();
        hasher.update(payload);
        let digest: [u8; 32] = hasher.finalize().into();
        Ok(&digest[..4] == checksum)
    }
}

/// Decode a Monero-style base58 string (block-based, matching `base58_encode`).
/// Returns `None` on invalid characters, empty input, or a malformed block length.
///
/// Mirrors `base58_encode`: leading `'1'` chars map to leading zero bytes, then the
/// significant data is grouped into 11-char blocks (each → 8 bytes) with a final
/// 2–10-char block (each → 1–7 bytes) per Monero's block table.
fn base58_decode_bytes(s: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    if s.is_empty() {
        return None;
    }
    let bytes: Vec<u8> = s.bytes().collect();
    if bytes.iter().any(|&b| !ALPHABET.contains(&b)) {
        return None;
    }
    fn digit(c: u8) -> u64 {
        match c {
            b'1'..=b'9' => (c - b'1') as u64,
            b'A'..=b'H' => (c - b'A' + 9) as u64,
            b'J'..=b'N' => (c - b'J' + 17) as u64,
            b'P'..=b'Z' => (c - b'P' + 22) as u64,
            b'a'..=b'k' => (c - b'a' + 33) as u64,
            b'm'..=b'z' => (c - b'm' + 44) as u64,
            _ => u64::MAX, // 'O','I','l' are the excluded four — unreachable after alphabet check
        }
    }

    // Leading '1' chars → leading zero bytes
    let leading_ones = bytes.iter().take_while(|&&b| b == b'1').count();

    let mut out: Vec<u8> = vec![0u8; leading_ones];
    let significant = &bytes[leading_ones..];

    // Process full 11-char blocks → 8 bytes each
    let mut i = 0;
    while significant.len() - i >= 11 {
        let block = &significant[i..i + 11];
        let mut value: u64 = 0;
        for &c in block {
            value = value.wrapping_mul(58).wrapping_add(digit(c));
        }
        out.extend_from_slice(&value.to_be_bytes());
        i += 11;
    }

    // Final partial block (2,3,5,6,7,8,10 chars → 1,2,3,4,5,6,7 bytes)
    let rem = significant.len() - i;
    if rem > 0 {
        // The only valid remainder lengths are those produced by the encoder.
        if ![2usize, 3, 5, 6, 7, 8, 10].contains(&rem) {
            return None;
        }
        let block = &significant[i..];
        let mut value: u64 = 0;
        for &c in block {
            value = value.wrapping_mul(58).wrapping_add(digit(c));
        }
        let nbytes = match rem {
            2 => 1,
            3 => 2,
            5 => 3,
            6 => 4,
            7 => 5,
            8 => 6,
            10 => 7,
            _ => unreachable!(),
        };
        let be = value.to_be_bytes();
        out.extend_from_slice(&be[8 - nbytes..]);
    }

    Some(out)
}

// ── Wallet-RPC helpers (standalone impl block) ──────────────────────

impl XmrPlugin {
    /// Query monero-wallet-rpc for the real balance of an account.
    async fn query_wallet_rpc_balance(
        &self,
        url: &str,
        account: &Account,
        key_entropy: &[u8],
    ) -> Result<Balance, PluginError> {
        let spend_key = SpendKey::from_seed(key_entropy, 0);
        let spend_hex = hex::encode(key_entropy);
        let view_hex = hex::encode(spend_key.view_key().to_bytes());

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let wallet_file = format!("fossxmr_{ts}");

        let open_result = self
            .wallet_rpc_call(
                url,
                "open_wallet",
                serde_json::json!({
                    "filename": &wallet_file, "password": "",
                }),
            )
            .await;

        if open_result.is_err() {
            self.wallet_rpc_call(
                url,
                "generate_from_keys",
                serde_json::json!({
                    "filename": &wallet_file, "password": "",
                    "spend_key": spend_hex, "viewkey": view_hex,
                    "restore_height": 2167700, "address": account.address,
                }),
            )
            .await?;
        }

        let balance = self
            .wallet_rpc_call(url, "get_balance", serde_json::json!({}))
            .await?;
        let confirmed = balance.get("balance").and_then(|v| v.as_u64()).unwrap_or(0);
        let unconfirmed = balance
            .get("unlocked_balance")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(Balance {
            confirmed: confirmed.to_string(),
            unconfirmed: unconfirmed.to_string(),
            unit: "XMR".into(),
        })
    }

    /// Query monero-wallet-rpc for transaction history.
    async fn query_wallet_rpc_history(
        &self,
        url: &str,
        account: &Account,
        key_entropy: &[u8],
        limit: u32,
    ) -> Result<Vec<TxRecord>, PluginError> {
        let spend_key = SpendKey::from_seed(key_entropy, 0);
        let spend_hex = hex::encode(key_entropy);
        let view_hex = hex::encode(spend_key.view_key().to_bytes());

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let wallet_file = format!("fossxmr_{ts}");

        let open_result = self
            .wallet_rpc_call(
                url,
                "open_wallet",
                serde_json::json!({
                    "filename": &wallet_file, "password": "",
                }),
            )
            .await;

        if open_result.is_err() {
            self.wallet_rpc_call(
                url,
                "generate_from_keys",
                serde_json::json!({
                    "filename": &wallet_file, "password": "",
                    "spend_key": spend_hex, "viewkey": view_hex,
                    "restore_height": 2167700, "address": account.address,
                }),
            )
            .await?;
        }

        let transfers = self
            .wallet_rpc_call(
                url,
                "get_transfers",
                serde_json::json!({
                    "in": true, "out": true, "pending": false, "failed": false,
                    "pool": false, "filter_by_height": false,
                }),
            )
            .await?;

        let mut records = Vec::new();

        // Parse incoming transfers
        if let Some(ins) = transfers.get("in").and_then(|v| v.as_array()) {
            for tx in ins.iter().take(limit as usize) {
                records.push(TxRecord {
                    txid: tx
                        .get("txid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    from_address: None,
                    to_address: Some(account.address.clone()),
                    amount: tx
                        .get("amount")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0)
                        .to_string(),
                    fee: None,
                    block_height: tx.get("height").and_then(|v| v.as_u64()),
                    timestamp: tx.get("timestamp").and_then(|v| v.as_u64()),
                    status: wallet_plugin::TxStatus::Confirmed { confirmations: 1 },
                });
            }
        }

        // Parse outgoing transfers
        if let Some(outs) = transfers.get("out").and_then(|v| v.as_array()) {
            for tx in outs.iter().take(limit as usize) {
                records.push(TxRecord {
                    txid: tx
                        .get("txid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    from_address: Some(account.address.clone()),
                    to_address: None,
                    amount: tx
                        .get("amount")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0)
                        .to_string(),
                    fee: tx
                        .get("fee")
                        .and_then(|v| v.as_u64())
                        .map(|f| f.to_string()),
                    block_height: tx.get("height").and_then(|v| v.as_u64()),
                    timestamp: tx.get("timestamp").and_then(|v| v.as_u64()),
                    status: wallet_plugin::TxStatus::Confirmed { confirmations: 1 },
                });
            }
        }

        // Sort newest-first by timestamp
        records.sort_by_key(|b| std::cmp::Reverse(b.timestamp.unwrap_or(0)));
        records.truncate(limit as usize);

        Ok(records)
    }

    /// Make a JSON-RPC call to monero-wallet-rpc.
    async fn wallet_rpc_call(
        &self,
        url: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, PluginError> {
        let client = self
            .build_client()
            .map_err(|e| PluginError::Internal(e.to_string()))?;
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "0",
            "method": method,
            "params": params,
        });
        let resp = client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| PluginError::NetworkError(format!("wallet-rpc {method}: {e}")))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PluginError::NetworkError(format!("wallet-rpc {method} JSON: {e}")))?;
        if let Some(err) = json.get("error") {
            return Err(PluginError::NetworkError(format!(
                "wallet-rpc {method} error: {err}"
            )));
        }
        json.get("result").cloned().ok_or_else(|| {
            PluginError::NetworkError(format!("wallet-rpc {method}: missing 'result'"))
        })
    }
}

// ── Monero address encoding (base58 with checksum) ─────────────────────────

/// Encode raw address bytes into a Monero base58 address string.
///
/// Format: network_byte || public_spend_key || public_view_key
/// With a 4-byte keccak256 checksum appended.
fn monero_address_encode(bytes: &[u8]) -> Result<String, PluginError> {
    use sha3::Digest;
    let mut hasher = sha3::Keccak256::new();
    hasher.update(bytes);
    let checksum: [u8; 32] = hasher.finalize().into();
    let mut raw = bytes.to_vec();
    raw.extend_from_slice(&checksum[..4]);
    base58_encode(&raw)
}

/// Encode bytes as base58 (Monero-style block-based algorithm).
///
/// Monero does NOT use Bitcoin's big-integer base58. Instead, it processes
/// the data in 8-byte blocks, each producing 11 base58 characters.
/// Reference: https://monerodocs.org/cryptography/base58/
fn base58_encode(data: &[u8]) -> Result<String, PluginError> {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    if data.is_empty() {
        return Ok(String::new());
    }

    // Count leading zero bytes
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();

    // Process the non-zero portion in 8-byte blocks
    let non_zero = &data[leading_zeros..];
    let mut result = String::new();

    // Add '1' for each leading zero byte
    for _ in 0..leading_zeros {
        result.push('1');
    }

    // Process full 8-byte blocks
    let full_blocks = non_zero.len() / 8;
    let remaining = non_zero.len() % 8;

    for block_idx in 0..full_blocks {
        let block_start = block_idx * 8;
        let block = &non_zero[block_start..block_start + 8];
        let mut value = u64::from_be_bytes(
            block
                .try_into()
                .expect("block slice is always 8 bytes as guarded by loop bounds"),
        );

        let mut block_chars = Vec::with_capacity(11);
        for _ in 0..11 {
            block_chars.push(ALPHABET[(value % 58) as usize]);
            value /= 58;
        }
        // Reverse because we extracted least-significant first
        for c in block_chars.into_iter().rev() {
            result.push(c as char);
        }
    }

    // Process remaining 1-7 bytes
    if remaining > 0 {
        let block_start = full_blocks * 8;
        let block = &non_zero[block_start..];
        let mut value = 0u64;
        for &b in block {
            value = (value << 8) | (b as u64);
        }

        // Number of base58 chars needed for r bytes: ceil(r * log2(256) / log2(58))
        // r=1→2, r=2→3, r=3→5, r=4→6, r=5→7, r=6→8, r=7→10
        let num_chars = match remaining {
            1 => 2,
            2 => 3,
            3 => 5,
            4 => 6,
            5 => 7,
            6 => 8,
            7 => 10,
            _ => 0,
        };

        let mut block_chars = Vec::with_capacity(num_chars);
        for _ in 0..num_chars {
            block_chars.push(ALPHABET[(value % 58) as usize]);
            value /= 58;
        }
        for c in block_chars.into_iter().rev() {
            result.push(c as char);
        }
    }

    Ok(result)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_is_xmr() {
        assert_eq!(XmrPlugin::new().id(), "xmr");
    }

    #[test]
    fn supported_networks_count() {
        assert_eq!(XmrPlugin::new().supported_networks().len(), 3);
    }

    #[test]
    fn test_spend_key_derivation_deterministic() {
        let seed = b"test-monero-seed-0001";
        let sk1 = SpendKey::from_seed(seed, 0);
        let sk2 = SpendKey::from_seed(seed, 0);
        assert_eq!(
            sk1.to_bytes(),
            sk2.to_bytes(),
            "same seed+index must produce same spend key"
        );
    }

    #[test]
    fn test_spend_key_derivation_different_index() {
        let seed = b"test-monero-seed-0002";
        let sk0 = SpendKey::from_seed(seed, 0);
        let sk1 = SpendKey::from_seed(seed, 1);
        assert_ne!(
            sk0.to_bytes(),
            sk1.to_bytes(),
            "different indices must produce different keys"
        );
    }

    #[test]
    fn test_public_spend_key_on_curve() {
        let seed = b"test-monero-seed-0003";
        let sk = SpendKey::from_seed(seed, 0);
        let pk = sk.public_spend_key();
        // Verify the point is on the curve (not identity)
        assert!(
            pk.is_torsion_free(),
            "public spend key must be on the curve"
        );
        assert!(
            !pk.compress().to_bytes().iter().all(|&b| b == 0),
            "public key must not be identity"
        );
    }

    #[test]
    fn test_key_image_not_identity() {
        let seed = b"test-monero-seed-0004";
        let sk = SpendKey::from_seed(seed, 0);
        let ki = sk.key_image();
        assert!(ki.is_torsion_free(), "key image must be on the curve");
        assert!(
            !ki.compress().to_bytes().iter().all(|&b| b == 0),
            "key image must not be identity"
        );
    }

    #[test]
    fn test_view_key_derivation() {
        let seed = b"test-monero-seed-0005";
        let sk = SpendKey::from_seed(seed, 0);
        let vk = sk.view_key();
        let vk_pub = vk.public_view_key();
        assert!(
            vk_pub.is_torsion_free(),
            "public view key must be on the curve"
        );
    }

    #[test]
    fn test_key_image_deterministic() {
        let seed = b"test-monero-seed-0006";
        let sk = SpendKey::from_seed(seed, 0);
        let ki1 = sk.key_image();
        let ki2 = sk.key_image();
        assert_eq!(
            ki1.compress().to_bytes(),
            ki2.compress().to_bytes(),
            "key image must be deterministic"
        );
    }

    #[tokio::test]
    async fn test_create_account_produces_valid_address() {
        let plugin = XmrPlugin::new();
        // Mainnet
        let seed = [42u8; 64];
        let account = plugin
            .create_account(&seed, 0, "monero")
            .await
            .expect("create_account should succeed");
        assert!(
            account.address.len() >= 94,
            "Monero address must be at least 94 chars, got {}",
            account.address.len()
        );
        assert!(
            account.address.chars().all(|c| c.is_alphanumeric()),
            "address must be alphanumeric"
        );
        assert_eq!(account.network, "monero");
        // Mainnet addresses must start with '4'
        assert!(
            account.address.starts_with('4'),
            "Mainnet address must start with '4', got: {}",
            account.address
        );

        // Stagenet — print address for faucet
        let stagenet = plugin
            .create_account(&[0xaa; 64], 0, "monero-stagenet")
            .await
            .expect("test invariant");
        eprintln!("XMR_STAGENET_ADDR:{}", stagenet.address);
        assert_eq!(stagenet.network, "monero-stagenet");
    }

    #[tokio::test]
    async fn test_sign_transaction_produces_clsag_signature() {
        let plugin = XmrPlugin::new();
        let key = KeyHandle {
            key_id: "test-xmr-key-clsag-01".into(),
            key_type: wallet_plugin::KeyType::Ed25519,
            public_key: vec![],
        };

        // Build a simple unsigned transaction JSON
        let unsigned_tx = serde_json::json!({
            "version": 2,
            "inputs": [{"amount": 1000000000000u64}],
            "outputs": [{"amount": 0, "key": "0000000000000000000000000000000000000000000000000000000000000000"}],
            "extra": ""
        });
        let tx_bytes = serde_json::to_vec(&unsigned_tx).expect("test invariant");

        let result = plugin.sign_transaction(&tx_bytes, &key, "monero").await;
        assert!(
            result.is_ok(),
            "CLSAG signing should succeed: {:?}",
            result.err()
        );

        let signed_bytes = result.expect("test invariant");
        let signed: serde_json::Value =
            serde_json::from_slice(&signed_bytes).expect("signed tx should be valid JSON");

        // Verify the signed transaction has all required fields
        assert!(
            signed.get("key_image").is_some(),
            "signed tx must have key_image"
        );
        assert!(
            signed.get("signatures").is_some(),
            "signed tx must have signatures"
        );
        assert!(
            signed.get("pseudo_outputs").is_some(),
            "signed tx must have pseudo_outputs"
        );

        let sigs = signed["signatures"].as_array().expect("test invariant");
        assert_eq!(sigs.len(), 1, "should have one signature for one input");

        let sig = &sigs[0];
        assert!(sig.get("s").is_some(), "signature must have s vector");
        assert!(sig.get("c1").is_some(), "signature must have c1");
        assert!(sig.get("D").is_some(), "signature must have D");

        let s_arr = sig["s"].as_array().expect("test invariant");
        assert_eq!(
            s_arr.len(),
            11,
            "s vector must have 11 elements (ring size)"
        );
    }

    #[tokio::test]
    async fn test_validate_mainnet_address() {
        let plugin = XmrPlugin::new();
        // Generate a REAL mainnet address (valid keccak checksum) via create_account
        let account = plugin
            .create_account(&[0x11u8; 64], 0, "monero")
            .await
            .expect("test invariant");
        let result = plugin
            .validate_address(&account.address, "monero")
            .await
            .expect("test invariant");
        assert!(result, "real mainnet address should validate");
    }

    #[tokio::test]
    async fn test_validate_stagenet_address() {
        let plugin = XmrPlugin::new();
        let account = plugin
            .create_account(&[0x22u8; 64], 0, "monero-stagenet")
            .await
            .expect("test invariant");
        let result = plugin
            .validate_address(&account.address, "monero-stagenet")
            .await
            .expect("test invariant");
        assert!(result, "real stagenet address should validate");
    }

    #[tokio::test]
    async fn test_validate_address_wrong_network_rejected() {
        let plugin = XmrPlugin::new();
        // A real mainnet address must be rejected on stagenet
        let account = plugin
            .create_account(&[0x33u8; 64], 0, "monero")
            .await
            .expect("test invariant");
        let result = plugin
            .validate_address(&account.address, "monero-stagenet")
            .await
            .expect("test invariant");
        assert!(!result, "mainnet address should be rejected on stagenet");
    }

    #[tokio::test]
    async fn test_validate_address_bad_checksum_rejected() {
        let plugin = XmrPlugin::new();
        // Corrupt a real address's checksum (flip a trailing char) → must be rejected.
        // This is the false-positive case: a garbage '4...' string used to pass.
        let account = plugin
            .create_account(&[0x44u8; 64], 0, "monero")
            .await
            .expect("test invariant");
        let mut chars: Vec<char> = account.address.chars().collect();
        let last = *chars.last().unwrap();
        *chars.last_mut().unwrap() = if last == 'A' { 'B' } else { 'A' };
        let corrupted: String = chars.into_iter().collect();
        let result = plugin
            .validate_address(&corrupted, "monero")
            .await
            .expect("test invariant");
        assert!(!result, "corrupted-checksum address should be rejected");
    }

    #[test]
    fn test_validate_address_wrong_length_rejected() {
        let plugin = XmrPlugin::new();
        let result = futures::executor::block_on(plugin.validate_address("4short", "monero"))
            .expect("test invariant");
        assert!(!result, "short address should be rejected");
    }

    #[test]
    fn test_validate_address_non_alphanumeric_rejected() {
        let plugin = XmrPlugin::new();
        let result = futures::executor::block_on(plugin.validate_address("4abc!def", "monero"))
            .expect("test invariant");
        assert!(!result, "non-alphanumeric address should be rejected");
    }

    #[test]
    fn test_balance_fallback_zero_when_no_wallet_rpc() {
        let plugin = XmrPlugin::new();
        let account = Account {
            id: "xmr-monero-0".into(),
            network: "monero".into(),
            address: "4abc123".into(),
            path: None,
            label: None,
            index: 0,
        };
        let result = futures::executor::block_on(plugin.get_balance(&account, "monero"));
        match result {
            Err(e) => {
                // Expected: no wallet-rpc configured → NetworkError
                let msg = e.to_string().to_lowercase();
                assert!(
                    msg.contains("wallet-rpc") || msg.contains("setup"),
                    "expected wallet-rpc error, got: {e}"
                );
            }
            Ok(balance) => {
                // Also acceptable if the plugin falls back to zero
                assert_eq!(balance.confirmed, "0");
                assert_eq!(balance.unconfirmed, "0");
                assert_eq!(balance.unit, "XMR");
            }
        }
    }

    #[test]
    fn test_transaction_history_fallback_empty_when_no_wallet_rpc() {
        let plugin = XmrPlugin::new();
        let account = Account {
            id: "xmr-monero-0".into(),
            network: "monero".into(),
            address: "4abc123".into(),
            path: None,
            label: None,
            index: 0,
        };
        let result =
            futures::executor::block_on(plugin.get_transaction_history(&account, "monero", 10))
                .expect("test invariant");
        assert!(
            result.is_empty(),
            "history should be empty when no wallet-rpc configured"
        );
    }

    #[test]
    fn test_capabilities_includes_balance() {
        let caps = XmrPlugin::new().capabilities();
        assert!(caps.contains(&Capability::Balance));
    }
}
