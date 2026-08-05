use crate::isolation;
/// Tauri command handlers — the bridge between Svelte UI and Rust vault engine.
///
/// SECURITY: These commands route through the vault-core engine. The UI never
/// touches raw key material. All signing, key derivation, and cryptographic
/// operations happen exclusively in the Rust memory heap.
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::Emitter;
use tauri::State;
use tokio::sync::RwLock;
use vault_core::Vault;

/// Application state holding the vault engine + IPC lifecycle.
pub struct VaultState {
    pub vault: Arc<RwLock<Option<Vault>>>,
    pub ipc_port: u16,
    pub ipc_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    pub biometric_engine: Arc<dyn auth_core::BiometricEngine>,
    pub native_biometry_enabled: std::sync::atomic::AtomicBool,
    /// Consecutive biometric failure counter (resets on success)
    pub biometric_failures: std::sync::atomic::AtomicU8,
    pub fido2_authenticator: Arc<dyn auth_core::Fido2Authenticator>,
}

impl Default for VaultState {
    fn default() -> Self {
        Self {
            vault: Arc::new(RwLock::new(Some(Vault::new()))),
            ipc_port: 19876,
            ipc_handle: Arc::new(RwLock::new(None)),
            biometric_engine: Arc::new(auth_core::MockEngine),
            native_biometry_enabled: std::sync::atomic::AtomicBool::new(true),
            biometric_failures: std::sync::atomic::AtomicU8::new(0),
            fido2_authenticator: Arc::new(auth_core::MockFido2Authenticator),
        }
    }
}

impl VaultState {
    /// Builder: inject a real biometric engine (desktop Touch ID / Android
    /// BiometricPrompt). Defaults to `MockEngine`. Mirrors the `Vault`
    /// key-provider seam so Android wires a Tauri-plugin-backed engine here.
    // #[allow(dead_code)] — consumed by the Android Tauri setup (not the desktop
    // build path, so the Rust compiler flags it unused here).
    #[allow(dead_code)]
    pub fn with_biometric_engine(mut self, engine: Arc<dyn auth_core::BiometricEngine>) -> Self {
        self.biometric_engine = engine;
        self
    }

    /// Builder: inject a real FIDO2 authenticator (YubiKey / Android FIDO2).
    /// Defaults to `MockFido2Authenticator`.
    #[allow(dead_code)]
    pub fn with_fido2_authenticator(
        mut self,
        auth: Arc<dyn auth_core::Fido2Authenticator>,
    ) -> Self {
        self.fido2_authenticator = auth;
        self
    }
}

// ── Response types ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultStatus {
    pub initialized: bool,
    pub connected: bool,
    pub tor_enabled: bool,
    pub active_plugins: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub network: String,
    pub address: String,
    pub balance: String,
}

// ── Command Handlers ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_vault_status(
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<VaultStatus, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let initialized = vault_guard
        .as_ref()
        .map(|v| v.initialized.load(Ordering::SeqCst))
        .unwrap_or(false);
    let ipc_running = vs.ipc_handle.read().await.is_some();
    Ok(VaultStatus {
        initialized,
        connected: ipc_running,
        tor_enabled: false,
        active_plugins: vec!["btc".into(), "evm".into(), "xmr".into()],
    })
}

/// Biometric unlock — attempts to restore the vault from persisted keystore,
/// then prompts for biometric verification before revealing the dashboard.
/// On Android this bridges to the OS biometric prompt via the Tauri plugin.
#[tauri::command]
pub async fn biometric_unlock_vault(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<VaultStatus, String> {
    let vs = vault_state.read().await;
    let mut vault_guard = vs.vault.write().await;

    if vault_guard.is_some() {
        // Vault already loaded — just verify biometric
        let vault = vault_guard
            .as_ref()
            .expect("vault_guard.is_some() just checked");
        if vault.initialized.load(std::sync::atomic::Ordering::SeqCst) {
            // Prompt biometric auth
            match vs
                .biometric_engine
                .verify(auth_core::AuthStatus::BiometricUnlocked)
            {
                Ok(()) => {
                    vault
                        .auth_manager
                        .try_biometric()
                        .map_err(|e| format!("Auth error: {e}"))?;
                    vault.auth_manager.touch();
                    return Ok(VaultStatus {
                        initialized: true,
                        connected: vs.ipc_handle.read().await.is_some(),
                        tor_enabled: false,
                        active_plugins: vec!["btc".into(), "evm".into(), "xmr".into()],
                    });
                }
                Err(auth_core::AuthError::NotSupported) => {
                    // No biometric hardware — still allow access (fallback to no auth)
                    return Ok(VaultStatus {
                        initialized: true,
                        connected: vs.ipc_handle.read().await.is_some(),
                        tor_enabled: false,
                        active_plugins: vec!["btc".into(), "evm".into(), "xmr".into()],
                    });
                }
                Err(e) => return Err(format!("Biometric verification failed: {e}")),
            }
        }
    }

    // No vault loaded — try restoring from persisted keystore
    let mut vault = vault_core::Vault::new();
    vault
        .try_restore()
        .await
        .map_err(|e| format!("Failed to restore vault: {e}"))?;

    if vault.initialized.load(std::sync::atomic::Ordering::SeqCst) {
        // Prompt biometric before releasing the dashboard
        match vs
            .biometric_engine
            .verify(auth_core::AuthStatus::BiometricUnlocked)
        {
            Ok(()) => {
                vault
                    .auth_manager
                    .try_biometric()
                    .map_err(|e| format!("Auth error: {e}"))?;
                vault.auth_manager.touch();
                *vault_guard = Some(vault);
                Ok(VaultStatus {
                    initialized: true,
                    connected: vs.ipc_handle.read().await.is_some(),
                    tor_enabled: false,
                    active_plugins: vec!["btc".into(), "evm".into(), "xmr".into()],
                })
            }
            Err(auth_core::AuthError::NotSupported) => {
                *vault_guard = Some(vault);
                Ok(VaultStatus {
                    initialized: true,
                    connected: vs.ipc_handle.read().await.is_some(),
                    tor_enabled: false,
                    active_plugins: vec!["btc".into(), "evm".into(), "xmr".into()],
                })
            }
            Err(e) => Err(format!("Biometric verification failed: {e}")),
        }
    } else {
        Err("No persisted vault found — please create or restore a wallet".into())
    }
}

#[tauri::command]
pub async fn initialize_vault(
    seed: Option<String>,
    passphrase: Option<String>,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<VaultStatus, String> {
    let vs = state.read().await;
    let mut vault_guard = vs.vault.write().await;
    let vault = vault_guard.as_mut().ok_or("Vault state not available")?;

    let seed_phrase = seed.unwrap_or_default();
    let pass = passphrase.as_deref().unwrap_or("");
    vault
        .initialize(&seed_phrase, pass)
        .await
        .map_err(|e| e.to_string())?;

    Ok(VaultStatus {
        initialized: true,
        connected: vs.ipc_handle.read().await.is_some(),
        tor_enabled: false,
        active_plugins: vec!["btc".into(), "evm".into(), "xmr".into()],
    })
}

#[tauri::command]
pub async fn create_account(
    network: String,
    index: u32,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<AccountInfo, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    if !vault.initialized.load(Ordering::SeqCst) {
        return Err("Vault not initialized".into());
    }

    let account = vault
        .create_account(&network, index)
        .await
        .map_err(|e| e.to_string())?;
    Ok(AccountInfo {
        id: account.id,
        network: account.network,
        address: account.address,
        balance: "0".into(),
    })
}

#[tauri::command]
pub async fn get_balance(
    network: String,
    address: String,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<String, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    if !vault.initialized.load(Ordering::SeqCst) {
        return Err("Vault not initialized".into());
    }

    let account = wallet_plugin::Account {
        id: format!("{network}-query"),
        network: network.clone(),
        address,
        path: None,
        label: None,
    };
    let host = vault.plugin_host.read().await;
    let balance = host
        .get_balance(&account, &network)
        .await
        .map_err(|e| e.to_string())?;
    Ok(balance.confirmed)
}

#[tauri::command]
pub async fn sign_transaction(
    network: String,
    tx_hex: String,
    key_id: String,
    account_index: Option<u32>,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<String, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    let tx_bytes = hex::decode(&tx_hex).map_err(|e| format!("Invalid hex: {e}"))?;
    // Embed account index in key_id so BTC/LTC plugins derive the correct key
    let full_key_id = match account_index {
        Some(idx) => format!("{}@{}", key_id, idx),
        None => key_id.clone(),
    };
    let key = wallet_plugin::KeyHandle {
        key_id: full_key_id,
        key_type: wallet_plugin::KeyType::Secp256k1,
        public_key: Vec::new(),
    };
    let host = vault.plugin_host.read().await;
    let signed = host
        .sign_transaction(&tx_bytes, &key, &network)
        .await
        .map_err(|e| e.to_string())?;
    Ok(hex::encode(signed))
}

#[tauri::command]
pub async fn broadcast_transaction(
    network: String,
    signed_tx_hex: String,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<String, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    let tx_bytes = hex::decode(&signed_tx_hex).map_err(|e| format!("Invalid hex: {e}"))?;
    let host = vault.plugin_host.read().await;
    let txid = host
        .broadcast_transaction(&tx_bytes, &network)
        .await
        .map_err(|e| e.to_string())?;
    Ok(txid)
}

#[tauri::command]
pub async fn get_transaction_history(
    network: String,
    address: String,
    limit: Option<u32>,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<serde_json::Value, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    let account = wallet_plugin::Account {
        id: format!("{network}-hist"),
        network: network.clone(),
        address,
        path: None,
        label: None,
    };
    let host = vault.plugin_host.read().await;
    let records = host
        .get_transaction_history(&account, &network, limit.unwrap_or(10))
        .await
        .map_err(|e| e.to_string())?;
    let tx_list: Vec<serde_json::Value> = records
        .into_iter()
        .map(|r| {
            let status = match r.status {
                wallet_plugin::TxStatus::Pending => "pending",
                wallet_plugin::TxStatus::Confirmed { .. } => "confirmed",
                wallet_plugin::TxStatus::Failed { .. } => "failed",
            };
            json!({
                "txid": r.txid,
                "from": r.from_address.unwrap_or_default(),
                "to": r.to_address.unwrap_or_default(),
                "amount": r.amount,
                "blockHeight": r.block_height,
                "timestamp": r.timestamp,
                "status": status,
            })
        })
        .collect();
    Ok(json!({ "transactions": tx_list }))
}

// ── Simulation ─────────────────────────────────────────────────

/// Simulate an EVM transfer via eth_call before signing.
/// Takes simple params (network, from, to, value) — no raw tx hex needed.
/// Returns gas estimate and revert check result.
#[tauri::command]
pub async fn simulate_transfer(
    network: String,
    from: String,
    to: String,
    value: String,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<serde_json::Value, String> {
    let vs = state.read().await;
    let _vault_guard = vs.vault.read().await;

    // Map network id to chain_id + RPC endpoint
    let (rpc_url, _chain_id) = match network.as_str() {
        "ethereum" => ("https://ethereum-rpc.publicnode.com", 1u64),
        "arbitrum" => ("https://arbitrum-one-rpc.publicnode.com", 42161),
        "base" => ("https://base-rpc.publicnode.com", 8453),
        "polygon" => ("https://polygon-bor-rpc.publicnode.com", 137),
        "optimism" => ("https://optimism-rpc.publicnode.com", 10),
        "bnb" => ("https://bsc-rpc.publicnode.com", 56),
        "sepolia" => ("https://ethereum-sepolia-rpc.publicnode.com", 11155111),
        _ => return Err(format!("Unsupported network for simulation: {network}")),
    };

    // Build eth_call params
    let tx_obj = serde_json::json!({
        "from": from,
        "to": to,
        "value": format!("0x{:x}", value.parse::<u128>().unwrap_or(0)),
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    // Step 1: eth_estimateGas
    let gas_used: u64 = {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_estimateGas",
            "params": [tx_obj.clone()],
            "id": 1,
        });
        let resp = client
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Gas estimation HTTP failed: {e}"))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Gas estimation response invalid: {e}"))?;
        if let Some(err) = json.get("error") {
            let msg = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(format!("Gas estimation failed: {msg}"));
        }
        let hex = json
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("0x5208");
        u64::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(21000)
    };

    // Step 2: eth_call (latest block) to check for revert
    let (success, return_data) = {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [tx_obj, "latest"],
            "id": 2,
        });
        let resp = client
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("eth_call HTTP failed: {e}"))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("eth_call response invalid: {e}"))?;
        if let Some(err) = json.get("error") {
            let msg = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("reverted");
            (false, format!("reverted: {msg}"))
        } else {
            let hex = json.get("result").and_then(|v| v.as_str()).unwrap_or("0x");
            (true, hex.to_string())
        }
    };

    Ok(serde_json::json!({
        "success": success,
        "gasUsed": gas_used,
        "gasEstimate": format!("{}", gas_used),
        "returnData": return_data,
        "revertReason": if success { serde_json::Value::Null } else { serde_json::Value::String(return_data) },
    }))
}

// ── Phase 2: Next-gen vault_* commands ────────────────────────────────────

#[tauri::command]
pub async fn execute_batch(
    network: String,
    operations: Vec<serde_json::Value>,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<serde_json::Value, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    let host = vault.plugin_host.read().await;
    let hashes = host
        .execute_batch(&operations, &network)
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "userOpHashes": hashes,
        "status": "validated",
    }))
}

#[tauri::command]
pub async fn request_session_key(
    network: String,
    permissions: serde_json::Value,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<serde_json::Value, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    let host = vault.plugin_host.read().await;
    host.request_session_key(&permissions, &network)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn simulate_and_send(
    network: String,
    tx_hex: String,
    key_id: String,
    address: String,
    state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<serde_json::Value, String> {
    let vs = state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    let tx_bytes = hex::decode(&tx_hex).map_err(|e| format!("Invalid hex: {e}"))?;
    let key = wallet_plugin::KeyHandle {
        key_id,
        key_type: wallet_plugin::KeyType::Secp256k1,
        public_key: Vec::new(),
    };
    let account = wallet_plugin::Account {
        id: format!("{network}-sim"),
        network: network.clone(),
        address,
        path: None,
        label: None,
    };
    let host = vault.plugin_host.read().await;
    host.simulate_and_send(&tx_bytes, &key, &account, &network)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_accounts() -> Vec<AccountInfo> {
    // Accounts are managed client-side in the Svelte store.
    // The vault-core backend is the source of truth for derivation.
    vec![]
}

#[tauri::command]
pub async fn launch_ipc_server(state: State<'_, Arc<RwLock<VaultState>>>) -> Result<u16, String> {
    let vs = state.read().await;
    let handle_guard = vs.ipc_handle.write().await;
    if handle_guard.is_some() {
        return Ok(vs.ipc_port); // already running
    }
    drop(handle_guard);

    let mut vault_guard = vs.vault.write().await;
    let vault = vault_guard.as_mut().ok_or("Vault not available")?;
    let port = vs.ipc_port;
    vault.launch(port, None).await.map_err(|e| e.to_string())?;

    // Take the real IPC handle from vault and track it in VaultState
    let mut handle_guard = vs.ipc_handle.write().await;
    *handle_guard = vault.take_ipc_handle();
    Ok(port)
}

// ── Isolation Pattern Commands ──────────────────────────────────────────────

/// One-time key exchange for the isolation iframe.
/// Called once by the isolation iframe to obtain the per-session AES-GCM key.
/// After this call, `key_sent` is set to true and subsequent calls returns empty.
#[tauri::command]
pub async fn __get_isolation_key(
    isolation: State<'_, Arc<isolation::IsolationState>>,
) -> Result<String, String> {
    if isolation.key_sent.swap(true, Ordering::SeqCst) {
        return Err("Isolation key already delivered".into());
    }
    Ok(isolation.key_hex.clone())
}

/// Encrypted dispatch — the isolation iframe calls this for ALL vault operations.
///
/// Receives an encrypted payload, decrypts it, extracts the method name and
/// args, dispatches to the appropriate handler, then encrypts the response.
#[tauri::command]
pub async fn isolation_invoke(
    isolation: State<'_, Arc<isolation::IsolationState>>,
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
    method: String,
    encrypted: isolation::EncryptedPayload,
) -> Result<serde_json::Value, String> {
    // Decrypt the payload
    let args = isolation.decrypt(&encrypted).await?;

    // Dispatch to the appropriate handler
    let result = dispatch_method(&method, &args, &vault_state).await?;

    // Encrypt the response
    Ok(isolation::maybe_encrypt_response(&isolation, &result).await)
}

/// Dispatch a decrypted IPC call to the correct handler implementation.
/// This mirrors the Tauri command handlers but works with generic JSON params.
async fn dispatch_method(
    method: &str,
    args: &serde_json::Value,
    vault_state: &State<'_, Arc<RwLock<VaultState>>>,
) -> Result<serde_json::Value, String> {
    let vs = vault_state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    match method {
        "get_vault_status" | "vault.status" => {
            let initialized = vault.initialized.load(Ordering::SeqCst);
            let ipc_running = vs.ipc_handle.read().await.is_some();
            Ok(serde_json::json!({
                "initialized": initialized,
                "connected": ipc_running,
                "tor_enabled": false,
                "active_plugins": ["btc", "evm", "xmr"],
            }))
        }

        "initialize_vault" | "vault.initialize" => {
            let seed_phrase = args
                .get("seed_phrase")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let passphrase = args
                .get("passphrase")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            drop(vault_guard);
            let mut vault_guard = vs.vault.write().await;
            let vault = vault_guard.as_mut().ok_or("Vault not available")?;
            vault
                .initialize(seed_phrase, passphrase)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"success": true}))
        }

        "create_account" | "vault.create_account" => {
            let network = args
                .get("network")
                .and_then(|v| v.as_str())
                .ok_or("Missing network")?;
            let index = args.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let account = vault
                .create_account(network, index)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({
                "account": {
                    "id": account.id,
                    "network": account.network,
                    "address": account.address,
                    "balance": "0",
                }
            }))
        }

        "get_balance" | "vault.get_balance" => {
            let network = args
                .get("network")
                .and_then(|v| v.as_str())
                .ok_or("Missing network")?;
            let address = args
                .get("address")
                .and_then(|v| v.as_str())
                .ok_or("Missing address")?;
            let account = wallet_plugin::Account {
                id: format!("{network}-query"),
                network: network.to_string(),
                address: address.to_string(),
                path: None,
                label: None,
            };
            let host = vault.plugin_host.read().await;
            let balance = host
                .get_balance(&account, network)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"balance": balance.confirmed}))
        }

        "sign_transaction" | "vault.sign_transaction" => {
            let network = args
                .get("network")
                .and_then(|v| v.as_str())
                .ok_or("Missing network")?;
            let tx_hex = args
                .get("tx_hex")
                .and_then(|v| v.as_str())
                .ok_or("Missing tx_hex")?;
            let key_id = args
                .get("key_id")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let tx_bytes = hex::decode(tx_hex).map_err(|e| format!("Invalid hex: {e}"))?;
            let key = wallet_plugin::KeyHandle {
                key_id: key_id.to_string(),
                key_type: wallet_plugin::KeyType::Secp256k1,
                public_key: Vec::new(),
            };
            let host = vault.plugin_host.read().await;
            let signed = host
                .sign_transaction(&tx_bytes, &key, network)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"signed_tx_hex": hex::encode(signed)}))
        }

        "broadcast_transaction" | "vault.broadcast_transaction" => {
            let network = args
                .get("network")
                .and_then(|v| v.as_str())
                .ok_or("Missing network")?;
            let signed_tx_hex = args
                .get("signed_tx_hex")
                .and_then(|v| v.as_str())
                .ok_or("Missing signed_tx_hex")?;
            let tx_bytes = hex::decode(signed_tx_hex).map_err(|e| format!("Invalid hex: {e}"))?;
            let host = vault.plugin_host.read().await;
            let txid = host
                .broadcast_transaction(&tx_bytes, network)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"txid": txid}))
        }

        "get_transaction_history" | "vault.get_transaction_history" => {
            let network = args
                .get("network")
                .and_then(|v| v.as_str())
                .ok_or("Missing network")?;
            let address = args
                .get("address")
                .and_then(|v| v.as_str())
                .ok_or("Missing address")?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
            let account = wallet_plugin::Account {
                id: format!("{network}-hist"),
                network: network.to_string(),
                address: address.to_string(),
                path: None,
                label: None,
            };
            let host = vault.plugin_host.read().await;
            let records = host
                .get_transaction_history(&account, network, limit)
                .await
                .map_err(|e| e.to_string())?;
            let txs: Vec<serde_json::Value> = records
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "txid": r.txid,
                        "from": r.from_address.unwrap_or_default(),
                        "to": r.to_address.unwrap_or_default(),
                        "amount": r.amount,
                        "status": match r.status {
                            wallet_plugin::TxStatus::Pending => "pending",
                            wallet_plugin::TxStatus::Confirmed { .. } => "confirmed",
                            wallet_plugin::TxStatus::Failed { .. } => "failed",
                        },
                        "timestamp": r.timestamp,
                    })
                })
                .collect();
            Ok(serde_json::json!({"transactions": txs}))
        }

        "execute_batch" | "vault.execute_batch" => {
            let network = args
                .get("network")
                .and_then(|v| v.as_str())
                .ok_or("Missing network")?;
            let operations = args
                .get("operations")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let host = vault.plugin_host.read().await;
            let hashes = host
                .execute_batch(&operations, network)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"userOpHashes": hashes, "status": "validated"}))
        }

        "request_session_key" | "vault.request_session_key" => {
            let network = args
                .get("network")
                .and_then(|v| v.as_str())
                .ok_or("Missing network")?;
            let permissions = args
                .get("permissions")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let host = vault.plugin_host.read().await;
            let result = host
                .request_session_key(&permissions, network)
                .await
                .map_err(|e| e.to_string())?;
            Ok(result)
        }

        "simulate_and_send" | "vault.simulate_and_send" => {
            let network = args
                .get("network")
                .and_then(|v| v.as_str())
                .ok_or("Missing network")?;
            let tx_hex = args
                .get("tx_hex")
                .and_then(|v| v.as_str())
                .ok_or("Missing tx_hex")?;
            let key_id = args
                .get("key_id")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let address = args
                .get("address")
                .and_then(|v| v.as_str())
                .ok_or("Missing address")?;
            let tx_bytes = hex::decode(tx_hex).map_err(|e| format!("Invalid hex: {e}"))?;
            let key = wallet_plugin::KeyHandle {
                key_id: key_id.to_string(),
                key_type: wallet_plugin::KeyType::Secp256k1,
                public_key: Vec::new(),
            };
            let account = wallet_plugin::Account {
                id: format!("{network}-sim"),
                network: network.to_string(),
                address: address.to_string(),
                path: None,
                label: None,
            };
            let host = vault.plugin_host.read().await;
            let result = host
                .simulate_and_send(&tx_bytes, &key, &account, network)
                .await
                .map_err(|e| e.to_string())?;
            drop(host);
            Ok(result)
        }

        "launch_ipc_server" => {
            drop(vault_guard);
            let mut vault_guard = vs.vault.write().await;
            let vault = vault_guard.as_mut().ok_or("Vault not available")?;
            let port = vs.ipc_port;
            // On mobile (Android), the WebView talks to localhost via loopback
            // which is trusted — skip AES-GCM encryption overhead and avoid
            // WASM crypto dependency in the mobile WebView.
            vault.launch(port, None).await.map_err(|e| e.to_string())?;
            let mut handle_guard = vs.ipc_handle.write().await;
            *handle_guard = vault.take_ipc_handle();
            Ok(serde_json::json!({"port": port}))
        }

        _ => Err(format!("Unknown method: {method}")),
    }
}

// ── Tor Daemon Commands ──────────────────────────────────────────────────────

/// Toggle the Tor SOCKS5 proxy daemon on or off.
/// Returns the new state: `true` if Tor is now running.
#[tauri::command]
pub async fn toggle_tor(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
    enabled: bool,
) -> Result<bool, String> {
    let vs = vault_state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;

    if enabled {
        // Start Tor — the tor-daemon is already built into the workspace
        // vault-core's `set_tor_enabled(true)` will configure plugins to proxy through Tor
        vault
            .set_tor_enabled(true)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!("[tor] Tor daemon enabled");
        Ok(true)
    } else {
        vault
            .set_tor_enabled(false)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!("[tor] Tor daemon disabled");
        Ok(false)
    }
}

// ── Auth Commands ──────────────────────────────────────────────────────────

/// Lock the vault — resets auth status to Unauthenticated.
/// Emits a `security-lock` event so the frontend reacts immediately.
#[tauri::command]
pub async fn lock_vault(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let vs = vault_state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;
    vault.auth_manager.lock();
    tracing::info!("[auth] Vault locked by user");
    let _ = app_handle.emit("security-lock", serde_json::json!({"reason": "user"}));
    Ok(())
}

/// Confirm hardware authentication — routes through the BiometricEngine.
///
/// If the engine returns NotSupported, falls back to SoftwareAuth.
/// If PermissionDenied 5 times, disables native biometry for the session.
#[tauri::command]
pub async fn confirm_hardware(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    let vs = vault_state.read().await;

    // ── FIDO2 path (HardwareRequired) ─────────────────────────────────
    {
        let vault_guard = vs.vault.read().await;
        let vault = vault_guard.as_ref().ok_or("Vault not available")?;
        if vault.auth_manager.status() != auth_core::AuthStatus::HardwareRequired {
            // Not FIDO2 tier — fall through to biometric/software path
        } else {
            let fido2 = Arc::clone(&vs.fido2_authenticator);

            // Probe first (non-blocking)
            match fido2.probe() {
                auth_core::Fido2Status::DeviceNotFound => {
                    return Err("FIDO2_DEVICE_NOT_FOUND".into());
                }
                auth_core::Fido2Status::DeviceFound => { /* proceed */ }
                _ => return Err("Unexpected FIDO2 status".into()),
            }

            // Generate random 32-byte challenge
            let challenge: [u8; 32] = rand::random();

            // Spawn blocking HID I/O
            let result =
                tokio::task::spawn_blocking(move || fido2.authenticate(&challenge, 30_000))
                    .await
                    .map_err(|e| format!("FIDO2 task failed: {e}"))?
                    .map_err(|e| format!("FIDO2 error: {e}"))?;

            match result {
                auth_core::Fido2Status::AssertionReceived(_sig) => {
                    // Need to re-acquire vault reference
                    drop(vault_guard);
                    let vault_guard = vs.vault.read().await;
                    let vault = vault_guard.as_ref().ok_or("Vault not available")?;
                    vault
                        .auth_manager
                        .confirm_hardware()
                        .map_err(|e| format!("Auth error: {e}"))?;
                    vault.auth_manager.touch();
                    tracing::info!("[auth] FIDO2 authentication succeeded");
                    return Ok(true);
                }
                auth_core::Fido2Status::Timeout => {
                    tracing::warn!("[auth] FIDO2 timeout — locking vault");
                    drop(vault_guard);
                    let vault_guard = vs.vault.read().await;
                    let vault = vault_guard.as_ref().ok_or("Vault not available")?;
                    vault.auth_manager.lock();
                    let _ = app_handle.emit(
                        "security-lock",
                        serde_json::json!({"reason": "fido2_timeout"}),
                    );
                    return Err("FIDO2 authentication timed out".into());
                }
                _ => return Err("FIDO2 authentication failed".into()),
            }
        }
    } // vault_guard dropped here — FIDO2 path done

    // ── Tier 1: Biometric path ─────────────────────────────────────────
    if vs
        .native_biometry_enabled
        .load(std::sync::atomic::Ordering::Acquire)
    {
        match vs
            .biometric_engine
            .verify(auth_core::AuthStatus::BiometricUnlocked)
        {
            Ok(()) => {
                // Success — reset failure counter via the tested policy.
                let policy = auth_core::BiometricPolicy::new();
                let (outcome, _) =
                    policy.classify(Ok(()), vs.biometric_failures.load(Ordering::Acquire));
                debug_assert_eq!(outcome, auth_core::BiometricOutcome::Success);
                vs.biometric_failures
                    .store(0, std::sync::atomic::Ordering::Release);
                let vault_guard = vs.vault.read().await;
                let vault = vault_guard.as_ref().ok_or("Vault not available")?;
                vault
                    .auth_manager
                    .try_biometric()
                    .map_err(|e| format!("Auth error: {e}"))?;
                vault.auth_manager.touch();
                tracing::info!("[auth] Biometric authentication succeeded");
                return Ok(true);
            }
            Err(auth_core::AuthError::NotSupported) => {
                // Engine can't handle this — fall back to SoftwareAuth.
                // NotSupported does NOT count as a user denial (policy rule).
                vs.native_biometry_enabled
                    .store(false, std::sync::atomic::Ordering::Release);
                tracing::warn!(
                    "[auth] Biometric engine not supported — falling back to SoftwareAuth"
                );
            }
            Err(auth_core::AuthError::PermissionDenied) => {
                // Route the failure through the tested lockout policy.
                let policy = auth_core::BiometricPolicy::new();
                let prev = vs.biometric_failures.load(Ordering::Acquire);
                let (outcome, count) =
                    policy.classify(Err(auth_core::AuthError::PermissionDenied), prev);
                vs.biometric_failures
                    .store(count, std::sync::atomic::Ordering::Release);
                match outcome {
                    auth_core::BiometricOutcome::LockedOut => {
                        vs.native_biometry_enabled
                            .store(false, std::sync::atomic::Ordering::Release);
                        return Err("Biometric authentication failed 5 times. Falling back to manual confirmation.".into());
                    }
                    _ => {
                        tracing::warn!("[auth] Biometric denied ({count}/5)");
                        return Err("Biometric authentication denied".into());
                    }
                }
            }
            Err(auth_core::AuthError::BiometricFailed(msg)) => {
                tracing::error!("[auth] Biometric error: {msg}");
                return Err(format!("Biometric error: {msg}"));
            }
            Err(e) => {
                return Err(format!("Biometric error: {e}"));
            }
        }
    }

    // Fallback path: SoftwareAuth (no hardware needed)
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;
    vault
        .auth_manager
        .confirm_hardware()
        .map_err(|e| format!("Hardware confirmation failed: {e}"))?;
    vault.auth_manager.touch();
    tracing::info!("[auth] Software confirmation — vault unlocked");
    Ok(true)
}

/// Get remaining security stats for the UI.
/// Returns `{ remaining_seconds: u32, auth_status: string }`.
#[tauri::command]
pub async fn get_security_stats(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<serde_json::Value, String> {
    let vs = vault_state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;
    let remaining = vault.auth_manager.remaining_seconds();
    let status = vault.auth_manager.status().as_str();
    Ok(serde_json::json!({
        "remaining_seconds": remaining,
        "auth_status": status,
    }))
}

// ── Account Management ──────────────────────────────────────────────────────

/// Rename an account (label is stored client-side in localStorage).
/// This is a convenience command — the vault backend doesn't store labels.
#[tauri::command]
pub async fn rename_account(
    _vault_state: State<'_, Arc<RwLock<VaultState>>>,
    _network: String,
    _address: String,
    _label: String,
) -> Result<(), String> {
    Ok(())
}

/// Get the BIP-39 mnemonic phrase (for seed re-export in Settings).
/// Requires wallet to be initialized. Returns the full phrase.
#[tauri::command]
pub async fn get_seed_phrase(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<String, String> {
    let vs = vault_state.read().await;
    let vault_guard = vs.vault.read().await;
    let vault = vault_guard.as_ref().ok_or("Vault not available")?;
    vault
        .get_mnemonic()
        .await
        .ok_or("Vault not initialized".into())
}

// ── Bug Reporter ────────────────────────────────────────────────────────────

/// Build a GitHub issue URL with the sanitized crash payload and open it
/// in the user's default browser.
///
/// SAFETY: The reporter strips PII (local paths, IPs, hex key material)
/// before URL-encoding the markdown body. No seed data, keys, or
/// transaction details are ever included.
#[tauri::command]
pub async fn webview_log(level: String, message: String) -> Result<(), String> {
    // Bridge: route frontend console.* into the native/Rust log so WebView
    // errors surface in the terminal (desktop) and logcat (Android), not just
    // the web inspector. Best-effort — never fails the caller.
    let msg = format!("[webview:{level}] {message}");
    match level.as_str() {
        "error" | "warn" => {
            tracing::warn!("{msg}");
        }
        _ => {
            tracing::info!("{msg}");
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn report_bug(
    description: Option<String>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    use std::sync::LazyLock;
    use std::sync::Mutex;

    static LAST_CRASH: LazyLock<Mutex<Option<serde_json::Value>>> =
        LazyLock::new(|| Mutex::new(None));

    let mut body = String::new();
    body.push_str("## Bug Report\n\n");

    if let Some(desc) = &description {
        body.push_str(&format!("**Description:** {desc}\n\n"));
    }

    // Inject crash report if one exists
    if let Ok(guard) = LAST_CRASH.lock()
        && let Some(ref crash) = *guard
    {
        let sanitized = sanitize_crash_report(crash);
        body.push_str(&format!(
            "### Crash Report\n\n```json\n{}\n```\n\n",
            serde_json::to_string_pretty(&sanitized).unwrap_or_default()
        ));
    }

    // Read and rotate crash files from disk
    let crash_dir = dirs_next::home_dir()
        .unwrap_or_default()
        .join(".gullbur")
        .join("crashes");

    if let Ok(entries) = std::fs::read_dir(&crash_dir) {
        let mut crash_files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .collect();
        crash_files.sort_by_key(|e| e.path());
        // Take the most recent crash
        if let Some(latest) = crash_files.pop()
            && let Ok(raw) = std::fs::read_to_string(latest.path())
            && let Ok(crash) = serde_json::from_str::<serde_json::Value>(&raw)
        {
            let sanitized = sanitize_crash_report(&crash);
            if let Ok(mut guard) = LAST_CRASH.lock() {
                *guard = Some(crash.clone());
            }
            body.push_str(&format!(
                "### Crash Report\n\n```json\n{}\n```\n\n",
                serde_json::to_string_pretty(&sanitized).unwrap_or_default()
            ));
        }
    }

    body.push_str("### Environment\n\n");
    body.push_str(&format!(
        "- **Version:** {}\n- **OS:** {} {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    ));

    // Build the GitHub issue URL
    // Build the GitHub issue URL
    let repo = "MarcusBrammeier/gullbur-enclave"; // REPLACE_ME: actual GitHub org/repo
    let base_url = format!("https://github.com/{repo}/issues/new");

    // Build final URL with URL-encoded body
    let title = urlencoding("Bug Report: [describe the issue]");
    let labels = urlencoding("bug");
    let encoded_body = urlencoding(&body);
    let issue_url = format!("{base_url}?title={title}&labels={labels}&body={encoded_body}");

    // Open in default browser using tauri-plugin-shell
    use tauri_plugin_shell::ShellExt;
    let shell = app_handle.shell();
    #[allow(deprecated)]
    let _ = shell.open(&issue_url, None);

    Ok(issue_url)
}

/// Sanitize PII from a crash report JSON value.
/// Strips: local filesystem paths, IP addresses, hex key material (64+ chars).
fn sanitize_crash_report(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            let mut s = s.clone();
            // Remove home directory paths
            if let Some(home) = dirs_next::home_dir() {
                let home_str = home.to_string_lossy();
                s = s.replace(&*home_str, "$HOME");
            }
            // Remove /tmp paths
            let cleaned: String = s
                .split_whitespace()
                .map(|word| {
                    if word.starts_with('/') || word.starts_with("./") || word.starts_with("../") {
                        // Keep filename:line but strip absolute prefixes
                        if word.contains(':') && word.contains(".rs") {
                            // It's a source location — keep relative form
                            let rel = word.rsplit('/').next().unwrap_or(word);
                            rel.to_string()
                        } else {
                            // It's a raw path — mask it
                            "$PATH".to_string()
                        }
                    } else if word.len() >= 64 && word.chars().all(|c| c.is_ascii_hexdigit()) {
                        // Looks like key material
                        let (prefix, _) = word.split_at(8);
                        format!("{prefix}...")
                    } else if is_ip_address(word) {
                        "[REDACTED]".to_string()
                    } else {
                        word.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            serde_json::Value::String(cleaned)
        }
        serde_json::Value::Object(map) => {
            let sanitized: serde_json::Map<_, _> = map
                .iter()
                .map(|(k, v)| (k.clone(), sanitize_crash_report(v)))
                .collect();
            serde_json::Value::Object(sanitized)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sanitize_crash_report).collect())
        }
        other => other.clone(),
    }
}

/// Crude IP-address detector for sanitization.
fn is_ip_address(s: &str) -> bool {
    // IPv4: 4 octets separated by dots
    if s.chars().filter(|&c| c == '.').count() == 3
        && s.split('.').all(|octet| octet.parse::<u8>().is_ok())
    {
        return true;
    }
    // IPv6: contains at least two colons and only hex-ish chars + colons
    if s.contains("::") || s.chars().filter(|&c| c == ':').count() >= 2 {
        let hex_colon: Vec<char> = "0123456789abcdefABCDEF:".chars().collect();
        if s.chars().all(|c| hex_colon.contains(&c)) && s.len() >= 3 {
            return true;
        }
    }
    false
}

/// Percent-encode a string for use in a URL query parameter.
fn urlencoding(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 3 / 2);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            other => {
                result.push_str(&format!("%{other:02X}"));
            }
        }
    }
    result
}

// ── Vault File Management ───────────────────────────────────────────────

/// Open a vault from a custom keystore file path.
/// Replaces the current vault instance with one loaded from the given file.
#[tauri::command]
pub async fn open_vault_from_path(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
    path: String,
) -> Result<(), String> {
    let path_buf = std::path::PathBuf::from(&path);
    if !path_buf.exists() {
        return Err(format!("Keystore file not found: {path}"));
    }
    // Load the keystore from the file and restore vault state
    let encrypted =
        std::fs::read(&path_buf).map_err(|e| format!("Failed to read keystore: {e}"))?;
    let vs = vault_state.write().await;
    let mut vault = vault_core::Vault::new();
    // Copy the encrypted seed into vault so try_restore can decrypt it
    vault.set_encrypted_seed(encrypted);
    vault
        .try_restore()
        .await
        .map_err(|e| format!("Failed to open vault: {e}"))?;
    *vs.vault.write().await = Some(vault);
    tracing::info!("Vault opened from custom path: {path}");
    Ok(())
}

/// Open a vault from raw encrypted keystore bytes (content:// URI compatible).
/// Frontend reads the file content and passes bytes here — avoids Android
/// filesystem path issues with content:// URIs.
#[tauri::command]
pub async fn open_vault_from_bytes(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
    data: Vec<u8>,
) -> Result<(), String> {
    if data.is_empty() {
        return Err("Keystore data is empty".into());
    }
    let vs = vault_state.write().await;
    let mut vault = vault_core::Vault::new();
    vault.set_encrypted_seed(data.clone());
    vault
        .try_restore()
        .await
        .map_err(|e| format!("Failed to open vault: {e}"))?;
    *vs.vault.write().await = Some(vault);
    tracing::info!("Vault opened from raw bytes ({} bytes)", data.len());
    Ok(())
}

/// Export the current vault's encrypted keystore to a custom file path.
/// Returns the number of bytes written on success.
#[tauri::command]
pub async fn export_current_keystore(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
    destination: String,
) -> Result<usize, String> {
    let vs = vault_state.read().await;
    let vault_guard = vs.vault.read().await;
    let _vault = vault_guard.as_ref().ok_or("Vault not available")?;

    let seed_data = vault_core::Vault::load_persisted_seed()
        .ok_or("No persisted keystore found — vault may not be initialized")?;

    let dest = std::path::Path::new(&destination);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    std::fs::write(dest, &seed_data).map_err(|e| format!("Write failed: {e}"))?;
    tracing::info!("Keystore exported to: {destination} ({})", seed_data.len());
    Ok(seed_data.len())
}

// ── Update Checker ─────────────────────────────────────────────────────

/// Check the GitHub releases API for a newer version of Gullbúr Enclave.
#[derive(Debug, serde::Serialize)]
pub struct UpdateCheckResult {
    pub local_version: String,
    pub latest_version: String,
    pub up_to_date: bool,
    pub release_url: Option<String>,
    pub release_notes: Option<String>,
    pub prerelease: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn check_for_updates() -> UpdateCheckResult {
    let token = std::env::var("GULLBUR_UPDATE_TOKEN").ok();
    match update_checker::check_for_updates("MarcusBrammeier/gullbur-enclave", token.as_deref())
        .await
    {
        // REPLACE_ME
        Ok(Some(info)) => UpdateCheckResult {
            local_version: info.local_version,
            latest_version: info.release.tag_name.clone(),
            up_to_date: info.up_to_date,
            release_url: Some(info.release.html_url),
            release_notes: info.release.body,
            prerelease: info.release.prerelease,
            error: None,
        },
        Ok(None) => UpdateCheckResult {
            local_version: update_checker::CURRENT_VERSION.to_string(),
            latest_version: update_checker::CURRENT_VERSION.to_string(),
            up_to_date: true,
            release_url: None,
            release_notes: None,
            prerelease: false,
            error: Some("No releases found on GitHub".into()),
        },
        Err(e) => UpdateCheckResult {
            local_version: update_checker::CURRENT_VERSION.to_string(),
            latest_version: "unknown".into(),
            up_to_date: true,
            release_url: None,
            release_notes: None,
            prerelease: false,
            error: Some(e.to_string()),
        },
    }
}

// ── Debug Report ──────────────────────────────────────────────────────

/// Generate a privacy-safe debug report for bug triage.
///
/// Contents:
/// - Version info (commit, build date, OS)
/// - Loaded plugins and their config (networks — NO API keys)
/// - Account list (addresses only — NO seeds, NO private keys)
/// - Last N errors from the internal error log
/// - Plugin manifest versions
/// - Config state (non-sensitive sections)
///
/// This report is designed to be safe for sharing on GitHub issues.
/// NO seed phrases, private keys, or balances are included.
#[derive(Debug, serde::Serialize)]
pub struct DebugReport {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub build_date: String,
    pub plugins: Vec<DebugPluginInfo>,
    pub accounts: Vec<DebugAccountInfo>,
    pub env_config: DebugConfigInfo,
    pub recent_crashes: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Serialize)]
pub struct DebugPluginInfo {
    pub id: String,
    pub name: String,
    pub networks: Vec<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct DebugAccountInfo {
    pub network: String,
    pub address: String,
    pub path: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct DebugConfigInfo {
    pub testnet_only: bool,
    pub tor_enabled: bool,
    pub auto_lock_seconds: u32,
}

#[tauri::command]
pub async fn generate_debug_report(
    vault_state: State<'_, Arc<RwLock<VaultState>>>,
) -> Result<DebugReport, String> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    use std::time::SystemTime;
    let build_date = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string();

    // Collect plugin info
    let mut plugins = Vec::new();
    {
        let vs = vault_state.read().await;
        let vault_guard = vs.vault.read().await;
        if let Some(ref vault) = *vault_guard {
            let host = vault.plugin_host.read().await;
            for pid in host.plugin_ids() {
                if let Some(plugin) = host.get(&pid) {
                    let networks: Vec<String> = plugin
                        .supported_networks()
                        .iter()
                        .map(|n| n.id.clone())
                        .collect();
                    let caps: Vec<String> = plugin
                        .capabilities()
                        .iter()
                        .map(|c| format!("{c:?}"))
                        .collect();
                    plugins.push(DebugPluginInfo {
                        id: pid,
                        name: plugin.name().to_string(),
                        networks,
                        capabilities: caps,
                    });
                }
            }
        }
    }

    // Collect account info (addresses only — no balances, no keys)
    let mut accounts = Vec::new();
    {
        let vs = vault_state.read().await;
        let vault_guard = vs.vault.read().await;
        if let Some(ref vault) = *vault_guard {
            let host = vault.plugin_host.read().await;
            for acct in host.list_accounts() {
                accounts.push(DebugAccountInfo {
                    network: acct.network,
                    address: acct.address,
                    path: acct.path,
                });
            }
        }
    }

    // Read crash reports from disk
    let mut recent_crashes: Vec<serde_json::Value> = Vec::new();
    if let Some(home) = dirs_next::home_dir() {
        let crash_dir = home.join(".gullbur").join("crashes");
        if let Ok(entries) = std::fs::read_dir(&crash_dir) {
            let mut crash_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .collect();
            crash_files.sort_by_key(|e| e.path());
            // Take up to 3 most recent crashes
            for entry in crash_files.iter().rev().take(3) {
                if let Ok(raw) = std::fs::read_to_string(entry.path())
                    && let Ok(crash) = serde_json::from_str::<serde_json::Value>(&raw)
                {
                    let sanitized = sanitize_crash_report(&crash);
                    recent_crashes.push(sanitized);
                }
            }
        }
    }

    Ok(DebugReport {
        version,
        os,
        arch,
        build_date,
        plugins,
        accounts,
        env_config: DebugConfigInfo {
            testnet_only: false,
            tor_enabled: false,
            auto_lock_seconds: 30,
        },
        recent_crashes,
    })
}
