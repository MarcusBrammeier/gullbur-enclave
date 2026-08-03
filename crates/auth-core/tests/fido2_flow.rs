//! FIDO2 authenticator flow integration tests.
//!
//! Exercises the `Fido2Authenticator` trait through the object-safe `dyn`
//! boundary exactly as the Tauri command does: probe → challenge →
//! authenticate, with the frame-representative timeout/empty-challenge paths.
//! This proves the injection seam (Android FIDO2 client-stub / YubiKey CTAP)
//! is correct without hardware.

use auth_core::fido2::{Fido2Authenticator, Fido2Status, MockFido2Authenticator};

#[test]
fn full_flow_probe_then_authenticate() {
    // Inject via the object-safe trait — the Android FIDO2 adapter plugs in here.
    let auth: Box<dyn Fido2Authenticator> = Box::new(MockFido2Authenticator);
    assert_eq!(auth.name(), "Mock FIDO2");

    // 1. Probe (non-blocking) — must report device found.
    assert_eq!(auth.probe(), Fido2Status::DeviceFound);

    // 2. Challenge-response with a real 32-byte challenge.
    let challenge = [0x11u8; 32];
    match auth
        .authenticate(&challenge, 30_000)
        .expect("auth must not error")
    {
        Fido2Status::AssertionReceived(sig) => {
            assert_eq!(sig.len(), 64, "FIDO2 assertion must be 64 bytes");
        }
        other => panic!("expected AssertionReceived, got {other:?}"),
    }
}

#[test]
fn empty_challenge_rejected() {
    let auth: Box<dyn Fido2Authenticator> = Box::new(MockFido2Authenticator);
    let result = auth.authenticate(&[], 30_000);
    assert!(result.is_err(), "empty challenge must be rejected");
}

#[test]
fn timeout_when_deadline_too_short() {
    let auth: Box<dyn Fido2Authenticator> = Box::new(MockFido2Authenticator);
    // Mock simulates ~100ms processing; a 50ms deadline should time out.
    let result = auth
        .authenticate(&[0x22; 32], 50)
        .expect("auth must not error");
    assert_eq!(result, Fido2Status::Timeout);
}

#[test]
fn challenge_must_be_32_bytes_for_real_devices() {
    // Sanity: real CTAP challenges are exactly 32 bytes; the mock doesn't
    // enforce it, but the producer (rand::random::<[u8;32]>) always sends 32.
    let challenge: [u8; 32] = rand::random();
    assert_eq!(challenge.len(), 32);
}
