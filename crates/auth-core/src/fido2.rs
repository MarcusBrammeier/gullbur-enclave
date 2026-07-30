//! FIDO2 / CTAP HID authenticator driver.
//!
//! This module provides the [`Fido2Authenticator`] trait for hardware FIDO2
//! authenticators (YubiKey, etc.) communicating over CTAP HID, plus a
//! [`MockFido2Authenticator`] for testing and development.
//!
//! All HID I/O is **blocking** — callers MUST use
//! [`tokio::task::spawn_blocking`] to avoid stalling the async runtime.

use crate::error::AuthError;

/// Status of a FIDO2 hardware authenticator operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fido2Status {
    /// No FIDO2 device found on the system.
    DeviceNotFound,
    /// A FIDO2 device is present and ready.
    DeviceFound,
    /// A valid cryptographic assertion was received from the device.
    AssertionReceived(Vec<u8>),
    /// The operation timed out waiting for user interaction.
    Timeout,
}

/// Hardware FIDO2 authenticator (YubiKey, etc.).
///
/// Operations are blocking HID I/O — callers MUST use
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime.
pub trait Fido2Authenticator: Send + Sync {
    /// Human-readable name of the authenticator (e.g. "YubiKey 5").
    fn name(&self) -> &'static str;

    /// Check if a FIDO2 device is currently present.
    ///
    /// This is a quick, non-blocking probe of the HID device list.
    fn probe(&self) -> Fido2Status;

    /// Perform a full challenge-response authentication flow.
    ///
    /// - `challenge`: random 32-byte challenge to sign
    /// - `timeout_ms`: max milliseconds to wait for user touch
    ///
    /// Returns `Fido2Status::AssertionReceived(signature)` on success,
    /// or an error variant on failure.
    ///
    /// NOTE: This method may block HID I/O. Callers MUST use
    /// `tokio::task::spawn_blocking` to run it.
    fn authenticate(
        &self,
        challenge: &[u8],
        timeout_ms: u32,
    ) -> Result<Fido2Status, AuthError>;
}

// ---------------------------------------------------------------------------
// Mock implementation
// ---------------------------------------------------------------------------

/// A mock FIDO2 authenticator for testing and development.
///
/// Always reports `DeviceFound` on [`probe`](Fido2Authenticator::probe)
/// and returns a deterministic 64-byte signature from
/// [`authenticate`](Fido2Authenticator::authenticate) for any non-empty
/// challenge. If the simulated delay exceeds `timeout_ms` the call returns
/// [`Fido2Status::Timeout`].
pub struct MockFido2Authenticator;

impl Fido2Authenticator for MockFido2Authenticator {
    fn name(&self) -> &'static str {
        "Mock FIDO2"
    }

    fn probe(&self) -> Fido2Status {
        Fido2Status::DeviceFound
    }

    fn authenticate(
        &self,
        challenge: &[u8],
        timeout_ms: u32,
    ) -> Result<Fido2Status, AuthError> {
        // Reject empty challenges as a sanity check.
        if challenge.is_empty() {
            return Err(AuthError::Internal(
                "FIDO2: challenge must not be empty".into(),
            ));
        }

        // Simulate a constant processing delay (≈ 100 ms).
        let simulated_delay_ms: u32 = 100;

        if simulated_delay_ms > timeout_ms {
            return Ok(Fido2Status::Timeout);
        }

        // We don't actually sleep here — this is a mock. A real CTAP HID
        // driver would block on HID reads during the touch window.
        let _ = (challenge, timeout_ms, simulated_delay_ms);

        // Produce a deterministic 64-byte signature.
        Ok(Fido2Status::AssertionReceived(vec![0u8; 64]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_name() {
        let m = MockFido2Authenticator;
        assert_eq!(m.name(), "Mock FIDO2");
    }

    #[test]
    fn mock_probe() {
        let m = MockFido2Authenticator;
        assert_eq!(m.probe(), Fido2Status::DeviceFound);
    }

    #[test]
    fn mock_authenticate_success() {
        let m = MockFido2Authenticator;
        let challenge = [1u8; 32];
        let result = m.authenticate(&challenge, 5_000).expect("test invariant");
        assert!(matches!(result, Fido2Status::AssertionReceived(ref sig) if sig.len() == 64));
    }

    #[test]
    fn mock_authenticate_empty_challenge() {
        let m = MockFido2Authenticator;
        let result = m.authenticate(&[], 5_000);
        assert!(result.is_err());
    }

    #[test]
    fn mock_authenticate_timeout() {
        let m = MockFido2Authenticator;
        // timeout_ms lower than the simulated 100 ms delay → Timeout
        let result = m.authenticate(&[1u8; 32], 50).expect("test invariant");
        assert_eq!(result, Fido2Status::Timeout);
    }

    #[test]
    fn fido2_status_debug_clone_eq() {
        let s = Fido2Status::AssertionReceived(vec![1, 2, 3]);
        let s2 = s.clone();
        assert_eq!(s, s2);
        assert!(!format!("{:?}", s).is_empty());
    }

    #[test]
    fn fido2_authenticator_is_object_safe() {
        // Compile-time check: the trait can be used as a dyn object.
        fn _take(_: &dyn Fido2Authenticator) {}
    }
}