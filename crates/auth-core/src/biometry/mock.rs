use crate::{AuthError, AuthStatus};
use super::BiometricEngine;

/// A mock biometric engine that always succeeds for `BiometricUnlocked`
/// and returns `Err(NotSupported)` for `HardwareRequired`.
///
/// Used in CI/CD and testing when no real biometric hardware is available.
pub struct MockEngine;

impl BiometricEngine for MockEngine {
    fn name(&self) -> &'static str {
        "Mock Engine"
    }

    fn verify(&self, status: AuthStatus) -> Result<(), AuthError> {
        match status {
            AuthStatus::BiometricUnlocked => Ok(()),
            AuthStatus::HardwareRequired => Err(AuthError::NotSupported),
            AuthStatus::Unauthenticated => Err(AuthError::NotSupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_engine_name() {
        assert_eq!(MockEngine.name(), "Mock Engine");
    }

    #[test]
    fn mock_engine_verify_biometric_succeeds() {
        assert!(MockEngine.verify(AuthStatus::BiometricUnlocked).is_ok());
    }

    #[test]
    fn mock_engine_verify_hardware_returns_not_supported() {
        let result = MockEngine.verify(AuthStatus::HardwareRequired);
        assert!(matches!(result, Err(AuthError::NotSupported)));
    }

    #[test]
    fn mock_engine_verify_unauthenticated_returns_not_supported() {
        let result = MockEngine.verify(AuthStatus::Unauthenticated);
        assert!(matches!(result, Err(AuthError::NotSupported)));
    }
}