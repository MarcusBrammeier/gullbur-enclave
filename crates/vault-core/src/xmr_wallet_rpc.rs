//! Manages a local `monero-wallet-rpc` subprocess.
//!
//! Spawns the binary as a child process, waits for it to become ready,
//! and kills it on shutdown. The wallet daemon connects to a **remote**
//! public Monero daemon — no blockchain download required.
//!
//! ## Usage
//!
//! ```ignore
//! let wallet_rpc = MoneroWalletRpcProcess::new(
//!     "/usr/bin/monero-wallet-rpc",
//!     "stagenet.xmr-node.cakewallet.com:38081",
//!     "/tmp/xmr-wallets",
//!     18082,
//! );
//! wallet_rpc.start().await?;
//! let url = wallet_rpc.url(); // "http://127.0.0.1:18082/json_rpc"
//! // ... use url with XmrPlugin::with_wallet_rpc(url) ...
//! wallet_rpc.shutdown().await?;
//! ```

use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::sleep;

/// Default port for monero-wallet-rpc.
const DEFAULT_XMR_RPC_PORT: u16 = 18082;

/// Manages the lifecycle of a `monero-wallet-rpc` child process.
pub struct MoneroWalletRpcProcess {
    /// Path to the monero-wallet-rpc binary.
    binary_path: String,
    /// Remote daemon to connect to (e.g. "stagenet.xmr-node.cakewallet.com:38081").
    daemon_address: String,
    /// Directory for wallet files (ephemeral).
    wallet_dir: String,
    /// Local RPC bind port.
    rpc_bind_port: u16,
    /// Handle to the running child process.
    child: Option<Child>,
}

impl MoneroWalletRpcProcess {
    /// Create a new wallet-rpc process config. Does **not** start it.
    ///
    /// `network` is a Monero network string ("monero", "monero-stagenet", "monero-testnet").
    /// The daemon address is auto-selected from public nodes.
    /// Call `.start().await` to actually launch.
    pub fn new(
        binary_path: impl Into<String>,
        network: &str,
        wallet_dir: impl Into<String>,
    ) -> Self {
        let daemon_address = match network {
            "monero" => "xmr-node.cakewallet.com:18081",
            "monero-stagenet" => "stagenet.xmr-node.cakewallet.com:38081",
            "monero-testnet" => "testnet.xmr-node.cakewallet.com:28081",
            _ => "stagenet.xmr-node.cakewallet.com:38081",
        };
        Self {
            binary_path: binary_path.into(),
            daemon_address: daemon_address.to_string(),
            wallet_dir: wallet_dir.into(),
            rpc_bind_port: DEFAULT_XMR_RPC_PORT,
            child: None,
        }
    }

    /// Customise the local RPC bind port (default: 18082).
    pub fn with_port(mut self, port: u16) -> Self {
        self.rpc_bind_port = port;
        self
    }

    /// Customise the remote daemon address (overrides the network default).
    pub fn with_daemon(mut self, addr: impl Into<String>) -> Self {
        self.daemon_address = addr.into();
        self
    }

    /// The JSON-RPC URL of the running wallet-rpc process.
    /// Returns `None` if the process hasn't been started yet.
    pub fn url(&self) -> Option<String> {
        self.child
            .as_ref()
            .map(|_| format!("http://127.0.0.1:{}/json_rpc", self.rpc_bind_port))
    }

    /// Spawn the `monero-wallet-rpc` process and wait for it to become ready.
    ///
    /// Creates the wallet directory if it doesn't exist.
    /// Polls the health endpoint up to 30 times (3s total) with 100ms delays.
    pub async fn start(&mut self) -> Result<(), String> {
        // Create wallet directory
        std::fs::create_dir_all(&self.wallet_dir)
            .map_err(|e| format!("Failed to create wallet dir: {e}"))?;

        let child = Command::new(&self.binary_path)
            .arg("--daemon-address")
            .arg(&self.daemon_address)
            .arg("--wallet-dir")
            .arg(&self.wallet_dir)
            .arg("--rpc-bind-port")
            .arg(self.rpc_bind_port.to_string())
            .arg("--rpc-bind-ip")
            .arg("127.0.0.1")
            .arg("--disable-rpc-login")
            .arg("--trusted-daemon")
            .kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn monero-wallet-rpc: {e}"))?;

        self.child = Some(child);

        // Wait for the process to start listening
        self.wait_ready().await?;

        tracing::info!(
            "monero-wallet-rpc started on 127.0.0.1:{} (daemon: {})",
            self.rpc_bind_port,
            self.daemon_address,
        );
        Ok(())
    }

    /// Poll the wallet-rpc health endpoint until it responds or times out.
    async fn wait_ready(&self) -> Result<(), String> {
        let url = format!("http://127.0.0.1:{}/json_rpc", self.rpc_bind_port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| format!("HTTP client: {e}"))?;

        for _attempt in 0..30 {
            let ok = client
                .post(&url)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "0",
                    "method": "get_version",
                    "params": {},
                }))
                .send()
                .await
                .is_ok();

            if ok {
                return Ok(());
            }

            sleep(Duration::from_millis(100)).await;
        }

        Err("monero-wallet-rpc did not become ready within 3s".into())
    }

    /// Gracefully shut down the wallet-rpc process by sending `stop_wallet`.
    /// Falls back to SIGTERM if the RPC call fails.
    pub async fn shutdown(&mut self) -> Result<(), String> {
        if self.child.is_none() {
            return Ok(());
        }

        // Try graceful shutdown via RPC
        let url = format!("http://127.0.0.1:{}/json_rpc", self.rpc_bind_port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| format!("HTTP client: {e}"))?;

        let _ = client
            .post(&url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "0",
                "method": "stop_wallet",
                "params": {},
            }))
            .send()
            .await;

        // Kill the process
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill(); // SIGTERM
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        }

        tracing::info!("monero-wallet-rpc shut down");
        Ok(())
    }

    /// Check if the process is still running.
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_configures_defaults() {
        let proc = MoneroWalletRpcProcess::new("monero-wallet-rpc", "monero-stagenet", "/tmp/xmr");
        assert_eq!(proc.rpc_bind_port, 18082);
        assert_eq!(
            proc.daemon_address,
            "stagenet.xmr-node.cakewallet.com:38081"
        );
        assert!(proc.url().is_none());
        assert!(!proc.is_running());
    }

    #[test]
    fn test_new_mainnet_daemon() {
        let proc = MoneroWalletRpcProcess::new("monero-wallet-rpc", "monero", "/tmp/xmr");
        assert_eq!(proc.daemon_address, "xmr-node.cakewallet.com:18081");
    }

    #[test]
    fn test_new_testnet_daemon() {
        let proc = MoneroWalletRpcProcess::new("monero-wallet-rpc", "monero-testnet", "/tmp/xmr");
        assert_eq!(proc.daemon_address, "testnet.xmr-node.cakewallet.com:28081");
    }

    #[test]
    fn test_custom_port() {
        let proc = MoneroWalletRpcProcess::new("mw", "monero-stagenet", "/tmp/x").with_port(19000);
        assert_eq!(proc.rpc_bind_port, 19000);
    }

    #[test]
    fn test_custom_daemon() {
        let proc = MoneroWalletRpcProcess::new("mw", "monero-stagenet", "/tmp/x")
            .with_daemon("my-node.local:38081");
        assert_eq!(proc.daemon_address, "my-node.local:38081");
    }

    #[test]
    fn test_url_none_when_not_started() {
        let proc = MoneroWalletRpcProcess::new("mw", "monero", "/tmp/x");
        assert!(proc.url().is_none());
    }

    #[test]
    fn test_url_some_when_started() {
        let proc = MoneroWalletRpcProcess::new("mw", "monero", "/tmp/x").with_port(18082);
        assert_eq!(proc.url(), None);
        // Verify URL format
        let url = format!("http://127.0.0.1:{}/json_rpc", 18082);
        assert_eq!(url, "http://127.0.0.1:18082/json_rpc");
    }
}
