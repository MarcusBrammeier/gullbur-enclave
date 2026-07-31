//! JSON-RPC IPC handlers for vault-core operations.
//!
//! Registers all vault operations as JSON-RPC methods on the IPC
//! MessageHandler. Each handler is a closure that captures the
//! shared plugin_host, seed, initialized, and approval_queue Arc's.
//!
//! # Approval Gating
//!
//! State-changing operations that arrive via the extension relay
//! (with an `origin` param) are gated through the ApprovalQueue.
//! The handler submits an approval request and blocks until the
//! Tauri/Svelte UI responds. If denied, the handler returns an
//! RPC error. If approved, the operation proceeds normally.
//!
//! Operations without an `origin` (called directly by the Tauri
//! desktop app) bypass approval gating — the user is already
//! interacting with the app.

use ipc_core::handler::MessageHandler;
use ipc_protocol::RpcError;
use rand::TryRngCore;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use wallet_plugin::{Account, KeyHandle, KeyType};

use crate::approval::ApprovalQueue;
use crate::host::PluginHost;

/// Enforce a minimum authentication level for the given operation.
///
/// Called at the top of every sensitive handler. If the current
/// `AuthStatus` is below `required`, returns an `AuthRequired` error
/// that the frontend uses to trigger the appropriate auth prompt.
fn enforce_auth(
    auth: &auth_core::AuthManager,
    required: auth_core::AuthStatus,
) -> Result<(), RpcError> {
    // Check auto-lock timer before anything else.
    // If the timer expired, lock() was called and we're now Unauthenticated.
    if auth.check_and_lock() {
        return Err(RpcError::auth_required("unauthenticated"));
    }

    let current = auth.status();
    if current.can_sign() && required == auth_core::AuthStatus::BiometricUnlocked {
        return Ok(());
    }
    if current == auth_core::AuthStatus::HardwareRequired
        && required == auth_core::AuthStatus::HardwareRequired
    {
        return Ok(());
    }
    // Map the current status to a string for the error payload
    let level = match required {
        auth_core::AuthStatus::BiometricUnlocked => "biometric_unlocked",
        auth_core::AuthStatus::HardwareRequired => "hardware_required",
        _ => "unauthenticated",
    };
    Err(RpcError::auth_required(level))
}

/// Register all vault JSON-RPC methods on the given handler.
///
/// Captures the shared Arc'd state so each handler closure can
/// operate independently without borrowing the Vault struct.
pub fn register_vault_handlers(
    handler: &mut MessageHandler,
    plugin_host: Arc<RwLock<PluginHost>>,
    seed: Arc<RwLock<Option<zeroize::Zeroizing<Vec<u8>>>>>,
    mnemonic: Arc<RwLock<Option<String>>>,
    initialized: Arc<AtomicBool>,
    approval_queue: Arc<RwLock<ApprovalQueue>>,
    auth_manager: Arc<auth_core::AuthManager>,
) {
    // ── vault.initialize ────────────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        let sd = Arc::clone(&seed);
        let mn = Arc::clone(&mnemonic);
        let init = Arc::clone(&initialized);
        handler.register("vault.initialize", move |params: Value| {
            let ph = Arc::clone(&ph);
            let sd = Arc::clone(&sd);
            let mn = Arc::clone(&mn);
            let init = Arc::clone(&init);
            async move {
                if init.load(Ordering::SeqCst) {
                    return Err(RpcError::new(-32000, "Vault is already initialized"));
                }

                let seed_phrase = params
                    .get("seed_phrase")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?;
                let passphrase = params
                    .get("passphrase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Process seed phrase through BIP-39 → BIP-44 (matching Vault::initialize)
                let (phrase_str, seed_512): (String, [u8; 64]) = if seed_phrase.is_empty() {
                    // Generate a new BIP-39 mnemonic and derive the 512-bit seed
                    let phrase = crypto_core::keys::generate_mnemonic(
                        crypto_core::MnemonicStrength::TwentyFourWords,
                    )
                    .map_err(|e| {
                        RpcError::new(-32000, format!("Mnemonic generation failed: {e}"))
                    })?;
                    let phrase_str = phrase.to_string();
                    let seed = crypto_core::keys::mnemonic_to_seed(phrase.as_words(), passphrase)
                        .map_err(|e| {
                        RpcError::new(-32000, format!("Seed derivation failed: {e}"))
                    })?;
                    (phrase_str, *seed)
                } else {
                    // Parse an existing BIP-39 mnemonic
                    let phrase = crypto_core::keys::mnemonic_from_string(seed_phrase)
                        .map_err(|e| RpcError::new(-32000, format!("Invalid mnemonic: {e}")))?;
                    let phrase_str = phrase.to_string();
                    let seed = crypto_core::keys::mnemonic_to_seed(phrase.as_words(), passphrase)
                        .map_err(|e| {
                        RpcError::new(-32000, format!("Seed derivation failed: {e}"))
                    })?;
                    (phrase_str, *seed)
                };

                // Derive master key via BIP-44 path: m/44'/60'/0'/0/0
                let master_secret = crypto_core::keys::derive_bip44_eth_key(&seed_512, 0)
                    .map_err(|e| RpcError::new(-32000, format!("Key derivation failed: {e}")))?;
                let master_key = crypto_core::KeyHandle {
                    key_id: "bip44-eth-0".to_string(),
                    key_type: crypto_core::KeyType::Secp256k1,
                    public_key: master_secret.public_key().to_sec1_bytes().to_vec(),
                };

                // Encrypt seed (random per-device key, same approach as vault-core)
                let mut dev_key = [0u8; 32];
                rand::rngs::OsRng.try_fill_bytes(&mut dev_key).ok();
                let _encrypted =
                    keystore_core::vault::encrypt_with_password(&dev_key, &seed_512, b"vault-seed")
                        .map_err(|e| RpcError::new(-32000, format!("Encryption failed: {e}")))?;

                // Store seed (64-byte BIP-39 seed)
                *sd.write().await = Some(zeroize::Zeroizing::new(seed_512.to_vec()));
                // Store mnemonic for later retrieval
                *mn.write().await = Some(phrase_str.clone());

                // Initialize default plugins
                {
                    let mut host = ph.write().await;
                    host.initialize_default_plugins();
                }

                init.store(true, Ordering::SeqCst);

                let mut resp = serde_json::json!({
                    "success": true,
                    "initialized": true,
                    "master_key": serde_json::to_value(&master_key)
                        .map_err(|e| RpcError::new(-32000, format!("Serialization failed: {e}")))?,
                });

                // Only return mnemonic when we generated a new one (seed_phrase was empty)
                if seed_phrase.is_empty() {
                    resp["mnemonic"] = serde_json::Value::String(phrase_str);
                }

                Ok(resp)
            }
        });
    }

    // ── vault.generate_mnemonic ───────────────────────────────────────────
    handler.register("vault.generate_mnemonic", |_params: Value| async move {
        let phrase =
            crypto_core::keys::generate_mnemonic(crypto_core::MnemonicStrength::TwentyFourWords)
                .map_err(|e| RpcError::new(-32000, e.to_string()))?;
        Ok(serde_json::json!({ "mnemonic": phrase.to_string() }))
    });

    // ── vault.status ────────────────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        let init = Arc::clone(&initialized);
        handler.register("vault.status", move |_params: Value| {
            let ph = Arc::clone(&ph);
            let init = Arc::clone(&init);
            async move {
                let initialized = init.load(Ordering::SeqCst);
                let host = ph.read().await;
                let plugin_ids = host.plugin_ids();
                let mut networks = Vec::new();
                for pid in &plugin_ids {
                    if let Some(plugin) = host.get(pid) {
                        networks.extend_from_slice(plugin.supported_networks());
                    }
                }
                let accounts = host.list_accounts();

                Ok(serde_json::json!({
                    "initialized": initialized,
                    "status": if initialized { "Initialized" } else { "Connected" },
                    "plugin_ids": plugin_ids,
                    "networks": networks,
                    "accounts": accounts,
                }))
            }
        });
    }

    // ── vault.create_account ────────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        let sd = Arc::clone(&seed);
        let aq = Arc::clone(&approval_queue);
        handler.register("vault.create_account", move |params: Value| {
            let ph = Arc::clone(&ph);
            let sd = Arc::clone(&sd);
            let aq = Arc::clone(&aq);
            async move {
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let index = params
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(RpcError::invalid_params)? as u32;

                // Approval gate: if called from extension with origin,
                // require user consent before deriving an account.
                let origin = params.get("origin").and_then(|v| v.as_str());
                if let Some(origin) = origin {
                    let mut summary = HashMap::new();
                    summary.insert("network".into(), network.clone());
                    let (approval_id, rx) = {
                        let mut queue = aq.write().await;
                        queue.submit(
                            origin,
                            "vault.create_account",
                            "dApp is requesting access to your Ethereum accounts",
                            summary,
                        )
                    };
                    tracing::info!(
                        "Approval requested: {} for {} from {}",
                        approval_id,
                        "vault.create_account",
                        origin
                    );
                    match rx.await {
                        Ok(crate::approval::ApprovalResponse::Approved) => {
                            tracing::info!("Approval granted: {}", approval_id);
                        }
                        Ok(crate::approval::ApprovalResponse::Denied(reason)) => {
                            tracing::info!("Approval denied: {} ({})", approval_id, reason);
                            return Err(RpcError::new(-32001, format!("User denied: {reason}")));
                        }
                        Err(_) => {
                            return Err(RpcError::new(-32000, "Approval request timed out"));
                        }
                    }
                }

                let seed_guard = sd.read().await;
                let seed = seed_guard
                    .as_ref()
                    .map(|s| s.as_slice())
                    .ok_or_else(|| RpcError::new(-32000, "Vault is not initialized"))?;
                let seed_vec = seed.to_vec();
                drop(seed_guard);

                let host = ph.read().await;
                let account = host
                    .create_account(&seed_vec, index, &network)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("Account creation failed: {e}")))?;

                serde_json::to_value(&account)
                    .map_err(|e| RpcError::new(-32000, format!("Serialization failed: {e}")))
            }
        });
    }

    // ── vault.get_balance ───────────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        handler.register("vault.get_balance", move |params: Value| {
            let ph = Arc::clone(&ph);
            async move {
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let address = params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();

                let account = Account {
                    id: format!("{network}-temp"),
                    network: network.clone(),
                    address,
                    path: None,
                    label: None,
                };

                let host = ph.read().await;
                let balance = host
                    .get_balance(&account, &network)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("Balance fetch failed: {e}")))?;

                serde_json::to_value(&balance)
                    .map_err(|e| RpcError::new(-32000, format!("Serialization failed: {e}")))
            }
        });
    }

    // ── vault.sign_transaction ──────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        let aq = Arc::clone(&approval_queue);
        let auth = Arc::clone(&auth_manager);
        handler.register("vault.sign_transaction", move |params: Value| {
            let ph = Arc::clone(&ph);
            let aq = Arc::clone(&aq);
            let auth = Arc::clone(&auth);
            async move {
                enforce_auth(&auth, auth_core::AuthStatus::BiometricUnlocked)?;
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let tx_hex = params
                    .get("tx_hex")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?;
                let key_id = params
                    .get("key_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let key_type_str = params
                    .get("key_type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?;

                // Approval gate: if called from extension with origin,
                // require user consent before signing.
                let origin = params.get("origin").and_then(|v| v.as_str());
                if let Some(origin) = origin {
                    let mut summary = HashMap::new();
                    summary.insert("network".into(), network.clone());
                    summary.insert("key".into(), key_id.clone());
                    let (approval_id, rx) = {
                        let mut queue = aq.write().await;
                        queue.submit(
                            origin,
                            "vault.sign_transaction",
                            "Confirm transaction in Gullbúr Enclave",
                            summary,
                        )
                    };
                    tracing::info!(
                        "Approval requested: {} for sign_transaction from {}",
                        approval_id,
                        origin
                    );
                    match rx.await {
                        Ok(crate::approval::ApprovalResponse::Approved) => {
                            tracing::info!("Approval granted: {}", approval_id);
                        }
                        Ok(crate::approval::ApprovalResponse::Denied(reason)) => {
                            tracing::info!("Approval denied: {} ({})", approval_id, reason);
                            return Err(RpcError::new(-32001, format!("User denied: {reason}")));
                        }
                        Err(_) => {
                            return Err(RpcError::new(-32000, "Approval request timed out"));
                        }
                    }
                }

                let tx_bytes = hex::decode(tx_hex)
                    .map_err(|e| RpcError::new(-32000, format!("Invalid tx_hex: {e}")))?;

                let key_type = match key_type_str {
                    "Secp256k1" => KeyType::Secp256k1,
                    "Ed25519" => KeyType::Ed25519,
                    _ => return Err(RpcError::invalid_params()),
                };

                // Append account index to key_id so BTC/LTC plugins derive the correct key.
                // Format: "hex_seed@index" — the plugin parses this to derive at the right BIP-84 path.
                // If no account_index is provided, defaults to 0 (backward compatible).
                let account_index = params
                    .get("account_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let full_key_id = format!("{}@{}", key_id, account_index);

                let key_handle = KeyHandle {
                    key_id: full_key_id,
                    key_type,
                    public_key: Vec::new(),
                };

                let host = ph.read().await;
                let signed = host
                    .sign_transaction(&tx_bytes, &key_handle, &network)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("Signing failed: {e}")))?;

                Ok(serde_json::json!({
                    "signed_tx_hex": hex::encode(signed),
                }))
            }
        });
    }

    // ── vault.broadcast_transaction ─────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        let auth = Arc::clone(&auth_manager);
        handler.register("vault.broadcast_transaction", move |params: Value| {
            let ph = Arc::clone(&ph);
            let auth = Arc::clone(&auth);
            async move {
                enforce_auth(&auth, auth_core::AuthStatus::BiometricUnlocked)?;
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let signed_tx_hex = params
                    .get("signed_tx_hex")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?;

                let tx_bytes = hex::decode(signed_tx_hex)
                    .map_err(|e| RpcError::new(-32000, format!("Invalid hex: {e}")))?;

                let host = ph.read().await;
                let txid = host
                    .broadcast_transaction(&tx_bytes, &network)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("Broadcast failed: {e}")))?;

                Ok(serde_json::json!({
                    "txid": txid,
                }))
            }
        });
    }

    // ── vault.get_transaction_history ───────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        handler.register("vault.get_transaction_history", move |params: Value| {
            let ph = Arc::clone(&ph);
            async move {
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let address = params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as u32;

                let account = Account {
                    id: format!("{network}-temp"),
                    network: network.clone(),
                    address,
                    path: None,
                    label: None,
                };

                let host = ph.read().await;
                let records = host
                    .get_transaction_history(&account, &network, limit)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("History fetch failed: {e}")))?;

                serde_json::to_value(&records)
                    .map_err(|e| RpcError::new(-32000, format!("Serialization failed: {e}")))
            }
        });
    }

    // ── vault.estimate_fee ──────────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        handler.register("vault.estimate_fee", move |params: Value| {
            let ph = Arc::clone(&ph);
            async move {
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();

                let host = ph.read().await;
                let estimate = host
                    .estimate_fee(&[], &network)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("Fee estimation failed: {e}")))?;

                serde_json::to_value(&estimate)
                    .map_err(|e| RpcError::new(-32000, format!("Serialization failed: {e}")))
            }
        });
    }

    // ── vault.list_networks ─────────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        handler.register("vault.list_networks", move |_params: Value| {
            let ph = Arc::clone(&ph);
            async move {
                let host = ph.read().await;
                let plugin_ids = host.plugin_ids();
                let mut networks = Vec::new();
                for pid in &plugin_ids {
                    if let Some(plugin) = host.get(pid) {
                        networks.extend_from_slice(plugin.supported_networks());
                    }
                }
                serde_json::to_value(&networks)
                    .map_err(|e| RpcError::new(-32000, format!("Serialization failed: {e}")))
            }
        });
    }

    // ── vault.validate_address ──────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        handler.register("vault.validate_address", move |params: Value| {
            let ph = Arc::clone(&ph);
            async move {
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let address = params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();

                let host = ph.read().await;
                let valid = host
                    .validate_address(&address, &network)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("Validation failed: {e}")))?;

                Ok(serde_json::json!({
                    "valid": valid,
                }))
            }
        });
    }

    // ── vault.lock ───────────────────────────────────────────────────────
    {
        let auth = Arc::clone(&auth_manager);
        handler.register("vault.lock", move |_params: Value| {
            let auth = Arc::clone(&auth);
            async move {
                auth.lock();
                Ok(serde_json::json!({ "locked": true }))
            }
        });
    }

    // ── vault.list_accounts ──────────────────────────────────────────────
    {
        let ph = Arc::clone(&plugin_host);
        handler.register("vault.list_accounts", move |_params: Value| {
            let ph = Arc::clone(&ph);
            async move {
                let host = ph.read().await;
                Ok(serde_json::to_value(host.list_accounts()).unwrap_or_default())
            }
        });
    }

    // ── Channel B: Next-gen vault_* API (Phase 2) ────────────────────
    // All three methods are ALWAYS approval-gated when accessed via IPC
    // (they're extension-only operations). The origin must be present.

    // vault_executeBatch — ERC-4337 bundler
    {
        let ph = Arc::clone(&plugin_host);
        let aq = Arc::clone(&approval_queue);
        let auth = Arc::clone(&auth_manager);
        handler.register("vault_executeBatch", move |params: Value| {
            let ph = Arc::clone(&ph);
            let aq = Arc::clone(&aq);
            let auth = Arc::clone(&auth);
            async move {
                enforce_auth(&auth, auth_core::AuthStatus::HardwareRequired)?;
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let operations = params
                    .get("operations")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| RpcError::new(-32000, "operations must be a non-empty array"))?;

                // Approval gate: these are always extension-originated
                let origin = params
                    .get("origin")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let mut summary = HashMap::new();
                summary.insert("network".into(), network.clone());
                summary.insert("count".into(), operations.len().to_string());
                let (_approval_id, rx) = {
                    let mut queue = aq.write().await;
                    queue.submit(
                        origin,
                        "vault_executeBatch",
                        &format!(
                            "Execute batch of {} transactions via ERC-4337 bundler",
                            operations.len()
                        ),
                        summary,
                    )
                };
                match rx.await {
                    Ok(crate::approval::ApprovalResponse::Approved) => {}
                    Ok(crate::approval::ApprovalResponse::Denied(reason)) => {
                        return Err(RpcError::new(-32001, format!("User denied: {reason}")));
                    }
                    Err(_) => {
                        return Err(RpcError::new(-32000, "Approval request timed out"));
                    }
                }

                let host = ph.read().await;
                let hashes = host
                    .execute_batch(operations, &network)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("Batch execution failed: {e}")))?;

                Ok(serde_json::json!({
                    "userOpHashes": hashes,
                    "status": "validated",
                }))
            }
        });
    }

    // vault_requestSessionKey — ERC-7579 session key engine
    {
        let ph = Arc::clone(&plugin_host);
        let aq = Arc::clone(&approval_queue);
        let auth = Arc::clone(&auth_manager);
        handler.register("vault_requestSessionKey", move |params: Value| {
            let ph = Arc::clone(&ph);
            let aq = Arc::clone(&aq);
            let auth = Arc::clone(&auth);
            async move {
                enforce_auth(&auth, auth_core::AuthStatus::HardwareRequired)?;
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let permissions = params
                    .get("permissions")
                    .ok_or_else(|| RpcError::new(-32000, "permissions object is required"))?;

                // Approval gate
                let origin = params
                    .get("origin")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let mut summary = HashMap::new();
                summary.insert("network".into(), network.clone());
                if let Some(methods) = permissions.get("allowedMethods").and_then(|v| v.as_array())
                {
                    summary.insert("methods".into(), methods.len().to_string());
                }
                let (_approval_id, rx) = {
                    let mut queue = aq.write().await;
                    queue.submit(
                        origin,
                        "vault_requestSessionKey",
                        "Generate ERC-7579 session key for dApp",
                        summary,
                    )
                };
                match rx.await {
                    Ok(crate::approval::ApprovalResponse::Approved) => {}
                    Ok(crate::approval::ApprovalResponse::Denied(reason)) => {
                        return Err(RpcError::new(-32001, format!("User denied: {reason}")));
                    }
                    Err(_) => {
                        return Err(RpcError::new(-32000, "Approval request timed out"));
                    }
                }

                let host = ph.read().await;
                let session_key = host
                    .request_session_key(permissions, &network)
                    .await
                    .map_err(|e| {
                        RpcError::new(-32000, format!("Session key generation failed: {e}"))
                    })?;

                Ok(session_key)
            }
        });
    }

    // vault_simulateAndSend — EVM fork simulator + broadcast
    {
        let ph = Arc::clone(&plugin_host);
        let aq = Arc::clone(&approval_queue);
        let auth = Arc::clone(&auth_manager);
        handler.register("vault_simulateAndSend", move |params: Value| {
            let ph = Arc::clone(&ph);
            let aq = Arc::clone(&aq);
            let auth = Arc::clone(&auth);
            async move {
                enforce_auth(&auth, auth_core::AuthStatus::HardwareRequired)?;
                let network = params
                    .get("network")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let tx_hex = params
                    .get("tx_hex")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?;
                let key_id = params
                    .get("key_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();
                let key_type_str = params
                    .get("key_type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?;
                let address = params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(RpcError::invalid_params)?
                    .to_string();

                // Approval gate
                let origin = params
                    .get("origin")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let mut summary = HashMap::new();
                summary.insert("network".into(), network.clone());
                summary.insert("key".into(), key_id.clone());
                let (_approval_id, rx) = {
                    let mut queue = aq.write().await;
                    queue.submit(
                        origin,
                        "vault_simulateAndSend",
                        "Simulate transaction and send with biometric signature",
                        summary,
                    )
                };
                match rx.await {
                    Ok(crate::approval::ApprovalResponse::Approved) => {}
                    Ok(crate::approval::ApprovalResponse::Denied(reason)) => {
                        return Err(RpcError::new(-32001, format!("User denied: {reason}")));
                    }
                    Err(_) => {
                        return Err(RpcError::new(-32000, "Approval request timed out"));
                    }
                }

                let tx_bytes = hex::decode(tx_hex)
                    .map_err(|e| RpcError::new(-32000, format!("Invalid tx_hex: {e}")))?;

                let key_type = match key_type_str {
                    "Secp256k1" => wallet_plugin::KeyType::Secp256k1,
                    "Ed25519" => wallet_plugin::KeyType::Ed25519,
                    _ => return Err(RpcError::invalid_params()),
                };

                let key_handle = wallet_plugin::KeyHandle {
                    key_id,
                    key_type,
                    public_key: Vec::new(),
                };

                let account = wallet_plugin::Account {
                    id: format!("{network}-sim"),
                    network: network.clone(),
                    address,
                    path: None,
                    label: None,
                };

                let host = ph.read().await;
                let result = host
                    .simulate_and_send(&tx_bytes, &key_handle, &account, &network)
                    .await
                    .map_err(|e| RpcError::new(-32000, format!("Simulate-and-send failed: {e}")))?;

                Ok(result)
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth_core::AuthStatus;

    #[test]
    fn enforce_auth_allows_biometric_unlocked() {
        let auth = auth_core::AuthManager::new();
        auth.try_biometric().expect("test invariant");
        assert!(enforce_auth(&auth, AuthStatus::BiometricUnlocked).is_ok());
    }

    #[test]
    fn enforce_auth_rejects_unauthenticated_for_signing() {
        let auth = auth_core::AuthManager::new();
        let result = enforce_auth(&auth, AuthStatus::BiometricUnlocked);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32002);
    }

    #[test]
    fn enforce_auth_allows_hardware_required() {
        let auth = auth_core::AuthManager::new();
        auth.try_biometric().expect("test invariant");
        auth.request_hardware().expect("test invariant");
        assert!(enforce_auth(&auth, AuthStatus::HardwareRequired).is_ok());
    }

    #[test]
    fn enforce_auth_rejects_hardware_required_when_unauthenticated() {
        let auth = auth_core::AuthManager::new();
        let result = enforce_auth(&auth, AuthStatus::HardwareRequired);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32002);
    }

    #[test]
    fn enforce_auth_rejects_after_lock() {
        let auth = auth_core::AuthManager::new();
        auth.try_biometric().expect("test invariant");
        auth.lock();
        let result = enforce_auth(&auth, AuthStatus::BiometricUnlocked);
        assert!(result.is_err());
    }

    #[test]
    fn enforce_auth_status_after_try_biometric_is_unlocked() {
        let auth = auth_core::AuthManager::new();
        auth.try_biometric().expect("test invariant");
        assert_eq!(auth.status(), auth_core::AuthStatus::BiometricUnlocked);
    }

    #[test]
    fn enforce_auth_status_after_request_hardware() {
        let auth = auth_core::AuthManager::new();
        auth.try_biometric().expect("test invariant");
        auth.request_hardware().expect("test invariant");
        assert_eq!(auth.status(), auth_core::AuthStatus::HardwareRequired);
    }
}
