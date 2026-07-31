use thiserror::Error;

/// Errors from the Tor daemon manager.
#[derive(Debug, Error)]
pub enum TorError {
    #[error("arti daemon not found: {0}")]
    DaemonNotFound(String),

    #[error("daemon crashed: {0}")]
    DaemonCrashed(String),

    #[error("daemon is already running")]
    AlreadyRunning,

    #[error("failed to spawn arti: {0}")]
    SpawnFailed(String),

    #[error("readiness check timed out after {0}s")]
    ReadinessTimeout(u64),

    #[error("max retries ({0}) exceeded")]
    MaxRetriesExceeded(u32),

    #[error("circuit isolation failed: {0}")]
    CircuitIsolationFailed(String),
}
