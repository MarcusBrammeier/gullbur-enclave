use thiserror::Error;

/// Errors from the IPC layer.
#[derive(Debug, Error)]
pub enum IpcError {
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("connection error: {0}")]
    ConnectionError(String),

    #[error("timeout waiting for client hello")]
    Timeout,

    #[error("bind error: {0}")]
    BindError(String),

    #[error("internal error: {0}")]
    Internal(String),
}