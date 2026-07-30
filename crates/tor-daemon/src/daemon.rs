use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout, Duration};
use tracing::{info, warn};

use crate::error::TorError;

/// Configuration for the Tor daemon.
#[derive(Debug, Clone)]
pub struct TorConfig {
    pub socks_port: u16,
    pub control_port: u16,
    pub readiness_timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for TorConfig {
    fn default() -> Self {
        Self {
            socks_port: 9050,
            control_port: 9051,
            readiness_timeout_secs: 30,
            connect_timeout_secs: 30,
            max_retries: 5,
        }
    }
}

/// Manages an out-of-process arti Tor daemon.
///
/// Spawns `arti` as a child process and provides lifecycle management
/// (start, health check, shutdown), retry with exponential backoff,
/// SOCKS port readiness polling, and circuit isolation.
pub struct TorDaemon {
    config: TorConfig,
    child: Option<Child>,
    /// Current retry count for backoff tracking
    retry_count: u32,
    /// Total circuit counter for naming
    circuit_counter: u32,
}

impl TorDaemon {
    /// Create a new Tor daemon from a TorConfig.
    pub fn new(config: TorConfig) -> Self {
        Self {
            config,
            child: None,
            retry_count: 0,
            circuit_counter: 0,
        }
    }

    /// Convenience: create with just a SOCKS port (backward-compat).
    pub fn with_port(socks_port: u16) -> Self {
        Self::new(TorConfig {
            socks_port,
            ..Default::default()
        })
    }

    /// Returns the SOCKS proxy address.
    pub fn socks_proxy(&self) -> String {
        format!("socks5://127.0.0.1:{}", self.config.socks_port)
    }

    /// Returns the raw SOCKS address (host:port).
    pub fn socks_addr(&self) -> String {
        format!("127.0.0.1:{}", self.config.socks_port)
    }

    /// Returns the underlying config.
    pub fn config(&self) -> &TorConfig {
        &self.config
    }

    // ── Lifecycle ──────────────────────────────────────────────────────────

    /// Start the arti daemon with retry and backoff.
    ///
    /// Spawns `arti proxy --socks-port {port}` and polls the SOCKS port
    /// for readiness. Retries with exponential backoff on failure.
    pub async fn start(&mut self) -> Result<(), TorError> {
        if self.child.is_some() {
            return Err(TorError::AlreadyRunning);
        }

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match self.try_start_once().await {
                Ok(()) => {
                    self.retry_count = 0;
                    return Ok(());
                }
                Err(e) => {
                    if attempt >= self.config.max_retries {
                        self.retry_count = 0;
                        return Err(TorError::MaxRetriesExceeded(self.config.max_retries));
                    }
                    let backoff = Duration::from_secs(
                        (1u64 << attempt.saturating_sub(1)).min(32)
                    );
                    warn!(
                        "Tor daemon start failed (attempt {attempt}/{}): {e}. Retrying in {backoff:?}",
                        self.config.max_retries
                    );
                    sleep(backoff).await;
                    self.retry_count = attempt;
                }
            }
        }
    }

    /// Single start attempt: spawn arti and wait for SOCKS readiness.
    async fn try_start_once(&mut self) -> Result<(), TorError> {
        let socks_arg = format!("--socks-port={}", self.config.socks_port);

        let child = Command::new("arti")
            .arg("proxy")
            .arg(&socks_arg)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    TorError::DaemonNotFound("arti binary not found in PATH".into())
                } else {
                    TorError::SpawnFailed(e.to_string())
                }
            })?;

        let pid = child.id().unwrap_or(0);
        self.child = Some(child);
        info!("Tor daemon spawned (pid={pid}), waiting for SOCKS readiness on port {}...", self.config.socks_port);

        // Poll SOCKS port for readiness
        self.wait_for_readiness().await?;

        info!("Tor daemon ready — SOCKS proxy available on 127.0.0.1:{}", self.config.socks_port);
        Ok(())
    }

    /// Poll the SOCKS port every 500ms until it accepts connections or timeout.
    async fn wait_for_readiness(&self) -> Result<(), TorError> {
        let addr = format!("127.0.0.1:{}", self.config.socks_port);
        let deadline = Duration::from_secs(self.config.readiness_timeout_secs);
        let start = tokio::time::Instant::now();

        loop {
            if start.elapsed() >= deadline {
                return Err(TorError::ReadinessTimeout(self.config.readiness_timeout_secs));
            }

            match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
                Ok(Ok(_)) => return Ok(()), // port is open
                Ok(Err(_)) => {}             // connection refused, retry
                Err(_) => {}                 // timeout, retry
            }

            sleep(Duration::from_millis(500)).await;
        }
    }

    /// Check whether the arti process is still running.
    pub fn is_running(&mut self) -> bool {
        match &mut self.child {
            Some(child) => {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        warn!("Tor daemon exited with status: {status}");
                        false
                    }
                    Ok(None) => true,
                    Err(e) => {
                        warn!("Failed to check Tor daemon status: {e}");
                        false
                    }
                }
            }
            None => false,
        }
    }

    /// Restart the daemon if it has crashed, with backoff.
    pub async fn restart_if_crashed(&mut self) -> Result<(), TorError> {
        if self.is_running() {
            return Ok(());
        }

        warn!("Tor daemon crashed — restarting (attempt {})...", self.retry_count + 1);
        self.child = None;
        Box::pin(self.start()).await
    }

    // ── Circuit Isolation ──────────────────────────────────────────────────

    /// Request a fresh circuit via arti control port (NEWNYM signal).
    ///
    /// Sends "AUTHENTICATE\r\nSIGNAL NEWNYM\r\n" to the control port.
    /// Returns a human-readable circuit identifier.
    pub async fn new_circuit(&mut self) -> Result<String, TorError> {
        let addr = format!("127.0.0.1:{}", self.config.control_port);

        let mut stream = timeout(
            Duration::from_secs(self.config.connect_timeout_secs),
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| {
            TorError::CircuitIsolationFailed("control port connection timed out".into())
        })?
        .map_err(|e| {
            TorError::CircuitIsolationFailed(format!("control port unreachable: {e}"))
        })?;

        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (reader, mut writer) = stream.split();
        let mut buf = BufReader::new(reader);

        // Read the banner line
        let mut line = String::new();
        buf.read_line(&mut line).await.map_err(|e| {
            TorError::CircuitIsolationFailed(format!("failed to read control banner: {e}"))
        })?;

        // Authenticate
        writer.write_all(b"AUTHENTICATE\r\n").await.map_err(|e| {
            TorError::CircuitIsolationFailed(format!("failed to send AUTHENTICATE: {e}"))
        })?;
        let mut resp = String::new();
        buf.read_line(&mut resp).await.map_err(|e| {
            TorError::CircuitIsolationFailed(format!("AUTHENTICATE read failed: {e}"))
        })?;
        if !resp.starts_with("250") {
            return Err(TorError::CircuitIsolationFailed(format!(
                "AUTHENTICATE rejected: {}",
                resp.trim()
            )));
        }

        // Signal NEWNYM
        writer.write_all(b"SIGNAL NEWNYM\r\n").await.map_err(|e| {
            TorError::CircuitIsolationFailed(format!("failed to send NEWNYM: {e}"))
        })?;
        resp.clear();
        buf.read_line(&mut resp).await.map_err(|e| {
            TorError::CircuitIsolationFailed(format!("NEWNYM read failed: {e}"))
        })?;
        if !resp.starts_with("250") {
            return Err(TorError::CircuitIsolationFailed(format!(
                "NEWNYM rejected: {}",
                resp.trim()
            )));
        }

        // QUIT
        let _ = writer.write_all(b"QUIT\r\n").await;

        self.circuit_counter += 1;
        let id = format!("tor-circuit-{}", self.circuit_counter);
        info!("New circuit established: {id}");
        Ok(id)
    }

    // ── Shutdown ───────────────────────────────────────────────────────────

    /// Shut down the arti daemon gracefully (SIGTERM), then force-kill after 5s.
    pub async fn shutdown(&mut self) -> Result<(), TorError> {
        let Some(mut child) = self.child.take() else {
            return Err(TorError::DaemonCrashed("no child process".into()));
        };

        // Send SIGTERM.
        if let Err(e) = child.start_kill() {
            warn!("Failed to send SIGTERM to arti: {e}");
        }

        // Wait up to 5 seconds for graceful shutdown.
        let deadline = Duration::from_secs(5);
        match timeout(deadline, child.wait()).await {
            Ok(Ok(status)) => {
                info!("Tor daemon exited gracefully: {status}");
            }
            Ok(Err(e)) => {
                warn!("Error waiting for Tor daemon: {e}");
                let _ = child.kill().await;
            }
            Err(_) => {
                warn!("Tor daemon did not shut down in time; sending SIGKILL");
                let _ = child.kill().await;
            }
        }

        self.child = None;
        self.retry_count = 0;
        Ok(())
    }
}

impl Drop for TorDaemon {
    fn drop(&mut self) {
        if self.child.is_some()
            && let Some(mut child) = self.child.take() {
                let _ = child.start_kill();
            }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let cfg = TorConfig::default();
        assert_eq!(cfg.socks_port, 9050);
        assert_eq!(cfg.control_port, 9051);
        assert_eq!(cfg.readiness_timeout_secs, 30);
        assert_eq!(cfg.connect_timeout_secs, 30);
        assert_eq!(cfg.max_retries, 5);
    }

    #[test]
    fn test_daemon_new_has_correct_port() {
        let daemon = TorDaemon::with_port(9050);
        assert_eq!(daemon.config().socks_port, 9050);
    }

    #[test]
    fn test_daemon_socks_proxy_returns_correct_addr() {
        let daemon = TorDaemon::with_port(9150);
        assert_eq!(daemon.socks_proxy(), "socks5://127.0.0.1:9150");
        assert_eq!(daemon.socks_addr(), "127.0.0.1:9150");
    }

    #[test]
    fn test_daemon_not_running_initially() {
        let mut daemon = TorDaemon::with_port(9050);
        assert!(!daemon.is_running());
    }

    #[tokio::test]
    async fn test_daemon_start_fails_when_arti_missing() {
        let mut daemon = TorDaemon::with_port(9999);
        let result = daemon.start().await;
        assert!(result.is_err());
        match result {
            Err(TorError::DaemonNotFound(_)) | Err(TorError::MaxRetriesExceeded(_)) => {}
            other => panic!("Expected DaemonNotFound/MaxRetriesExceeded, got: {other:?}"),
        }
    }

    #[test]
    fn test_readiness_timeout_error() {
        // With 0-second timeout, any poll should time out immediately
        let mut daemon = TorDaemon::new(TorConfig {
            socks_port: 19999,
            readiness_timeout_secs: 0,
            max_retries: 1,
            ..Default::default()
        });
        let rt = tokio::runtime::Runtime::new().expect("test invariant");
        let result = rt.block_on(daemon.start());
        assert!(result.is_err());
        match result {
            Err(TorError::ReadinessTimeout(0))
            | Err(TorError::DaemonNotFound(_))
            | Err(TorError::MaxRetriesExceeded(_)) => {}
            other => panic!("Expected timeout/notfound/max-retries, got: {other:?}"),
        }
    }

    #[test]
    fn test_with_port_uses_custom_socks_port() {
        let daemon = TorDaemon::with_port(9150);
        assert_eq!(daemon.config().socks_port, 9150);
        assert_eq!(daemon.config().control_port, 9051); // unchanged default
    }
}