//! Encrypted vault — AES-256-GCM envelope encryption with Argon2id KDF.
//!
//! The vault seals arbitrary data under a password. Key material is derived
//! from the password with **Argon2id** (memory-hard) and a random salt, then
//! used for AES-256-GCM encryption. The stored format (v2, self-describing):
//!
//! ```text
//! [MAGIC "GBKF" 4B][version u8=2][argon m_cost u32LE][argon t_cost u32LE]
//! [argon p_cost u8][salt 16B][nonce 12B][ciphertext ...]
//! ```
//!
//! Backward compatibility: pre-v2 vaults were sealed with HKDF-SHA256 in a
//! fixed `salt(32) || nonce(12) || ciphertext` layout with no magic prefix.
//! Decryption auto-detects the format: if the blob starts with the `GBKF`
//! magic it is parsed as v2 (Argon2id); otherwise it falls back to the legacy
//! HKDF layout. New seals always emit v2 (Argon2id), so existing saves keep
//! working and are transparently upgraded on next re-seal.
//!
//! Note: `Nonce::from_slice` is deprecated in aes-gcm 0.11 but the
//! replacement (`TryFrom`) has varying availability across versions.
//! Allowed until a stable cross-version API is available.
#![allow(deprecated)]

use crate::error::KeystoreError;
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::ZeroizeOnDrop;

/// Magic prefix identifying the v2 (Argon2id) sealed format.
const MAGIC: [u8; 4] = *b"GBKF";
/// Format version tag for Argon2id-encrypted blobs.
const KDF_VERSION_ARGON2: u8 = 2;

// ── Argon2id defaults (OWASP-informed, wallet-appropriate) ──
const ARGON2_SALT_LEN: usize = 16;
/// Memory cost (KiB) — 32 MiB.
const ARGON2_M_COST: u32 = 32 * 1024;
/// Time cost.
const ARGON2_T_COST: u32 = 3;
/// Parallelism.
const ARGON2_P_COST: u32 = 1;

/// An AES-256 key derived from a password.
#[derive(ZeroizeOnDrop)]
struct DerivedKey([u8; 32]);

impl AsRef<[u8]> for DerivedKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Argon2id KDF parameters that are stored alongside each sealed blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgonParams {
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Default for ArgonParams {
    fn default() -> Self {
        Self {
            m_cost: ARGON2_M_COST,
            t_cost: ARGON2_T_COST,
            p_cost: ARGON2_P_COST,
        }
    }
}

/// Derive a 256-bit key from a password + salt using Argon2id.
fn derive_key_argon2(
    password: &[u8],
    salt: &[u8],
    p: &ArgonParams,
) -> Result<DerivedKey, KeystoreError> {
    let params = Params::new(p.m_cost, p.t_cost, p.p_cost, Some(32))
        .map_err(|e| KeystoreError::Crypto(format!("argon2 params: {e}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = DerivedKey([0u8; 32]);
    argon2
        .hash_password_into(password, salt, &mut key.0)
        .map_err(|e| KeystoreError::Crypto(format!("argon2id kdf failed: {e}")))?;
    Ok(key)
}

/// Derive a 256-bit key from a password + salt using HKDF-SHA256 (legacy).
fn derive_key_hkdf(password: &[u8], salt: &[u8]) -> DerivedKey {
    let mut key = DerivedKey([0u8; 32]);
    hkdf::Hkdf::<Sha256>::new(Some(salt), password)
        .expand(b"gullbur-vault-key", &mut key.0)
        .expect("HKDF expand: 32 bytes fits SHA-256 output");
    key
}

/// A sealed blob: salt + nonce + ciphertext (plus KDF metadata for v2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedData {
    pub kdf: u8,
    pub salt: Vec<u8>,
    pub argon: Option<ArgonParams>,
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
        let key = match sealed.kdf {
            KDF_VERSION_ARGON2 => {
                let p = sealed
                    .argon
                    .as_ref()
                    .ok_or(KeystoreError::InvalidCiphertext)?;
                derive_key_argon2(password, &sealed.salt, p)?
            }
            _ => derive_key_hkdf(password, &sealed.salt),
        };
        self.derived_key = Some(key);
        self.unlocked = true;
        Ok(())
    }

    /// Seal (encrypt) data with AES-256-GCM (Argon2id key derivation).
    ///
    /// Returns serialized bytes: magic || header || salt || nonce || ciphertext.
    pub fn seal(
        &mut self,
        password: &[u8],
        data: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, KeystoreError> {
        let argon = ArgonParams::default();
        let mut salt = [0u8; ARGON2_SALT_LEN];
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut salt);
        rand::rng().fill_bytes(&mut nonce_bytes);

        let key = derive_key_argon2(password, &salt, &argon)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|e| KeystoreError::Crypto(format!("AES-GCM init: {e}")))?;

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, Payload { msg: data, aad })
            .map_err(|e| KeystoreError::Crypto(format!("encryption failed: {e}")))?;

        let sealed = SealedData {
            kdf: KDF_VERSION_ARGON2,
            salt: salt.to_vec(),
            argon: Some(argon),
            nonce: nonce_bytes.to_vec(),
            ciphertext: ciphertext.clone(),
        };

        // Store internally
        self.sealed = Some(sealed);
        self.derived_key = Some(key);

        Ok(serialize_v2(&argon, &salt, &nonce_bytes, &ciphertext))
    }

    /// Open (decrypt) sealed data with AES-256-GCM.
    ///
    /// The vault must be unlocked (i.e., `unlock()` called with the correct
    /// password matching the salt stored in the sealed data).
    pub fn open(&mut self, sealed_bytes: &[u8], aad: &[u8]) -> Result<Vec<u8>, KeystoreError> {
        if !self.unlocked {
            return Err(KeystoreError::Locked);
        }

        let (_, nonce_bytes, ciphertext) = split_blob(sealed_bytes)?;
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

/// Split a sealed blob into its logical parts, auto-detecting the format.
///
/// Returns `(salt, nonce, ciphertext)`.
type BlobParts<'a> = (&'a [u8], &'a [u8], &'a [u8]);

fn split_blob(blob: &[u8]) -> Result<BlobParts<'_>, KeystoreError> {
    if blob.len() < 4 {
        return Err(KeystoreError::InvalidCiphertext);
    }
    if blob[..4] == MAGIC {
        // v2: magic(4) || version(1) || m(4) || t(4) || p(1) || salt(16) || nonce(12) || ct
        let header_len = 4 + 1 + 4 + 4 + 1 + ARGON2_SALT_LEN + 12;
        if blob.len() < header_len {
            return Err(KeystoreError::InvalidCiphertext);
        }
        let salt = &blob[14..30];
        let nonce = &blob[30..42];
        let ct = &blob[42..];
        Ok((salt, nonce, ct))
    } else {
        // Legacy: salt(32) || nonce(12) || ct
        if blob.len() < 44 {
            return Err(KeystoreError::InvalidCiphertext);
        }
        let salt = &blob[0..32];
        let nonce = &blob[32..44];
        let ct = &blob[44..];
        Ok((salt, nonce, ct))
    }
}

/// Parse the argon2 params from a v2 blob (magic must already be confirmed).
fn parse_v2_params(blob: &[u8]) -> Result<ArgonParams, KeystoreError> {
    if blob.len() < 14 {
        return Err(KeystoreError::InvalidCiphertext);
    }
    // offsets: magic(4) version(1) m(4) t(4) p(1)
    let m_cost = u32::from_le_bytes([blob[5], blob[6], blob[7], blob[8]]);
    let t_cost = u32::from_le_bytes([blob[9], blob[10], blob[11], blob[12]]);
    let p_cost = blob[13] as u32;
    Ok(ArgonParams {
        m_cost,
        t_cost,
        p_cost,
    })
}

/// Serialize a v2 blob: magic || version || params || salt || nonce || ciphertext.
fn serialize_v2(p: &ArgonParams, salt: &[u8], nonce: &[u8], ct: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(42 + ct.len());
    out.extend_from_slice(&MAGIC);
    out.push(KDF_VERSION_ARGON2);
    out.extend_from_slice(&p.m_cost.to_le_bytes());
    out.extend_from_slice(&p.t_cost.to_le_bytes());
    out.push(p.p_cost as u8);
    out.extend_from_slice(salt);
    out.extend_from_slice(nonce);
    out.extend_from_slice(ct);
    out
}

/// One-shot encrypt with a password (Argon2id KDF → salt || nonce || ciphertext).
pub fn encrypt_with_password(
    password: &[u8],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, KeystoreError> {
    let argon = ArgonParams::default();
    let mut salt = [0u8; ARGON2_SALT_LEN];
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut salt);
    rand::rng().fill_bytes(&mut nonce_bytes);

    let key = derive_key_argon2(password, &salt, &argon)?;
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

    Ok(serialize_v2(&argon, &salt, &nonce_bytes, &ciphertext))
}

/// One-shot decrypt with a password (auto-detects Argon2id vs legacy HKDF).
pub fn decrypt_with_password(
    password: &[u8],
    sealed: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, KeystoreError> {
    if sealed.len() < 4 {
        return Err(KeystoreError::InvalidCiphertext);
    }

    // Auto-detect: v2 (magic) → Argon2id; otherwise legacy HKDF.
    let key = if sealed[..4] == MAGIC {
        let params = parse_v2_params(sealed)?;
        let salt = &sealed[14..30];
        derive_key_argon2(password, salt, &params)?
    } else {
        if sealed.len() < 44 {
            return Err(KeystoreError::InvalidCiphertext);
        }
        derive_key_hkdf(password, &sealed[0..32])
    };

    let (_, nonce_bytes, ciphertext) = split_blob(sealed)?;
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

// ── Device-key file encryption (raw 32-byte key, no KDF) ──────────────
//
// Used for at-rest encryption of non-secret-but-sensitive files like
// `accounts.json`, which must be readable before the user unlocks with their
// password. The 32-byte device key (keystore.key) is already high-entropy
// random, so no memory-hard KDF is needed. Format:
//   `[MAGIC "GBAF" 4B][version u8=1][nonce 12B][ciphertext ...]`

/// Magic prefix for device-key AES-GCM file blobs.
const FILE_MAGIC: [u8; 4] = *b"GBAF";
const FILE_VERSION: u8 = 1;

/// Encrypt a file payload with a raw 32-byte key (AES-256-GCM).
pub fn encrypt_file_with_key(
    key: &[u8; 32],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, KeystoreError> {
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(key)
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

    let mut out = Vec::with_capacity(17 + ciphertext.len());
    out.extend_from_slice(&FILE_MAGIC);
    out.push(FILE_VERSION);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a device-key encrypted file blob produced by `encrypt_file_with_key`.
///
/// Returns `Ok(None)` if the input is not a device-key blob (e.g. a legacy
/// plaintext file still on disk) — callers may then fall back to reading it
/// as plaintext. Returns `Err` on magic match but wrong key (integrity fail).
pub fn decrypt_file_with_key(
    key: &[u8; 32],
    blob: &[u8],
    aad: &[u8],
) -> Result<Option<Vec<u8>>, KeystoreError> {
    if blob.len() < 17 {
        return Ok(None);
    }
    if blob[..4] != FILE_MAGIC {
        return Ok(None); // not a device-key blob → legacy plaintext
    }
    let nonce_bytes = &blob[5..17];
    let ciphertext = &blob[17..];
    let cipher = Aes256Gcm::new_from_slice(key)
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
    Ok(Some(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_key_file_encrypt_decrypt_roundtrip() {
        let key = [0x77u8; 32];
        let payload = br#"{"accounts":[]}"#;
        let aad = b"gullbur-accounts";

        let blob = encrypt_file_with_key(&key, payload, aad).expect("test invariant");
        assert_eq!(&blob[..4], &FILE_MAGIC);
        assert_eq!(blob[4], FILE_VERSION);

        let decrypted = decrypt_file_with_key(&key, &blob, aad)
            .expect("test invariant")
            .expect("should decrypt");
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn test_device_key_wrong_key_fails() {
        let key = [0x11u8; 32];
        let blob = encrypt_file_with_key(&key, b"data", b"aad").expect("test invariant");
        let wrong = decrypt_file_with_key(&[0x22u8; 32], &blob, b"aad");
        // Magic matches → must be Err (integrity failure), not None.
        assert!(wrong.is_err(), "wrong key on a GBAF blob must fail");
    }

    #[test]
    fn test_device_key_empty_or_legacy_returns_none() {
        let key = [0x33u8; 32];
        // Legacy plaintext (no magic) → Ok(None)
        let legacy = b"[\"plaintext json\"]";
        let r = decrypt_file_with_key(&key, legacy, b"aad").expect("test invariant");
        assert!(r.is_none(), "legacy file must be reported as None");
    }

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
        let sealed_copy = sealed.clone();
        let result = vault.open(&sealed_copy, aad);
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

    #[test]
    fn test_v2_blob_has_magic_and_self_describing_params() {
        let sealed = encrypt_with_password(b"pw", b"payload", b"aad").expect("test invariant");
        assert_eq!(&sealed[..4], &MAGIC, "v2 blob must carry GBKF magic");
        assert_eq!(sealed[4], KDF_VERSION_ARGON2);
        let params = parse_v2_params(&sealed).expect("test invariant");
        assert_eq!(params.m_cost, ARGON2_M_COST);
        assert_eq!(params.t_cost, ARGON2_T_COST);
        // salt + nonce + ciphertext present after header
        assert!(sealed.len() > 42);
    }

    #[test]
    fn test_decrypt_legacy_hkdf_format_still_works() {
        // Simulate a legacy pre-upgrade blob: salt(32) || nonce(12) || ct,
        // encrypted under the old HKDF derivation.
        let password = b"legacy-vault-password";
        let plaintext = b"old persisted seed";
        let aad = b"vault-seed";

        let mut salt = [0u8; 32];
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut salt);
        rand::rng().fill_bytes(&mut nonce_bytes);

        let key = derive_key_hkdf(password, &salt);
        let cipher = Aes256Gcm::new_from_slice(key.as_ref()).expect("test invariant");
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ct = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .expect("test invariant");

        let mut legacy = Vec::new();
        legacy.extend_from_slice(&salt);
        legacy.extend_from_slice(&nonce_bytes);
        legacy.extend_from_slice(&ct);

        let decrypted = decrypt_with_password(password, &legacy, aad).expect("legacy decrypt");
        assert_eq!(
            decrypted, plaintext,
            "legacy HKDF blob must remain decryptable"
        );

        // Wrong password must still fail on legacy format.
        assert!(decrypt_with_password(b"wrong", &legacy, aad).is_err());
    }

    #[test]
    fn test_argon2_mem_safety() {
        assert!(
            ARGON2_M_COST >= 16 * 1024,
            "m_cost should be at least 16 MiB"
        );
    }
}
