/// Hardware Silicon Gating — Auth Status & State Machine
///
/// Defines the authentication state machine for Sprint C:
/// - `Unauthenticated` — No auth done, session key not available
/// - `BiometricUnlocked` — Biometric auth passed, session key loaded
/// - `HardwareRequired` — FIDO2 touch needed for this specific operation
///
/// The vault-core checks `AuthManager::status()` before routing any
/// JSON-RPC request. Errors propagate as `RpcError::AuthRequired()`.
use crate::error::AuthError;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::SystemTime;

/// Numeric representation for lock-free atomic storage.
const UNAUTHENTICATED: u8 = 0;
const BIOMETRIC_UNLOCKED: u8 = 1;
const HARDWARE_REQUIRED: u8 = 2;

/// Current authentication status of the vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStatus {
    /// No authentication has been performed — session key is not available.
    /// All signing operations will return `AuthRequired`.
    Unauthenticated,

    /// Biometric authentication passed (TouchID / Windows Hello / PAM).
    /// Session key is loaded and standard operations are permitted.
    BiometricUnlocked,

    /// A FIDO2 hardware touch is required for this specific high-value
    /// operation (e.g., `vault_executeBatch`, `vault_requestSessionKey`).
    HardwareRequired,
}

impl AuthStatus {
    fn _to_u8(self) -> u8 {
        match self {
            AuthStatus::Unauthenticated => UNAUTHENTICATED,
            AuthStatus::BiometricUnlocked => BIOMETRIC_UNLOCKED,
            AuthStatus::HardwareRequired => HARDWARE_REQUIRED,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            BIOMETRIC_UNLOCKED => AuthStatus::BiometricUnlocked,
            HARDWARE_REQUIRED => AuthStatus::HardwareRequired,
            _ => AuthStatus::Unauthenticated,
        }
    }

    /// Returns `true` if the vault is in a state where signing is permitted.
    pub fn can_sign(self) -> bool {
        matches!(self, AuthStatus::BiometricUnlocked)
    }

    /// Returns `true` if this status represents a locked state.
    pub fn is_locked(self) -> bool {
        matches!(self, AuthStatus::Unauthenticated)
    }

    /// Returns a lowercase snake_case string for frontend use.
    pub fn as_str(self) -> &'static str {
        match self {
            AuthStatus::Unauthenticated => "unauthenticated",
            AuthStatus::BiometricUnlocked => "biometric_unlocked",
            AuthStatus::HardwareRequired => "hardware_required",
        }
    }
}

/// Thread-safe authentication state machine.
///
/// Uses an `AtomicU8` for lock-free reads — no `Mutex` or `RwLock` needed
/// for status checks in the hot path (every JSON-RPC dispatch).
pub struct AuthManager {
    status: AtomicU8,
    /// Duration in milliseconds before the vault auto-locks due to inactivity.
    /// Default: 30_000 ms (30 seconds). Set to 0 to disable auto-lock.
    auto_lock_duration: AtomicU64,
    /// Timestamp (millis since UNIX_EPOCH) of the last user interaction.
    last_interaction: AtomicU64,
}

impl AuthManager {
    /// Create a new `AuthManager` in the `Unauthenticated` state.
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(UNAUTHENTICATED),
            auto_lock_duration: AtomicU64::new(30_000),
            last_interaction: AtomicU64::new(0),
        }
    }

    /// Return the current authentication status (lock-free read).
    pub fn status(&self) -> AuthStatus {
        AuthStatus::from_u8(self.status.load(Ordering::Acquire))
    }

    /// Transition to `BiometricUnlocked`.
    ///
    /// Called when biometric authentication succeeds. The caller is
    /// responsible for loading the session key into secure memory.
    pub fn try_biometric(&self) -> Result<(), AuthError> {
        // Can only transition from Unauthenticated
        self.status
            .compare_exchange(
                UNAUTHENTICATED,
                BIOMETRIC_UNLOCKED,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .map_err(|_| {
                AuthError::Internal("Already authenticated or hardware required".into())
            })?;
        Ok(())
    }

    /// Transition to `HardwareRequired`.
    ///
    /// Called when a high-value operation needs a physical FIDO2 touch.
    pub fn request_hardware(&self) -> Result<(), AuthError> {
        self.status
            .compare_exchange(
                BIOMETRIC_UNLOCKED,
                HARDWARE_REQUIRED,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .map_err(|_| AuthError::Internal("Must be biometric unlocked first".into()))?;
        Ok(())
    }

    /// Confirm the FIDO2 touch — transition back to `BiometricUnlocked`.
    pub fn confirm_hardware(&self) -> Result<(), AuthError> {
        self.status
            .compare_exchange(
                HARDWARE_REQUIRED,
                BIOMETRIC_UNLOCKED,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .map_err(|_| AuthError::Internal("Hardware confirmation not pending".into()))?;
        Ok(())
    }

    /// Lock the vault — transition to `Unauthenticated`.
    ///
    /// Wipes the auth state. The caller is responsible for also wiping
    /// the session key from secure memory and notifying IPC clients.
    pub fn lock(&self) {
        self.status.store(UNAUTHENTICATED, Ordering::Release);
    }

    /// Record a user interaction, resetting the auto-lock timer.
    ///
    /// Call this whenever the user performs an action (keystroke, click,
    /// touch) that should keep the vault unlocked.
    pub fn touch(&self) {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_interaction.store(now, Ordering::Release);
    }

    /// Set the auto-lock duration in milliseconds.
    ///
    /// Set to 0 to disable auto-lock entirely.
    pub fn set_auto_lock_duration(&self, millis: u64) {
        self.auto_lock_duration.store(millis, Ordering::Release);
    }

    /// Return the current auto-lock duration in milliseconds.
    pub fn auto_lock_duration(&self) -> u64 {
        self.auto_lock_duration.load(Ordering::Acquire)
    }

    /// Check if the auto-lock timer has expired and, if so, lock the vault.
    ///
    /// Returns `true` if the vault was just locked (auto-lock triggered),
    /// `false` otherwise. If `auto_lock_duration` is 0, auto-lock is disabled
    /// and this always returns `false`.
    pub fn check_and_lock(&self) -> bool {
        let duration = self.auto_lock_duration.load(Ordering::Acquire);
        // A duration of 0 means auto-lock is disabled.
        if duration == 0 {
            return false;
        }

        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last = self.last_interaction.load(Ordering::Acquire);

        // If we've never interacted, don't auto-lock (fresh start).
        if last == 0 {
            return false;
        }

        if now.saturating_sub(last) > duration {
            self.lock();
            return true;
        }

        false
    }

    /// Return the number of seconds remaining before auto-lock triggers.
    ///
    /// Returns 0 if auto-lock is disabled (duration == 0) or has already
    /// expired. Saturates at u32 max.
    pub fn remaining_seconds(&self) -> u32 {
        let duration = self.auto_lock_duration.load(Ordering::Acquire);
        if duration == 0 {
            return 0;
        }

        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last = self.last_interaction.load(Ordering::Acquire);

        if last == 0 {
            return 0;
        }

        let elapsed = now.saturating_sub(last);
        if elapsed >= duration {
            return 0;
        }

        ((duration - elapsed) / 1000) as u32
    }

    /// Check if the vault is in a signable state.
    /// Returns `Ok(())` if `BiometricUnlocked`, `Err(AuthStatus)` otherwise.
    pub fn require_signing(&self) -> Result<(), AuthStatus> {
        let s = self.status();
        if s.can_sign() { Ok(()) } else { Err(s) }
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_unauthenticated() {
        let mgr = AuthManager::new();
        assert_eq!(mgr.status(), AuthStatus::Unauthenticated);
    }

    #[test]
    fn test_biometric_unlock_succeeds() {
        let mgr = AuthManager::new();
        assert!(mgr.try_biometric().is_ok());
        assert_eq!(mgr.status(), AuthStatus::BiometricUnlocked);
    }

    #[test]
    fn test_biometric_from_locked_fails() {
        let mgr = AuthManager::new();
        mgr.try_biometric().expect("test invariant");
        // Second unlock from BiometricUnlocked should fail
        assert!(mgr.try_biometric().is_err());
    }

    #[test]
    fn test_hardware_required() {
        let mgr = AuthManager::new();
        mgr.try_biometric().expect("test invariant");
        assert!(mgr.request_hardware().is_ok());
        assert_eq!(mgr.status(), AuthStatus::HardwareRequired);
    }

    #[test]
    fn test_hardware_confirm() {
        let mgr = AuthManager::new();
        mgr.try_biometric().expect("test invariant");
        mgr.request_hardware().expect("test invariant");
        mgr.confirm_hardware().expect("test invariant");
        assert_eq!(mgr.status(), AuthStatus::BiometricUnlocked);
    }

    #[test]
    fn test_lock_wipes_state() {
        let mgr = AuthManager::new();
        mgr.try_biometric().expect("test invariant");
        mgr.lock();
        assert_eq!(mgr.status(), AuthStatus::Unauthenticated);
    }

    #[test]
    fn test_require_signing_ok() {
        let mgr = AuthManager::new();
        mgr.try_biometric().expect("test invariant");
        assert!(mgr.require_signing().is_ok());
    }

    #[test]
    fn test_require_signing_err() {
        let mgr = AuthManager::new();
        assert_eq!(mgr.require_signing(), Err(AuthStatus::Unauthenticated));
    }

    #[test]
    fn test_can_sign() {
        assert!(AuthStatus::BiometricUnlocked.can_sign());
        assert!(!AuthStatus::Unauthenticated.can_sign());
        assert!(!AuthStatus::HardwareRequired.can_sign());
    }

    #[test]
    fn test_auto_lock_defaults_disabled() {
        let mgr = AuthManager::new();
        assert_eq!(mgr.auto_lock_duration(), 30_000);
        // Before first touch, remaining is 0 and check_and_lock is a no-op
        assert_eq!(mgr.remaining_seconds(), 0);
        assert!(!mgr.check_and_lock());
    }

    #[test]
    fn test_auto_lock_touch_resets_timer() {
        let mgr = AuthManager::new();
        mgr.set_auto_lock_duration(10_000);
        mgr.touch();
        let rem = mgr.remaining_seconds();
        assert!(rem > 0 && rem <= 10, "expected 1-10s, got {rem}");
        assert!(!mgr.check_and_lock());
    }

    #[test]
    fn test_auto_lock_set_get() {
        let mgr = AuthManager::new();
        mgr.set_auto_lock_duration(5_000);
        assert_eq!(mgr.auto_lock_duration(), 5_000);
        mgr.set_auto_lock_duration(0);
        assert_eq!(mgr.auto_lock_duration(), 0);
    }

    #[test]
    fn test_auto_lock_disabled_never_locks() {
        let mgr = AuthManager::new();
        mgr.try_biometric().expect("test invariant");
        mgr.set_auto_lock_duration(0);
        mgr.touch();
        assert!(!mgr.check_and_lock());
        assert!(mgr.status().can_sign());
    }

    #[test]
    fn test_auto_lock_expiry_triggers_lock() {
        let mgr = AuthManager::new();
        mgr.try_biometric().expect("test invariant");
        mgr.set_auto_lock_duration(1); // 1ms — expires immediately
        mgr.touch();
        // Give the system clock a tick
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(mgr.check_and_lock(), "should auto-lock after expiry");
        assert!(mgr.status().is_locked());
        assert_eq!(mgr.remaining_seconds(), 0);
    }

    #[test]
    fn test_is_locked() {
        assert!(AuthStatus::Unauthenticated.is_locked());
        assert!(!AuthStatus::BiometricUnlocked.is_locked());
        assert!(!AuthStatus::HardwareRequired.is_locked());
    }
}
