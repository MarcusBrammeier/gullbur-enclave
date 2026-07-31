use thiserror::Error;

/// Errors emitted by keystore-core operations.
#[derive(Debug, Error)]
pub enum KeystoreError {
    #[error("cryptographic error: {0}")]
    Crypto(String),

    #[error("vault is locked")]
    Locked,

    #[error("vault is already locked")]
    AlreadyLocked,

    #[error("keychain error: {0}")]
    Keychain(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid password")]
    InvalidPassword,

    #[error("invalid ciphertext")]
    InvalidCiphertext,
}
