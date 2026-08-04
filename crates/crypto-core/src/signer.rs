//! Signing primitives: ECDSA (k256), Schnorr (k256), and k256.
//! All implementations use pure-Rust k256 (no C secp256k1-sys dependency).

use crate::error::CryptoError;
use crate::hash;

/// Sign `message` with ECDSA over secp256k1 using a raw 32-byte secret key.
///
/// The message is hashed with SHA-256 before signing. Returns the DER-encoded
/// ECDSA signature bytes.
pub fn sign_ecdsa_secp256k1(message: &[u8], secret: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
    use k256::ecdsa::signature::Signer;
    let signing_key = k256::ecdsa::SigningKey::from_slice(secret)
        .map_err(|e| CryptoError::InvalidKey(e.to_string()))?;
    let msg_hash = hash::sha256(message);
    let sig: k256::ecdsa::Signature = signing_key.sign(&msg_hash);
    Ok(sig.to_vec())
}

/// Sign a pre-computed 32-byte message hash with ECDSA over secp256k1 (k256).
///
/// Takes a `k256::SecretKey` and returns DER-encoded signature bytes. The caller
/// is responsible for hashing the message beforehand.
pub fn sign_ecdsa_k256(
    message_hash: &[u8; 32],
    secret: &k256::SecretKey,
) -> Result<Vec<u8>, CryptoError> {
    use k256::ecdsa::signature::Signer;
    let signing_key = k256::ecdsa::SigningKey::from(secret);
    let sig: k256::ecdsa::Signature = signing_key.sign(message_hash);
    Ok(sig.to_vec())
}

/// Sign `message` with BIP-340 Schnorr over secp256k1 using a raw 32-byte secret key.
///
/// The message is hashed with SHA-256 before signing. Returns the 64-byte Schnorr
/// signature (32-byte `r` || 32-byte `s`).
pub fn sign_schnorr(message: &[u8], secret: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
    use k256::schnorr::signature::Signer;
    let signing_key = k256::schnorr::SigningKey::from_slice(secret)
        .map_err(|e| CryptoError::InvalidKey(e.to_string()))?;
    let msg_hash = hash::sha256(message);
    let sig: k256::schnorr::Signature = signing_key.sign(&msg_hash);
    Ok(sig.to_bytes().to_vec())
}

/// Verify an ECDSA signature over secp256k1 against a SEC1-encoded public key.
///
/// The message is hashed with SHA-256 before verification (matching
/// [`sign_ecdsa_secp256k1`]). Returns `Ok(true)` if the signature is valid,
/// `Ok(false)` if it is not, and `Err` on malformed input.
pub fn verify_ecdsa_secp256k1(
    message: &[u8],
    signature: &[u8],
    public_key: &[u8],
) -> Result<bool, CryptoError> {
    use k256::ecdsa::signature::Verifier;
    let verifying_key = k256::ecdsa::VerifyingKey::from_sec1_bytes(public_key)
        .map_err(|e| CryptoError::InvalidKey(e.to_string()))?;
    let sig = k256::ecdsa::Signature::from_slice(signature)
        .map_err(|e| CryptoError::SigningFailed(e.to_string()))?;
    let msg_hash = hash::sha256(message);
    Ok(verifying_key.verify(&msg_hash, &sig).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schnorr_sign_64_bytes() {
        let mut secret = [0u8; 32];
        secret[0] = 1;
        let sig = sign_schnorr(b"test", &secret).expect("test invariant");
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn ecdsa_roundtrip() {
        let mut secret = [0u8; 32];
        secret[0] = 42;
        let msg = b"hello world";
        let sig = sign_ecdsa_secp256k1(msg, &secret).expect("test invariant");
        // Derive public key from secret
        let signing_key = k256::ecdsa::SigningKey::from_slice(&secret).expect("test invariant");
        let verifying_key = k256::ecdsa::VerifyingKey::from(&signing_key);
        let pk_bytes = verifying_key.to_sec1_bytes();
        let verified = verify_ecdsa_secp256k1(msg, &sig, &pk_bytes).expect("test invariant");
        assert!(verified);
    }

    #[test]
    fn ecdsa_invalid_signature() {
        let mut secret = [0u8; 32];
        secret[0] = 1;
        let msg = b"test message";
        let sig = sign_ecdsa_secp256k1(msg, &secret).expect("test invariant");
        // Use a different key
        let mut wrong_secret = [0u8; 32];
        wrong_secret[0] = 99;
        let signing_key =
            k256::ecdsa::SigningKey::from_slice(&wrong_secret).expect("test invariant");
        let verifying_key = k256::ecdsa::VerifyingKey::from(&signing_key);
        let pk_bytes = verifying_key.to_sec1_bytes();
        let result = verify_ecdsa_secp256k1(msg, &sig, &pk_bytes).expect("test invariant");
        assert!(!result);
    }

    // ── Proptest: sign→verify roundtrip ────────────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_schnorr_sign_64_bytes(seed: [u8; 32], msg_len in 0usize..256) {
            let mut secret = seed;
            secret[0] |= 1; // ensure non-zero
            let msg: Vec<u8> = (0..msg_len).map(|i| i as u8).collect();
            let sig = sign_schnorr(&msg, &secret).expect("test invariant");
            assert_eq!(sig.len(), 64, "Schnorr signature must be 64 bytes");
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_ecdsa_sign_verify_roundtrip(secret_seed: [u8; 32], msg: [u8; 64]) {
            let mut secret = secret_seed;
            secret[0] |= 1;
            let sig = sign_ecdsa_secp256k1(&msg, &secret).expect("test invariant");
            let signing_key = k256::ecdsa::SigningKey::from_slice(&secret).expect("test invariant");
            let verifying_key = k256::ecdsa::VerifyingKey::from(&signing_key);
            let pk_bytes = verifying_key.to_sec1_bytes();
            assert!(verify_ecdsa_secp256k1(&msg, &sig, &pk_bytes).expect("test invariant"));
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_ecdsa_wrong_key_rejected(secret_a: [u8; 32], secret_b: [u8; 32], msg: [u8; 32]) {
            if secret_a != secret_b {
                let mut a = secret_a; a[0] |= 1;
                let mut b = secret_b; b[0] |= 1;
                let sig = sign_ecdsa_secp256k1(&msg, &a).expect("test invariant");
                let signing_key = k256::ecdsa::SigningKey::from_slice(&b).expect("test invariant");
                let verifying_key = k256::ecdsa::VerifyingKey::from(&signing_key);
                let pk_bytes = verifying_key.to_sec1_bytes();
                assert!(!verify_ecdsa_secp256k1(&msg, &sig, &pk_bytes).expect("test invariant"));
            }
        }
    }
}
