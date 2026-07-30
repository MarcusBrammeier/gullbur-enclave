/// Authentication traits — generic interfaces for hardware gating.
///
/// These traits are the seam between `auth-core` and the actual hardware
/// drivers. Sprint C ships with `SoftwareAuth` (a dialog-based fallback).
/// Sprint D will add `TauriBiometryAuth` and `CtapFido2Auth` as
/// alternative implementors — swapped in `auth-core/src/lib.rs` without
/// touching `vault-core`.
use crate::auth::AuthStatus;
use crate::error::AuthError;

/// Result of an authentication challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    /// Authentication approved.
    Approved,
    /// Authentication denied by user.
    Denied,
    /// Authentication failed due to error.
    Failed(String),
}

/// A hardware or software authentication provider.
///
/// Each implementor handles one auth modality:
/// - `SoftwareAuth` — dialog-based confirmation (no hardware needed)
/// - `TauriBiometryAuth` — TouchID / Windows Hello / Linux PAM
/// - `CtapFido2Auth` — FIDO2 YubiKey via CTAP HID
pub trait AuthProvider: Send + Sync {
    /// A human-readable label for this provider (e.g., "Touch ID", "YubiKey").
    fn name(&self) -> &'static str;

    /// Request approval for an operation at the given auth level.
    ///
    /// - `required_level`: The minimum auth level needed. If the current
    ///   status is below this, the provider should escalate.
    /// - `operation`: A human-readable description of what's being approved.
    ///
    /// Returns `AuthResult::Approved` on success.
    fn request_approval(
        &self,
        required_level: AuthStatus,
        operation: &str,
    ) -> Result<AuthResult, AuthError>;
}