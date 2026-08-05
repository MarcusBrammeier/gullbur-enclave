//! Vault lifecycle management — IPC server, Tor daemon, shutdown coordination.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;

use crate::error::VaultError;
use crate::xmr_downloader;
use crate::xmr_wallet_rpc::MoneroWalletRpcProcess;
use tor_daemon::daemon::TorDaemon;

/// Manages the lifecycle of all vault sub-systems:
/// - IPC WebSocket server (extension bridge)
/// - Tor daemon (privacy proxy)
/// - Plugin host
pub struct LifecycleManager {
    ipc_port: Option<u16>,
    no_encrypt: bool,
    tor_socks_port: Option<u16>,
    /// Handle to the running IPC server task
    ipc_handle: Option<tokio::task::JoinHandle<()>>,
    /// Path to the IPC auth token file (for cleanup on shutdown)
    token_path: Option<std::path::PathBuf>,
    /// Managed Tor daemon instance
    tor_daemon: Option<TorDaemon>,
    /// Path to monero-wallet-rpc binary (optional — set to enable XMR balance)
    xmr_binary_path: Option<String>,
    /// Managed monero-wallet-rpc process
    xmr_wallet_rpc: Option<MoneroWalletRpcProcess>,
    /// Wallet directory for XMR
    xmr_wallet_dir: Option<String>,
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self {
            ipc_port: None,
            no_encrypt: false,
            tor_socks_port: None,
            ipc_handle: None,
            token_path: None,
            tor_daemon: None,
            xmr_binary_path: None,
            xmr_wallet_rpc: None,
            xmr_wallet_dir: None,
        }
    }

    /// Configure the path to the `monero-wallet-rpc` binary.
    /// Enables automatic XMR balance queries via a managed subprocess.
    pub fn with_xmr_wallet_rpc(mut self, binary_path: impl Into<String>) -> Self {
        self.xmr_binary_path = Some(binary_path.into());
        self
    }

    pub fn with_no_encrypt(mut self) -> Self {
        self.no_encrypt = true;
        self
    }

    /// Configure the IPC WebSocket server port.
    pub fn with_ipc_port(mut self, port: u16) -> Self {
        self.ipc_port = Some(port);
        self
    }

    /// Configure the Tor SOCKS proxy port.
    pub fn with_tor_port(mut self, port: u16) -> Self {
        self.tor_socks_port = Some(port);
        self
    }

    /// Start all configured sub-systems.
    ///
    /// Registers vault IPC handlers and returns the IPC server's `JoinHandle`
    /// so the caller can manage the task lifecycle (abort on shutdown).
    pub async fn start(
        &mut self,
        plugin_host: Arc<RwLock<crate::host::PluginHost>>,
        seed: Arc<RwLock<Option<zeroize::Zeroizing<Vec<u8>>>>>,
        mnemonic: Arc<RwLock<Option<String>>>,
        initialized: Arc<AtomicBool>,
        approval_queue: Arc<RwLock<crate::approval::ApprovalQueue>>,
        auth_manager: Arc<auth_core::AuthManager>,
    ) -> Result<tokio::task::JoinHandle<()>, VaultError> {
        if let Some(port) = self.ipc_port {
            let server = ipc_core::server::IpcServer::with_encryption(port, !self.no_encrypt)
                .map_err(|e| VaultError::Internal(format!("IPC server creation failed: {e}")))?;
            tracing::info!("IPC server starting on 127.0.0.1:{}", port);

            // Register vault IPC handlers before starting the server
            {
                let mut handler = server.handler().await;
                crate::ipc_handlers::register_vault_handlers(
                    &mut handler,
                    plugin_host,
                    seed,
                    mnemonic,
                    initialized,
                    approval_queue,
                    auth_manager,
                );
            } // handler write guard dropped here

            // Capture the token path for cleanup on shutdown
            self.token_path = Some(server.auth_token_path().to_path_buf());

            // Spawn the server and wait for it to actually bind the port.
            // The oneshot channel fires once bind() succeeds (or returns
            // the error). This prevents callers from racing a background
            // task that hasn't called TcpListener::bind() yet.
            let (handle, ready) = server.run();
            match ready.await {
                Ok(Ok(())) => {
                    self.ipc_handle = Some(handle);
                }
                Ok(Err(e)) => {
                    tracing::error!("IPC server failed to bind: {e}");
                    return Err(VaultError::Internal(format!(
                        "IPC server bind failed: {e}"
                    )));
                }
                Err(_) => {
                    tracing::error!("IPC server readiness channel closed without signal");
                    return Err(VaultError::Internal(
                        "IPC server readiness channel closed".into(),
                    ));
                }
            }
        }

        if let Some(port) = self.tor_socks_port {
            tracing::info!("Tor daemon starting with SOCKS on 127.0.0.1:{}", port);
            let mut daemon = TorDaemon::with_port(port);
            daemon
                .start()
                .await
                .map_err(|e| VaultError::Internal(format!("Tor daemon start failed: {e}")))?;
            self.tor_daemon = Some(daemon);
        }

        // Start monero-wallet-rpc subprocess if binary path is configured
        if self.xmr_binary_path.is_some() {
            // Determine cache directory (app data dir or ~/.gullbur/xmr-bin)
            let cache_dir = self.xmr_wallet_dir.clone().unwrap_or_else(|| {
                dirs_next::home_dir()
                    .map(|h| {
                        h.join(".gullbur")
                            .join("xmr-bin")
                            .to_string_lossy()
                            .to_string()
                    })
                    .unwrap_or_else(|| "/tmp/foss-xmr-bin".into())
            });
            let cache_path = std::path::PathBuf::from(&cache_dir);
            tracing::info!("Ensuring monero-wallet-rpc binary in {:?}", cache_path);
            let ensured = xmr_downloader::ensure_binary(&cache_path)
                .await
                .map_err(|e| VaultError::Internal(format!("XMR binary download failed: {e}")))?;

            let network = "monero-stagenet";
            let wallet_dir = format!("{}/wallets", cache_dir);
            tracing::info!(
                "Starting monero-wallet-rpc from {:?} (network: {}, dir: {})",
                ensured.path,
                network,
                wallet_dir
            );
            let mut xmr_proc = MoneroWalletRpcProcess::new(
                ensured.path.to_string_lossy().to_string(),
                network,
                wallet_dir,
            );
            xmr_proc.start().await.map_err(|e| {
                VaultError::Internal(format!("monero-wallet-rpc start failed: {e}"))
            })?;
            tracing::info!(
                "monero-wallet-rpc ready at {}",
                xmr_proc.url().unwrap_or_default()
            );
            self.xmr_wallet_rpc = Some(xmr_proc);
        }

        // Return the IPC handle so the vault can manage abort
        self.ipc_handle
            .take()
            .ok_or_else(|| VaultError::Internal("No IPC server was configured".into()))
    }

    /// Gracefully shut down all sub-systems.
    pub async fn shutdown(&mut self) -> Result<(), VaultError> {
        tracing::info!("Lifecycle manager shutting down all sub-systems");

        // Abort the IPC server task
        if let Some(handle) = self.ipc_handle.take() {
            handle.abort();
            tracing::info!("IPC server task aborted");
        }

        // Clean up the auth token file
        if let Some(path) = self.token_path.take()
            && let Err(e) = std::fs::remove_file(&path)
        {
            tracing::warn!("Failed to remove IPC token file {:?}: {e}", path);
        }

        // Shut down the Tor daemon if running
        if let Some(mut daemon) = self.tor_daemon.take() {
            tracing::info!("Shutting down Tor daemon");
            if let Err(e) = daemon.shutdown().await {
                tracing::warn!("Tor daemon shutdown error: {e}");
            }
        }

        // Shut down monero-wallet-rpc if running
        if let Some(mut xmr) = self.xmr_wallet_rpc.take() {
            tracing::info!("Shutting down monero-wallet-rpc");
            if let Err(e) = xmr.shutdown().await {
                tracing::warn!("monero-wallet-rpc shutdown error: {e}");
            }
        }
        Ok(())
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn lifecycle_new_is_idle() {
        let lm = LifecycleManager::new();
        assert!(lm.ipc_port.is_none());
        assert!(lm.ipc_handle.is_none());
        assert!(lm.tor_daemon.is_none());
    }

    #[tokio::test]
    async fn lifecycle_with_ipc_port_sets_config() {
        let lm = LifecycleManager::new().with_ipc_port(19876);
        assert_eq!(lm.ipc_port, Some(19876));
    }

    #[tokio::test]
    async fn lifecycle_with_tor_port_sets_config() {
        let lm = LifecycleManager::new().with_tor_port(19050);
        assert_eq!(lm.tor_socks_port, Some(19050));
    }

    #[tokio::test]
    async fn lifecycle_start_fails_without_ipc_port() {
        let mut lm = LifecycleManager::new();
        let plugin_host = Arc::new(RwLock::new(crate::host::PluginHost::new()));
        let seed = Arc::new(RwLock::new(None));
        let initialized = Arc::new(AtomicBool::new(false));
        let approval_queue = Arc::new(RwLock::new(crate::approval::ApprovalQueue::new()));
        let auth_manager = Arc::new(auth_core::AuthManager::new());

        let mnemonic = Arc::new(RwLock::new(None));
        let result = lm
            .start(
                plugin_host,
                seed,
                mnemonic,
                initialized,
                approval_queue,
                auth_manager,
            )
            .await;
        assert!(result.is_err(), "start without IPC port should fail");
        assert!(result.unwrap_err().to_string().contains("No IPC server"));
    }

    #[tokio::test]
    async fn lifecycle_shutdown_idempotent() {
        let mut lm = LifecycleManager::new();
        // Shutdown on a never-started manager should not panic
        let result = lm.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn lifecycle_with_no_encrypt_flag() {
        let lm = LifecycleManager::new().with_no_encrypt();
        assert!(lm.no_encrypt);
    }

    #[tokio::test]
    async fn lifecycle_double_shutdown_ok() {
        let mut lm = LifecycleManager::new();
        assert!(lm.shutdown().await.is_ok());
        assert!(lm.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn lifecycle_ipc_port_stored_after_config() {
        let lm = LifecycleManager::new().with_ipc_port(19877);
        assert_eq!(lm.ipc_port, Some(19877));
    }

    #[tokio::test]
    async fn lifecycle_start_configures_ipc_port_only() {
        let lm = LifecycleManager::new().with_ipc_port(19878);
        assert!(lm.ipc_port.is_some());
        assert!(lm.tor_socks_port.is_none());
    }
}
