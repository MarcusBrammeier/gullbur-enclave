#![allow(clippy::type_complexity)]
use super::BiometricEngine;
use crate::{AuthError, AuthStatus};
use std::sync::Arc;

/// A biometric engine backed by the Tauri biometric plugin.
///
/// The actual Tauri `AppHandle` is injected via a callback closure,
/// avoiding a direct dependency on `tauri` or `tauri-plugin-biometric`
/// in this crate. The desktop app wires the closure at startup.
///
/// The callback signature is:
/// ```ignore
/// fn(reason: &str) -> Result<(), String>
/// ```
/// where `reason` is the auth prompt text and `Err(String)` is a
/// user-facing error message from the platform biometric API.
pub struct TauriBiometryEngine {
    /// Callback to invoke the platform biometric dialog.
    /// Returns `Ok(())` on success, `Err(msg)` on failure/cancellation.
    auth_fn: Option<Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>>,
}

impl TauriBiometryEngine {
    /// Create a new engine with no auth callback set.
    ///
    /// Call `set_auth_fn` before using the engine, otherwise all
    /// verify calls will return `Err(NotSupported)`.
    pub fn new() -> Self {
        Self { auth_fn: None }
    }

    /// Inject the platform biometric callback.
    ///
    /// The callback receives a reason string and must invoke the
    /// OS biometric dialog (e.g., `app_handle.authenticate_biometric()`).
    pub fn set_auth_fn<F>(&mut self, f: F)
    where
        F: Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.auth_fn = Some(Arc::new(f));
    }
}

impl Default for TauriBiometryEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl BiometricEngine for TauriBiometryEngine {
    fn name(&self) -> &'static str {
        "tauri-biometric"
    }

    /// Verify biometric status — delegates to the platform dialog.
    fn verify(&self, status: AuthStatus) -> Result<(), AuthError> {
        match status {
            AuthStatus::BiometricUnlocked => match self.auth_fn.as_ref() {
                Some(f) => match f("biometric authentication required") {
                    Ok(()) => Ok(()),
                    Err(msg) => Err(AuthError::BiometricFailed(msg)),
                },
                None => Err(AuthError::NotSupported),
            },
            AuthStatus::HardwareRequired => Err(AuthError::NotSupported),
            AuthStatus::Unauthenticated => Err(AuthError::NotSupported),
        }
    }
}
