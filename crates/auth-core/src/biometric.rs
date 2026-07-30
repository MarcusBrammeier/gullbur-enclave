use async_trait::async_trait;

/// Trait for biometric authentication operations.
///
/// Implementations use platform-specific biometric APIs (e.g. Tauri's
/// biometric plugin on macOS/iOS/Android, Windows Hello on Windows).
#[async_trait]
pub trait BiometricAuth {
    /// Prompt the user for biometric authentication.
    ///
    /// `reason` is a human-readable string explaining why auth is needed
    /// (e.g. "Sign transaction 0xabcd...").
    ///
    /// Returns `true` if authentication succeeded, `false` if the user
    /// cancelled or the attempt failed.
    async fn authenticate(&self, reason: &str) -> Result<bool, super::AuthError>;
}

/// A test stub that always returns `Ok(true)`.
///
/// Used in development/testing when no real biometric hardware is available.
pub struct BiometricStub;

#[async_trait]
impl BiometricAuth for BiometricStub {
    async fn authenticate(&self, _reason: &str) -> Result<bool, super::AuthError> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_biometric_stub_always_returns_true() {
        let stub = BiometricStub;
        let result = stub.authenticate("test reason").await;
        assert!(result.is_ok());
        assert!(result.expect("test invariant"));
    }
}