//! Biometric lockout policy — pure, unit-testable state machine.
//!
//! This encapsulates the "N consecutive biometric denials disables native
//! biometry for the session" rule that lives inline in the Tauri commands.
//! Extracting it here makes the security policy testable without a UI, and
//! lets Android re-use the same rule when wiring its BiometricPrompt.
use crate::error::AuthError;

/// Outcome of recording a biometric attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiometricOutcome {
    /// Auth succeeded — failure counter reset.
    Success,
    /// Auth denied but under the lockout threshold.
    Denied { failures: u8 },
    /// Auth denied and the lockout threshold was crossed — native biometry
    /// must be disabled for the session.
    LockedOut,
}

/// Tracks consecutive biometric failures and applies the lockout rule.
#[derive(Debug, Default)]
pub struct BiometricPolicy {
    /// Max consecutive failures before native biometry is disabled.
    pub max_failures: u8,
}

impl BiometricPolicy {
    /// 5 consecutive failures disables native biometry (industry standard).
    pub const DEFAULT_MAX: u8 = 5;

    /// Create a policy with the default lockout threshold of 5.
    pub fn new() -> Self {
        Self {
            max_failures: Self::DEFAULT_MAX,
        }
    }

    /// Record a biometric result against a running failure counter.
    ///
    /// `current_failures` is the number of consecutive failures so far.
    /// Returns the outcome and the new failure count.
    pub fn record(&self, ok: bool, current_failures: u8) -> (BiometricOutcome, u8) {
        if ok {
            return (BiometricOutcome::Success, 0);
        }
        let failures = current_failures.saturating_add(1);
        if failures >= self.max_failures {
            (BiometricOutcome::LockedOut, failures)
        } else {
            (BiometricOutcome::Denied { failures }, failures)
        }
    }

    /// Convenience: map a success/error from an engine into a policy outcome.
    ///
    /// - `Ok(())` → Success (counter resets)
    /// - `Err(PermissionDenied)` → consumed by the failure counter
    /// - `Err(NotSupported)` → never counts as a user denial
    pub fn classify(
        &self,
        result: Result<(), AuthError>,
        current_failures: u8,
    ) -> (BiometricOutcome, u8) {
        match result {
            Ok(()) => self.record(true, current_failures),
            Err(AuthError::NotSupported) => (
                BiometricOutcome::Denied {
                    failures: current_failures,
                },
                current_failures,
            ),
            Err(_) => self.record(false, current_failures),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_resets_counter() {
        let policy = BiometricPolicy::new();
        assert_eq!(policy.record(true, 3), (BiometricOutcome::Success, 0));
    }

    #[test]
    fn single_denial_counts() {
        let policy = BiometricPolicy::new();
        let (outcome, count) = policy.record(false, 0);
        assert_eq!(outcome, BiometricOutcome::Denied { failures: 1 });
        assert_eq!(count, 1);
    }

    #[test]
    fn under_threshold_is_denied_not_locked() {
        let policy = BiometricPolicy::new();
        let (outcome, count) = policy.record(false, 3); // → 4, still under 5
        assert_eq!(outcome, BiometricOutcome::Denied { failures: 4 });
        assert_eq!(count, 4);
    }

    #[test]
    fn fifth_denial_locks_out() {
        let policy = BiometricPolicy::new();
        let (outcome, count) = policy.record(false, 4); // → 5 = lockout
        assert_eq!(outcome, BiometricOutcome::LockedOut);
        assert_eq!(count, 5);
    }

    #[test]
    fn classifies_engine_results() {
        let policy = BiometricPolicy::new();
        // Ok → success, resets
        assert_eq!(policy.classify(Ok(()), 4), (BiometricOutcome::Success, 0));
        // PermissionDenied → counts as a failure
        assert_eq!(
            policy.classify(Err(AuthError::PermissionDenied), 4),
            (BiometricOutcome::LockedOut, 5)
        );
        // Other error (BiometricFailed) → counts as a failure
        assert_eq!(
            policy.classify(Err(AuthError::BiometricFailed("hw".into())), 0),
            (BiometricOutcome::Denied { failures: 1 }, 1)
        );
    }
}
