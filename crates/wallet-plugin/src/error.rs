use thiserror::Error;

/// Errors that plugin operations can return.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("network error: {0}")]
    NetworkError(String),

    #[error("invalid address: {0}")]
    InvalidAddress(String),

    #[error("signing failed: {0}")]
    SigningFailed(String),

    #[error("broadcast failed: {0}")]
    BroadcastFailed(String),

    #[error("unsupported network: {0}")]
    UnsupportedNetwork(String),

    #[error("internal error: {0}")]
    Internal(String),
}