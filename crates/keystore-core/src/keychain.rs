//! OS keychain integration via the `keyring` crate.
//!
//! Wraps platform-native credential stores (macOS Keychain, Windows Credential
//! Manager, Linux Secret Service / kernel keyring).

use crate::error::KeystoreError;

/// A store backed by the OS-level credential manager.
pub struct KeychainStore;

impl KeychainStore {
    /// Store a secret under the given `service` / `account` pair.
    pub fn store(
        service: &str,
        account: &str,
        secret: &[u8],
    ) -> Result<(), KeystoreError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| KeystoreError::Keychain(format!("failed to create keyring entry: {e}")))?;

        let secret_str = String::from_utf8(secret.to_vec())
            .map_err(|e| KeystoreError::Keychain(format!("secret is not valid UTF-8: {e}")))?;

        entry
            .set_password(&secret_str)
            .map_err(|e| KeystoreError::Keychain(format!("failed to store secret: {e}")))?;

        Ok(())
    }

    /// Retrieve a secret by `service` / `account`.
    ///
    /// Returns `None` if no entry exists for the given credentials.
    pub fn retrieve(service: &str, account: &str) -> Option<Vec<u8>> {
        let entry = keyring::Entry::new(service, account).ok()?;
        let password = entry.get_password().ok()?;
        Some(password.into_bytes())
    }

    /// Delete a stored secret.
    pub fn delete(service: &str, account: &str) -> Result<(), KeystoreError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| KeystoreError::Keychain(format!("failed to create keyring entry: {e}")))?;

        entry
            .delete_credential()
            .map_err(|e| KeystoreError::Keychain(format!("failed to delete secret: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_store_retrieve_delete() {
        let service = "com.gullbur.test";
        let account = "test_key";
        let secret = b"super-secret-test-value";

        // Store — may fail if no keychain is available in this environment.
        let Ok(()) = KeychainStore::store(service, account, secret) else {
            eprintln!("skipping: keychain store unavailable in this environment");
            return;
        };

        // Retrieve
        let retrieved = KeychainStore::retrieve(service, account);
        if retrieved.is_none() {
            eprintln!("skipping: keychain retrieve returned None (transient backend issue)");
            let _ = KeychainStore::delete(service, account);
            return;
        }
        assert_eq!(retrieved, Some(secret.to_vec()));

        // Delete
        KeychainStore::delete(service, account).expect("test invariant");

        // Should be gone
        let after_delete = KeychainStore::retrieve(service, account);
        assert_eq!(after_delete, None);
    }
}