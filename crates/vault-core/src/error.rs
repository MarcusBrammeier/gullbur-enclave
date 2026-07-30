//! Vault error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("Vault is not initialized")]
    NotInitialized,

    #[error("Vault is already initialized")]
    AlreadyInitialized,

    #[error("IPC server error: {0}")]
    IpcError(String),

    #[error("Tor daemon error: {0}")]
    TorError(String),

    #[error("Plugin error: {0}")]
    PluginError(#[from] wallet_plugin::PluginError),

    #[error("Keystore error: {0}")]
    KeystoreError(String),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}