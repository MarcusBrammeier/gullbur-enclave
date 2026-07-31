use crate::{AuthError, AuthStatus};

/// Platform-agnostic biometric authentication engine.
///
/// Each platform implements this trait:
/// - `MockEngine` — returns Ok for CI/CD testing
/// - `TauriBiometryEngine` — wraps tauri-plugin-biometric for desktop
///
/// The engine is injected into the Vault struct via `Arc<dyn BiometricEngine>`
/// at Tauri setup time (lib.rs::setup()).
pub trait BiometricEngine: Send + Sync {
    /// A human-readable name for this engine (e.g. "Touch ID", "Mock").
    fn name(&self) -> &'static str;

    /// Verify the user's identity at the given auth status level.
    ///
    /// - `AuthStatus::BiometricUnlocked` — standard biometric check
    /// - `AuthStatus::HardwareRequired` — should return `Err(NotSupported)`
    ///   since FIDO2 is handled separately
    ///
    /// Returns:
    /// - `Ok(())` — authentication successful
    /// - `Err(AuthError::NotSupported)` — this engine can't handle this level
    /// - `Err(AuthError::PermissionDenied)` — user cancelled or denied
    /// - `Err(AuthError::BiometricFailed(msg))` — actual hardware error
    fn verify(&self, status: AuthStatus) -> Result<(), AuthError>;
}

pub mod mock;
pub mod tauri;
