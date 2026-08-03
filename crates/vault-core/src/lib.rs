//! Gullbúr Enclave — Vault Engine
//!
//! The vault-core is the headless orchestrator that ties together
//! all the modular crates. It hosts blockchain plugins, manages
//! the IPC server lifecycle, coordinates the Tor daemon, and
//! exposes a unified API for the Tauri desktop shell.
//!
//! ## Architecture
//!
//! ```text
//! Tauri Desktop Shell (Svelte 5)
//!         │
//!         ▼
//!    vault-core (this crate)
//!    ┌──────────────────────────┐
//!    │  PluginHost              │
//!    │  ├── BtcPlugin           │
//!    │  ├── EvmPlugin           │
//!    │  └── XmrPlugin           │
//!    ├──────────────────────────┤
//!    │  VaultManager            │
//!    │  ├── crypto-core         │
//!    │  ├── keystore-core       │
//!    │  └── auth-core           │
//!    ├──────────────────────────┤
//!    │  IpcLifecycle            │
//!    │  ├── ipc-core (internal) │
//!    │  └── Extension bridge    │
//!    ├──────────────────────────┤
//!    │  TorManager              │
//!    │  └── tor-daemon          │
//!    └──────────────────────────┘
//! ```

pub mod approval;
pub mod error;
pub mod host;
pub mod ipc_handlers;
pub mod lifecycle;
pub mod vault_bridge;
pub mod xmr_downloader;
pub mod xmr_wallet_rpc;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use zeroize::Zeroizing;

pub use error::VaultError;

/// Persisted seed path relative to ~/.gullbur/
const SEED_FILE: &str = "keystore";

/// The central vault engine — single entry point for all operations.
pub struct Vault {
    /// Plugin host managing all blockchain integrations
    pub plugin_host: Arc<RwLock<host::PluginHost>>,
    /// IPC server handle (started on `launch()`)
    ipc_handle: Option<tokio::task::JoinHandle<()>>,
    /// Whether the vault has been initialized with a seed
    pub initialized: Arc<AtomicBool>,
    /// Raw 64-byte BIP-39 seed (stored after initialization; zeroized on drop)
    seed: Arc<RwLock<Option<Zeroizing<Vec<u8>>>>>,
    /// BIP-39 mnemonic phrase (for display/recovery)
    mnemonic: Arc<RwLock<Option<String>>>,
    /// Encrypted seed blob (salt || nonce || ciphertext)
    encrypted_seed: Arc<RwLock<Option<Vec<u8>>>>,
    /// Approval queue for user-consent gating of state-changing operations
    pub approval_queue: Arc<RwLock<approval::ApprovalQueue>>,
    /// Whether Tor SOCKS5 proxy is enabled for plugin RPCs
    pub tor_enabled: Arc<AtomicBool>,
    /// Authentication state machine for hardware gating
    pub auth_manager: Arc<auth_core::AuthManager>,
    /// Optional monero-wallet-rpc URL for real balance queries
    xmr_wallet_rpc_url: Option<String>,
    /// Source of the per-device key used to seal the persisted seed.
    /// Defaults to `FileDeviceKeyProvider` (desktop); Android injects a
    /// hardware KeyStore-backed provider via `with_key_provider`.
    key_provider: Box<dyn keystore_core::DeviceKeyProvider>,
}

impl Vault {
    /// Create a new, uninitialized vault instance.
    pub fn new() -> Self {
        Self {
            plugin_host: Arc::new(RwLock::new(host::PluginHost::new())),
            ipc_handle: None,
            initialized: Arc::new(AtomicBool::new(false)),
            seed: Arc::new(RwLock::new(None)),
            mnemonic: Arc::new(RwLock::new(None)),
            encrypted_seed: Arc::new(RwLock::new(None)),
            approval_queue: Arc::new(RwLock::new(approval::ApprovalQueue::new())),
            tor_enabled: Arc::new(AtomicBool::new(false)),
            auth_manager: Arc::new(auth_core::AuthManager::new()),
            xmr_wallet_rpc_url: None,
            key_provider: Box::new(keystore_core::FileDeviceKeyProvider::default_home()),
        }
    }

    /// Builder: inject an alternate device-key provider (used by Android to
    /// source the seed-wrapping key from the hardware KeyStore).
    pub fn with_key_provider(
        mut self,
        provider: Box<dyn keystore_core::DeviceKeyProvider>,
    ) -> Self {
        self.key_provider = provider;
        tracing::info!("Vault using device-key backend: {}", self.backend_name());
        self
    }

    /// Name of the active device-key backend (diagnostics).
    pub fn backend_name(&self) -> &'static str {
        self.key_provider.backend_name()
    }

    /// Initialize the vault with a seed phrase (or generate a new BIP-39 wallet if empty).
    ///
    /// Flow: BIP-39 mnemonic generation → PBKDF2 seed → crypto-core key derivation
    /// → keystore-core persistence → wallet-plugin registration → returns the master `KeyHandle`.
    pub async fn initialize(
        &mut self,
        seed_phrase: &str,
        passphrase: &str,
    ) -> Result<crypto_core::KeyHandle, VaultError> {
        if self.initialized.load(Ordering::SeqCst) {
            return Err(VaultError::AlreadyInitialized);
        }

        // 1. BIP-39: Generate or parse mnemonic, derive 512-bit seed with passphrase
        let (mnemonic_str, seed_512): (String, [u8; 64]) = if seed_phrase.is_empty() {
            // Generate a new BIP-39 mnemonic (24 words)
            let phrase = crypto_core::keys::generate_mnemonic(
                crypto_core::MnemonicStrength::TwentyFourWords,
            )
            .map_err(|e| VaultError::CryptoError(e.to_string()))?;
            let phrase_str = phrase.to_string();
            let seed = crypto_core::keys::mnemonic_to_seed(phrase.as_words(), passphrase)
                .map_err(|e| VaultError::CryptoError(e.to_string()))?;
            (phrase_str, *seed)
        } else {
            // Parse an existing BIP-39 mnemonic
            let phrase = crypto_core::keys::mnemonic_from_string(seed_phrase)
                .map_err(|e| VaultError::CryptoError(e.to_string()))?;
            let phrase_str = phrase.to_string();
            let seed = crypto_core::keys::mnemonic_to_seed(phrase.as_words(), passphrase)
                .map_err(|e| VaultError::CryptoError(e.to_string()))?;
            (phrase_str, *seed)
        };

        // 2. Derive the master secp256k1 key (using BIP-44 path)
        let master_key = crypto_core::keys::derive_bip44_eth_key(&seed_512, 0)
            .map_err(|e| VaultError::CryptoError(e.to_string()))?;

        // 3. Encrypt seed and persist via keystore-core — stores {seed, mnemonic}
        let keystore_payload = serde_json::json!({
            "seed": hex::encode(seed_512),
            "mnemonic": mnemonic_str,
        });
        let keystore_bytes = serde_json::to_vec(&keystore_payload)
            .map_err(|e| VaultError::KeystoreError(e.to_string()))?;
        let vault_key = self
            .key_provider
            .get_or_create_key()
            .map_err(|e| VaultError::KeystoreError(e.to_string()))?;
        let encrypted =
            keystore_core::vault::encrypt_with_password(&vault_key, &keystore_bytes, b"vault-seed")
                .map_err(|e| VaultError::KeystoreError(e.to_string()))?;

        *self.seed.write().await = Some(Zeroizing::new(seed_512.to_vec()));
        *self.mnemonic.write().await = Some(mnemonic_str);
        *self.encrypted_seed.write().await = Some(encrypted.clone());

        // 3b. Persist encrypted seed to disk so it survives restart
        if let Some(home) = dirs_next::home_dir() {
            let data_dir = home.join(".gullbur");
            let _ = std::fs::create_dir_all(&data_dir);
            let _ = std::fs::write(data_dir.join(SEED_FILE), &encrypted);
        }

        // 4. Register default blockchain plugins
        {
            let mut host = self.plugin_host.write().await;
            host.set_xmr_wallet_rpc_url(self.xmr_wallet_rpc_url.clone());
            host.initialize_default_plugins();
        }

        self.initialized.store(true, Ordering::SeqCst);
        tracing::info!("Vault initialized successfully via BIP-39");
        Ok(crypto_core::KeyHandle {
            key_id: "bip44-eth-0".to_string(),
            key_type: crypto_core::KeyType::Secp256k1,
            public_key: master_key.public_key().to_sec1_bytes().to_vec(),
        })
    }

    /// Create a wallet account for the given network at the given derivation index.
    ///
    /// Reads the seed stored after initialization and delegates to the plugin host.
    pub async fn create_account(
        &self,
        network: &str,
        index: u32,
    ) -> Result<wallet_plugin::Account, VaultError> {
        let seed_guard = self.seed.read().await;
        let seed = seed_guard.as_ref().ok_or(VaultError::NotInitialized)?;

        let host = self.plugin_host.read().await;
        host.create_account(seed, index, network)
            .await
            .map_err(VaultError::PluginError)
    }

    /// Launch the vault sub-systems (IPC WebSocket server ± Tor daemon).
    pub async fn launch(&mut self, ipc_port: u16, tor_port: Option<u16>) -> Result<(), VaultError> {
        let mut lm = lifecycle::LifecycleManager::new().with_ipc_port(ipc_port);
        if let Some(tp) = tor_port {
            lm = lm.with_tor_port(tp);
        }
        self.ipc_handle = Some(
            lm.start(
                Arc::clone(&self.plugin_host),
                Arc::clone(&self.seed),
                Arc::clone(&self.mnemonic),
                Arc::clone(&self.initialized),
                Arc::clone(&self.approval_queue),
                Arc::clone(&self.auth_manager),
            )
            .await?,
        );
        tracing::info!("Vault sub-systems launched on port {}", ipc_port);
        Ok(())
    }

    /// Take the IPC server handle for external lifecycle tracking.
    /// After calling this, the vault no longer owns the handle and
    /// `shutdown()` will NOT abort the server — the caller is responsible.
    pub fn take_ipc_handle(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        self.ipc_handle.take()
    }

    /// Shut down the vault, stopping IPC and Tor daemon.
    pub async fn shutdown(&mut self) -> Result<(), VaultError> {
        if let Some(handle) = self.ipc_handle.take() {
            handle.abort();
        }
        // Zeroize seed on shutdown (Zeroizing wraps the Vec, so setting None drops and zeroizes)
        *self.seed.write().await = None;
        {
            let mut e = self.encrypted_seed.write().await;
            if let Some(ref mut enc_bytes) = *e {
                enc_bytes.fill(0);
            }
            *e = None;
        }
        tracing::info!("Vault shut down");
        Ok(())
    }

    /// Load and return the persisted encrypted seed from disk, if any.
    /// Returns `None` if no persisted seed file exists or it can't be read.
    pub fn load_persisted_seed() -> Option<Vec<u8>> {
        let path = dirs_next::home_dir()?.join(".gullbur").join(SEED_FILE);
        if path.exists() {
            std::fs::read(&path).ok()
        } else {
            None
        }
    }

    /// Load the per-device keystore key via the injected `DeviceKeyProvider`
    /// (desktop: file-backed; Android: hardware KeyStore).
    fn device_key(&self) -> Result<[u8; 32], VaultError> {
        self.key_provider
            .get_or_create_key()
            .map_err(|e| VaultError::KeystoreError(e.to_string()))
    }

    /// Attempt to restore vault state from a persisted keystore on disk.
    ///
    /// Called after Vault::new() when the async runtime is available.
    /// This is a best-effort restore:
    /// - If no keystore exists, it's a fresh start — silently ok.
    /// - If the keystore exists but decryption fails, the vault starts
    ///   uninitialized and the user must re-init.
    /// - On success, the seed is restored and the vault is marked initialized.
    ///   Callers can then call `launch()` without re-entering a mnemonic.
    pub async fn try_restore(&mut self) -> Result<(), VaultError> {
        let encrypted = match Self::load_persisted_seed() {
            Some(data) => data,
            None => return Ok(()),
        };

        let vault_key = self.device_key()?;
        let decrypted =
            keystore_core::vault::decrypt_with_password(&vault_key, &encrypted, b"vault-seed")
                .map_err(|e| {
                    tracing::warn!("Failed to decrypt persisted keystore: {e}");
                    VaultError::KeystoreError(e.to_string())
                })?;

        // Extract seed and mnemonic from the JSON payload
        let payload: serde_json::Value = serde_json::from_slice(&decrypted)
            .map_err(|e| VaultError::KeystoreError(format!("Keystore parse failed: {e}")))?;

        let seed_hex = payload["seed"]
            .as_str()
            .ok_or_else(|| VaultError::KeystoreError("Missing seed in keystore".into()))?;
        let seed_bytes = hex::decode(seed_hex)
            .map_err(|e| VaultError::KeystoreError(format!("Invalid seed hex: {e}")))?;

        let mnemonic_str = payload["mnemonic"]
            .as_str()
            .ok_or_else(|| VaultError::KeystoreError("Missing mnemonic in keystore".into()))?
            .to_string();

        *self.seed.write().await = Some(zeroize::Zeroizing::new(seed_bytes));
        *self.mnemonic.write().await = Some(mnemonic_str);

        {
            let mut host = self.plugin_host.write().await;
            host.initialize_default_plugins();
        }

        self.initialized.store(true, Ordering::SeqCst);
        tracing::info!("Vault state restored from persisted keystore");
        Ok(())
    }

    /// Get the BIP-39 mnemonic phrase (for seed re-export in Settings).
    /// Returns None if not initialized.
    pub async fn get_mnemonic(&self) -> Option<String> {
        self.mnemonic.read().await.clone()
    }

    /// Enable or disable Tor SOCKS5 proxy for plugin RPCs.
    pub async fn set_tor_enabled(&self, enabled: bool) -> Result<(), VaultError> {
        self.tor_enabled.store(enabled, Ordering::SeqCst);
        if enabled {
            tracing::info!("Tor proxy enabled for plugin RPCs");
        } else {
            tracing::info!("Tor proxy disabled for plugin RPCs");
        }
        Ok(())
    }

    /// Configure a monero-wallet-rpc URL for real XMR balance queries.
    /// Set before `initialize()` for it to take effect.
    pub fn with_xmr_wallet_rpc(mut self, url: impl Into<String>) -> Self {
        self.xmr_wallet_rpc_url = Some(url.into());
        self
    }

    /// Inject an encrypted seed blob (for restoring from file).
    /// The encrypted seed must be in the format (salt || nonce || ciphertext).
    /// Call this before `try_restore()` to bypass the default keystore path.
    pub fn set_encrypted_seed(&self, encrypted: Vec<u8>) {
        // SAFETY: This is called during vault initialization, before any
        // concurrent async access to encrypted_seed. The tokio RwLock
        // get_mut() is safe in this context because we hold no other
        // references and we're in a synchronous setup path.
        *self
            .encrypted_seed
            .try_write()
            .expect("try_restore not yet running") = Some(encrypted);
    }
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn vault_new_is_uninitialized() {
        let vault = Vault::new();
        assert!(!vault.initialized.load(Ordering::SeqCst));
        assert!(vault.ipc_handle.is_none());
    }

    #[tokio::test]
    async fn vault_initialize_sets_flag() {
        // Generate a valid BIP-39 mnemonic
        let phrase =
            crypto_core::keys::generate_mnemonic(crypto_core::MnemonicStrength::TwelveWords)
                .expect("test invariant");
        let mut vault = Vault::new();
        vault
            .initialize(&phrase.to_string(), "")
            .await
            .expect("test invariant");
        assert!(vault.initialized.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn vault_initialize_returns_bip44_key() {
        let phrase =
            crypto_core::keys::generate_mnemonic(crypto_core::MnemonicStrength::TwelveWords)
                .expect("test invariant");
        let mut vault = Vault::new();
        let key = vault
            .initialize(&phrase.to_string(), "")
            .await
            .expect("test invariant");
        assert_eq!(key.key_type, crypto_core::KeyType::Secp256k1);
        assert!(key.key_id.contains("bip44"));
        assert!(!key.public_key.is_empty());
    }

    #[tokio::test]
    async fn test_initialize_generates_seed() {
        let mut vault = Vault::new();
        vault.initialize("", "").await.expect("test invariant");
        assert!(vault.initialized.load(Ordering::SeqCst));
        assert!(vault.seed.read().await.is_some());
        assert!(vault.encrypted_seed.read().await.is_some());
    }

    #[tokio::test]
    async fn test_create_account_fails_when_not_initialized() {
        let vault = Vault::new();
        let result = vault.create_account("bitcoin", 0).await;
        assert!(result.is_err());
    }

    /// A deterministic DeviceKeyProvider — stands in for an Android KeyStore
    /// backend and proves the injected provider is actually used for sealing.
    #[derive(Debug)]
    struct TestKeyProvider;

    impl keystore_core::DeviceKeyProvider for TestKeyProvider {
        fn backend_name(&self) -> &'static str {
            "test-keystore"
        }
        fn get_or_create_key(&self) -> Result<[u8; 32], keystore_core::KeystoreError> {
            Ok([0x99; 32])
        }
    }

    #[tokio::test]
    async fn vault_uses_injected_key_provider() {
        // Default backend on desktop is file-backed.
        let vault = Vault::new();
        assert_eq!(vault.backend_name(), "file");

        // Inject a KeyStore-style provider — the vault must now report it.
        let vault = Vault::new().with_key_provider(Box::new(TestKeyProvider));
        assert_eq!(vault.backend_name(), "test-keystore");

        // End-to-end: initialize with the injected provider seals a keystore
        // that decrypts back to the same seed via the SAME provider.
        let phrase =
            crypto_core::keys::generate_mnemonic(crypto_core::MnemonicStrength::TwelveWords)
                .expect("test invariant");
        let mut vault = Vault::new().with_key_provider(Box::new(TestKeyProvider));
        vault
            .initialize(&phrase.to_string(), "")
            .await
            .expect("test invariant");

        let encrypted = vault.encrypted_seed.read().await.clone().expect("sealed");
        // Decrypt with the injected provider's fixed key — must round-trip.
        let decrypted = keystore_core::vault::decrypt_with_password(
            &[0x99; 32],
            &encrypted,
            b"vault-seed",
        )
        .expect("provider key must decrypt the sealed seed");
        let payload: serde_json::Value =
            serde_json::from_slice(&decrypted).expect("payload json");
        assert!(
            payload["mnemonic"].as_str().map(|s| s.len() > 0).unwrap_or(false),
            "sealed seed restores the mnemonic"
        );
    }
}
