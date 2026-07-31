//! Encrypted vault — AES-256-GCM envelope encryption with HKDF key derivation.
//!
//! The vault seals arbitrary data under a password. Key material is derived
//! via HKDF from the password and a random 32-byte salt. The stored format:
//! `salt (32) || nonce (12) || ciphertext`.
//!
//! Note: `Nonce::from_slice` is deprecated in aes-gcm 0.11 but the
//! replacement (`TryFrom`) has varying availability across versions.
//! Allowed until a stable cross-version API is available.
#![allow(deprecated)]

use crate::error::KeystoreError;
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::ZeroizeOnDrop;

/// An AES-256 key derived from a password.
#[derive(ZeroizeOnDrop)]
struct DerivedKey([u8; 32]);

impl AsRef<[u8]> for DerivedKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Derive a 256-bit key from a password and salt using HKDF-SHA256.
fn derive_key(password: &[u8], salt: &[u8]) -> DerivedKey {
    let mut key = DerivedKey([0u8; 32]);
    hkdf::Hkdf::<Sha256>::new(Some(salt), password)
        .expand(b"gullbur-vault-key", &mut key.0)
        .expect("HKDF expand: 32 bytes fits SHA-256 output");
    key
}

/// A sealed blob: salt + nonce + ciphertext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedData {
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// An encrypted vault that can lock / unlock and seal / open data.
///
/// Workflow:
/// 1. `seal(password, data, aad)` — encrypt data with a key derived from password.
/// 2. `lock()` — zeroize the derived key so the vault is locked.
/// 3. `unlock(password)` — re-derive the key from the password + stored salt.
/// 4. `open(sealed_bytes, aad)` — decrypt the sealed data with the key.
#[derive(Default)]
pub struct Vault {
    sealed: Option<SealedData>,
    derived_key: Option<DerivedKey>,
    unlocked: bool,
}

impl Vault {
    /// Create a new empty vault (unlocked, no sealed data).
    pub fn new() -> Self {
        Self {
            sealed: None,
            derived_key: None,
            unlocked: true,
        }
    }

    /// Lock the vault — zeroizes the derived key.
    pub fn lock(&mut self, _password: &[u8]) -> Result<(), KeystoreError> {
        if !self.unlocked {
            return Err(KeystoreError::AlreadyLocked);
        }
        self.derived_key = None;
        self.unlocked = false;
        Ok(())
    }

    /// Unlock the vault by re-deriving the key from the password + stored salt.
    pub fn unlock(&mut self, password: &[u8]) -> Result<(), KeystoreError> {
        if self.unlocked {
            return Ok(());
        }

        let sealed = self.sealed.as_ref().ok_or(KeystoreError::Locked)?;

        let key = derive_key(password, &sealed.salt);
        self.derived_key = Some(key);
        self.unlocked = true;
        Ok(())
    }

    /// Seal (encrypt) data with AES-256-GCM.
    ///
    /// Returns serialized bytes: salt || nonce || ciphertext.
    pub fn seal(
        &mut self,
        password: &[u8],
        data: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, KeystoreError> {
        let mut salt = [0u8; 32];
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut salt);
        rand::rng().fill_bytes(&mut nonce_bytes);

        let key = derive_key(password, &salt);
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|e| KeystoreError::Crypto(format!("AES-GCM init: {e}")))?;

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, Payload { msg: data, aad })
            .map_err(|e| KeystoreError::Crypto(format!("encryption failed: {e}")))?;

        let sealed = SealedData {
            salt: salt.to_vec(),
            nonce: nonce_bytes.to_vec(),
            ciphertext: ciphertext.clone(),
        };

        // Store internally
        self.sealed = Some(sealed);
        self.derived_key = Some(key);

        // Serialize: salt || nonce || ciphertext
        let mut output = Vec::with_capacity(32 + 12 + ciphertext.len());
        output.extend_from_slice(&salt);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    /// Open (decrypt) sealed data with AES-256-GCM.
    ///
    /// The vault must be unlocked (i.e., `unlock()` called with the correct
    /// password matching the salt stored in the sealed data).
    pub fn open(&mut self, sealed_bytes: &[u8], aad: &[u8]) -> Result<Vec<u8>, KeystoreError> {
        if !self.unlocked {
            return Err(KeystoreError::Locked);
        }

        if sealed_bytes.len() < 44 {
            return Err(KeystoreError::InvalidCiphertext);
        }

        let nonce_bytes = &sealed_bytes[32..44];
        let ciphertext = &sealed_bytes[44..];

        let key = self.derived_key.as_ref().ok_or(KeystoreError::Locked)?;

        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|e| KeystoreError::Crypto(format!("AES-GCM init: {e}")))?;

        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| KeystoreError::InvalidPassword)?;

        Ok(plaintext)
    }

    /// Returns true if the vault is unlocked.
    pub fn is_unlocked(&self) -> bool {
        self.unlocked
    }

    /// Returns a reference to the sealed data, if any.
    pub fn sealed_data(&self) -> Option<&SealedData> {
        self.sealed.as_ref()
    }
}

/// One-shot encrypt with a password (salt || nonce || ciphertext).
pub fn encrypt_with_password(
    password: &[u8],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, KeystoreError> {
    let mut salt = [0u8; 32];
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut salt);
    rand::rng().fill_bytes(&mut nonce_bytes);

    let key = derive_key(password, &salt);
    let cipher = Aes256Gcm::new_from_slice(key.as_ref())
        .map_err(|e| KeystoreError::Crypto(format!("AES-GCM init: {e}")))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|e| KeystoreError::Crypto(format!("encryption failed: {e}")))?;

    let mut output = Vec::with_capacity(32 + 12 + ciphertext.len());
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

/// One-shot decrypt with a password.
pub fn decrypt_with_password(
    password: &[u8],
    sealed: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, KeystoreError> {
    if sealed.len() < 44 {
        return Err(KeystoreError::InvalidCiphertext);
    }

    let salt = &sealed[..32];
    let nonce_bytes = &sealed[32..44];
    let ciphertext = &sealed[44..];

    let key = derive_key(password, salt);
    let cipher = Aes256Gcm::new_from_slice(key.as_ref())
        .map_err(|e| KeystoreError::Crypto(format!("AES-GCM init: {e}")))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| KeystoreError::InvalidPassword)?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seal_open_roundtrip() {
        let mut vault = Vault::new();
        let password = b"correct-horse-battery-staple";
        let data = b"sensitive key material";
        let aad = b"wallet-context";

        let sealed = vault.seal(password, data, aad).expect("test invariant");
        assert!(vault.is_unlocked());

        vault.lock(password).expect("test invariant");
        assert!(!vault.is_unlocked());

        // Open should fail when locked
        let mut sealed_copy = sealed.clone();
        let result = vault.open(&mut sealed_copy, aad);
        assert!(result.is_err());

        // Unlock and open
        vault.unlock(password).expect("test invariant");
        assert!(vault.is_unlocked());

        let decrypted = vault.open(&sealed, aad).expect("test invariant");
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_one_shot_encrypt_decrypt() {
        let password = b"my-secret-password";
        let plaintext = b"wallet-private-key-data";
        let aad = b"application-context";

        let sealed = encrypt_with_password(password, plaintext, aad).expect("test invariant");
        let decrypted = decrypt_with_password(password, &sealed, aad).expect("test invariant");
        assert_eq!(decrypted, plaintext);

        let wrong = decrypt_with_password(b"wrong-password", &sealed, aad);
        assert!(wrong.is_err());
    }

    #[test]
    fn test_tampered_ciphertext() {
        let password = b"password";
        let plaintext = b"data";
        let aad = b"ctx";

        let mut sealed = encrypt_with_password(password, plaintext, aad).expect("test invariant");
        if let Some(last) = sealed.last_mut() {
            *last ^= 0xff;
        }

        let result = decrypt_with_password(password, &sealed, aad);
        assert!(result.is_err());
    }
}
