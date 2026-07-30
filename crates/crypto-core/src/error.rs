use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid key: {0}")]
    InvalidKey(String),

    #[error("signing failed: {0}")]
    SigningFailed(String),

    #[error("hash computation failed: {0}")]
    HashFailed(String),

    #[error("mnemonic error: {0}")]
    MnemonicError(String),

    #[error("derivation error: {0}")]
    DerivationError(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}