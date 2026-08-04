use thiserror::Error;

/// Errors produced by the `crypto-core` primitives.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// The supplied key bytes were malformed or not a valid key for the curve.
    #[error("invalid key: {0}")]
    InvalidKey(String),

    /// A cryptographic signing operation failed.
    #[error("signing failed: {0}")]
    SigningFailed(String),

    /// A hash computation failed.
    #[error("hash computation failed: {0}")]
    HashFailed(String),

    /// A BIP-39 mnemonic could not be parsed or generated.
    #[error("mnemonic error: {0}")]
    MnemonicError(String),

    /// A key-derivation step (BIP-32/BIP-44) failed.
    #[error("derivation error: {0}")]
    DerivationError(String),

    /// The caller supplied invalid input.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// An underlying I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
