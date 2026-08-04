use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// A 32-byte seed that zeroizes on drop.
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct Seed(pub [u8; 32]);

/// A 64-byte BIP-39 seed that zeroizes on drop.
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct Seed64(pub [u8; 64]);

impl AsRef<[u8]> for Seed64 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// BIP-39 entropy strength (determines mnemonic length).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MnemonicStrength {
    /// 12 words (128 bits of entropy)
    TwelveWords,
    /// 24 words (256 bits of entropy)
    TwentyFourWords,
}

impl MnemonicStrength {
    /// Return the entropy size in bytes.
    pub fn entropy_bytes(&self) -> usize {
        match self {
            Self::TwelveWords => 16,
            Self::TwentyFourWords => 32,
        }
    }
}

/// A BIP-39 mnemonic phrase that zeroizes on drop.
#[derive(Clone, Debug, Zeroize)]
#[zeroize(drop)]
pub struct MnemonicPhrase(pub Vec<String>);

impl MnemonicPhrase {
    /// Construct a `MnemonicPhrase` from a word list.
    pub fn new(words: Vec<String>) -> Self {
        Self(words)
    }

    /// Borrow the words of the phrase.
    pub fn as_words(&self) -> &[String] {
        &self.0
    }

    /// Consume the phrase and return ownership of the underlying word list.
    pub fn into_words(self) -> Vec<String> {
        self.0.clone()
    }
}

impl std::fmt::Display for MnemonicPhrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join(" "))
    }
}

impl AsRef<[u8]> for Seed {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Key types supported across all plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyType {
    /// secp256k1 elliptic-curve keys (Bitcoin, Litecoin, EVM signatures).
    Secp256k1,
    /// ed25519 keys (Monero compatibility).
    Ed25519,
}

/// Generic key handle referencing a key in the keystore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyHandle {
    /// Opaque identifier resolving to a stored key (e.g. `secp256k1-<fingerprint>`).
    pub key_id: String,
    /// The family/cipher of the referenced key.
    pub key_type: KeyType,
    /// SEC1-encoded public key bytes (empty for keys kept locally secret).
    pub public_key: Vec<u8>,
}
