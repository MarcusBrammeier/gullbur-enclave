//! Device key provider abstraction.
//!
//! The vault encrypts its persisted seed with a per-device 32-byte key. That
//! key's *source* is platform-specific:
//!
//! - On desktop (Linux/macOS/Windows) the key is a random value persisted in
//!   app-private storage (`~/.gullbur/keystore.key`).
//! - On Android the key should be generated inside the hardware-backed Android
//!   KeyStore (TEE) so the plaintext key never touches disk.
//!
//! This module defines the `DeviceKeyProvider` trait and the default
//! `FileDeviceKeyProvider` (the existing desktop behaviour, unchanged). An
//! Android backend can supply `AndroidKeyStoreProvider` behind it without the
//! rest of the vault caring where the key came from.
use crate::error::KeystoreError;
use rand::RngCore;
use std::path::PathBuf;

/// Supplies the 32-byte per-device key used to seal the persisted seed.
pub trait DeviceKeyProvider: Send + Sync + std::fmt::Debug {
    /// Return the 32-byte device key, creating and persisting it if needed.
    fn get_or_create_key(&self) -> Result<[u8; 32], KeystoreError>;

    /// Human-readable backend name for diagnostics (e.g. "file", "android-keystore").
    fn backend_name(&self) -> &'static str;
}

/// Default desktop backend: a random 32-byte key persisted at
/// `$DATA_DIR/keystore.key` with mode 0600 on Unix.
#[derive(Debug)]
pub struct FileDeviceKeyProvider {
    data_dir: PathBuf,
}

impl FileDeviceKeyProvider {
    pub const KEY_FILE: &'static str = "keystore.key";

    /// Create a provider backed by the given directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Create a provider backed by the platform user-data dir (`~/.gullbur`).
    pub fn default_home() -> Self {
        let dir = dirs_next::home_dir().unwrap_or_default().join(".gullbur");
        Self::new(dir)
    }
}

impl DeviceKeyProvider for FileDeviceKeyProvider {
    fn backend_name(&self) -> &'static str {
        "file"
    }

    fn get_or_create_key(&self) -> Result<[u8; 32], KeystoreError> {
        let key_path = self.data_dir.join(Self::KEY_FILE);

        // Existing key → load it.
        if let Ok(raw) = std::fs::read(&key_path)
            && raw.len() == 32
        {
            let mut key = [0u8; 32];
            key.copy_from_slice(&raw);
            return Ok(key);
        }

        // Otherwise generate a fresh random key and persist it.
        let mut key = [0u8; 32];
        rand::rng().fill_bytes(&mut key);
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| KeystoreError::Internal(format!("failed to create data dir: {e}")))?;
        std::fs::write(&key_path, key)
            .map_err(|e| KeystoreError::Internal(format!("failed to persist device key: {e}")))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic in-memory backend for tests (no disk required).
    #[derive(Debug)]
    struct TestProvider;

    impl DeviceKeyProvider for TestProvider {
        fn backend_name(&self) -> &'static str {
            "test"
        }
        fn get_or_create_key(&self) -> Result<[u8; 32], KeystoreError> {
            Ok([0x42; 32])
        }
    }

    #[test]
    fn file_provider_roundtrips_key_across_calls() {
        let dir = std::env::temp_dir().join(format!("gullbur-devkey-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("test invariant");
        let provider = FileDeviceKeyProvider::new(dir.clone());

        let k1 = provider.get_or_create_key().expect("test invariant");
        let k2 = provider.get_or_create_key().expect("test invariant");
        assert_eq!(k1, k2, "key must be stable across calls (persisted)");
        assert_eq!(provider.backend_name(), "file");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_provider_generates_nonzero_key() {
        let dir = std::env::temp_dir().join(format!("gullbur-devkey2-{}", std::process::id()));
        let provider = FileDeviceKeyProvider::new(dir.clone());
        let key = provider.get_or_create_key().expect("test invariant");
        assert!(
            key.iter().any(|&b| b != 0),
            "fresh random key should be non-zero"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trait_supports_alternate_backends() {
        // Any `DeviceKeyProvider` can be used interchangeably — this is the seam
        // an Android KeyStore backend plugs into.
        let provider: Box<dyn DeviceKeyProvider> = Box::new(TestProvider);
        assert_eq!(provider.backend_name(), "test");
        assert_eq!(
            provider.get_or_create_key().expect("test invariant"),
            [0x42; 32]
        );
    }
}
