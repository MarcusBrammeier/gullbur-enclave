/// Software Confirmation — dialog-based auth fallback.
///
/// This is the default `AuthProvider` implementation for Sprint C.
/// Instead of touching hardware, it returns `Approved` for biometric-level
/// requests and returns a configurable response for hardware-level requests.
///
/// In production, this is replaced by `TauriBiometryAuth` (Sprint D).
/// The swap requires only changing the registration in `auth-core/src/lib.rs`.
use crate::auth::AuthStatus;
use crate::error::AuthError;
use crate::traits::{AuthProvider, AuthResult};

/// Closure type for user confirmation prompts in software auth.
type PromptFn = Box<dyn Fn(&str) -> bool + Send + Sync>;

/// Software-based authentication provider.
///
/// - `Unauthenticated` level: always returns `Denied` (re-auth required)
/// - `BiometricUnlocked` level: returns `Approved` (no hardware needed)
/// - `HardwareRequired` level: returns `Denied` if `strict` mode is on,
///   otherwise calls a closure for user confirmation
pub struct SoftwareAuth {
    /// If `true`, hardware-level requests are denied outright (simulating
    /// the real FIDO2 behavior). Set `false` during development.
    strict: bool,
    /// Optional closure for rendering a UI confirmation prompt.
    /// Called when `strict` is `false` and a hardware-level approval is needed.
    on_prompt: Option<PromptFn>,
}

impl SoftwareAuth {
    /// Create a new `SoftwareAuth` in strict mode (hardware requests denied).
    pub fn new() -> Self {
        Self {
            strict: true,
            on_prompt: None,
        }
    }

    /// Create a `SoftwareAuth` in permissive mode with a confirmation callback.
    ///
    /// The callback receives a human-readable operation description and
    /// returns `true` (approved) or `false` (denied). This maps to a
    /// Svelte dialog prompt in the desktop app.
    pub fn with_prompt(prompt_fn: Box<dyn Fn(&str) -> bool + Send + Sync>) -> Self {
        Self {
            strict: false,
            on_prompt: Some(prompt_fn),
        }
    }
}

impl Default for SoftwareAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthProvider for SoftwareAuth {
    fn name(&self) -> &'static str {
        "Software Confirmation"
    }

    fn request_approval(
        &self,
        required_level: AuthStatus,
        operation: &str,
    ) -> Result<AuthResult, AuthError> {
        match required_level {
            AuthStatus::Unauthenticated => {
                // Can't approve at this level — re-auth needed
                Ok(AuthResult::Denied)
            }
            AuthStatus::BiometricUnlocked => {
                // Software mode approves biometric-level requests automatically
                Ok(AuthResult::Approved)
            }
            AuthStatus::HardwareRequired => {
                if self.strict {
                    // Strict mode: hardware requests denied — mimics real FIDO2
                    Ok(AuthResult::Denied)
                } else if let Some(ref prompt) = self.on_prompt {
                    // Permissive mode: show confirmation dialog
                    let approved = prompt(operation);
                    Ok(if approved {
                        AuthResult::Approved
                    } else {
                        AuthResult::Denied
                    })
                } else {
                    // No prompt configured — deny by default
                    Ok(AuthResult::Denied)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_auth_strict_biometric_ok() {
        let auth = SoftwareAuth::new();
        let result = auth.request_approval(AuthStatus::BiometricUnlocked, "sign transaction");
        assert_eq!(result, Ok(AuthResult::Approved));
    }

    #[test]
    fn test_software_auth_strict_hardware_denied() {
        let auth = SoftwareAuth::new();
        let result = auth.request_approval(AuthStatus::HardwareRequired, "batch execute");
        assert_eq!(result, Ok(AuthResult::Denied));
    }

    #[test]
    fn test_software_auth_unauthenticated_denied() {
        let auth = SoftwareAuth::new();
        let result = auth.request_approval(AuthStatus::Unauthenticated, "anything");
        assert_eq!(result, Ok(AuthResult::Denied));
    }

    #[test]
    fn test_software_auth_with_prompt_approved() {
        let auth = SoftwareAuth::with_prompt(Box::new(|_op| true));
        let result = auth.request_approval(AuthStatus::HardwareRequired, "test");
        assert_eq!(result, Ok(AuthResult::Approved));
    }

    #[test]
    fn test_software_auth_with_prompt_denied() {
        let auth = SoftwareAuth::with_prompt(Box::new(|_op| false));
        let result = auth.request_approval(AuthStatus::HardwareRequired, "test");
        assert_eq!(result, Ok(AuthResult::Denied));
    }
}
