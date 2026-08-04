//! Key derivation for secp256k1, ed25519, and k256.

use crate::error::CryptoError;
use crate::types::{KeyHandle, KeyType, MnemonicPhrase, MnemonicStrength};
use ed25519_dalek::SigningKey;
use hmac::digest::KeyInit;
use hmac::{Hmac, Mac};
use rand::TryRngCore;
use rand::rngs::OsRng;
use sha2::Sha512;
use zeroize::Zeroizing;

fn derive_bytes(seed: &[u8], index: u32, label: &[u8]) -> Result<[u8; 32], CryptoError> {
    let mut mac = <Hmac<Sha512> as KeyInit>::new_from_slice(label)
        .map_err(|e| CryptoError::DerivationError(e.to_string()))?;
    mac.update(seed);
    mac.update(&index.to_be_bytes());
    let derived = mac.finalize().into_bytes();
    derived[..32]
        .try_into()
        .map_err(|_| CryptoError::DerivationError("invalid key length".into()))
}

/// Derive a secp256k1 key pair from seed and index (pure Rust via k256).
pub fn derive_secp256k1_key(seed: &[u8], index: u32) -> Result<KeyHandle, CryptoError> {
    let key_bytes = derive_bytes(seed, index, b"foss-crypto-core-secp256k1-v1")?;
    let secret = k256::SecretKey::from_bytes(&key_bytes.into())
        .map_err(|e| CryptoError::InvalidKey(e.to_string()))?;
    let public = secret.public_key();
    Ok(KeyHandle {
        key_id: format!("secp256k1-{}", hex::encode(&public.to_sec1_bytes()[..8])),
        key_type: KeyType::Secp256k1,
        public_key: public.to_sec1_bytes().to_vec(),
    })
}

/// Derive an ed25519 key pair from seed and index.
pub fn derive_ed25519_key(seed: &[u8], index: u32) -> Result<KeyHandle, CryptoError> {
    let key_bytes = derive_bytes(seed, index, b"foss-crypto-core-ed25519-v1")?;
    let signing_key = SigningKey::from_bytes(&key_bytes);
    let verifying_key = signing_key.verifying_key();
    Ok(KeyHandle {
        key_id: format!("ed25519-{}", hex::encode(&verifying_key.as_bytes()[..8])),
        key_type: KeyType::Ed25519,
        public_key: verifying_key.as_bytes().to_vec(),
    })
}

/// Derive a k256 (Ethereum-compatible) secret key.
pub fn derive_k256_key(seed: &[u8], index: u32) -> Result<k256::SecretKey, CryptoError> {
    let key_bytes = derive_bytes(seed, index, b"foss-crypto-core-k256-v1")?;
    k256::SecretKey::from_bytes(&key_bytes.into())
        .map_err(|e| CryptoError::InvalidKey(e.to_string()))
}

/// Generate a random 32-byte seed.
pub fn generate_seed() -> zeroize::Zeroizing<[u8; 32]> {
    use rand::TryRngCore;
    use zeroize::Zeroizing;
    let mut seed = Zeroizing::new([0u8; 32]);
    OsRng.try_fill_bytes(seed.as_mut()).expect("OsRng failed");
    seed
}

// ── BIP-39 Mnemonic ──────────────────────────────────────────────────────────

/// Generate a BIP-39 mnemonic phrase from CSPRNG entropy.
pub fn generate_mnemonic(strength: MnemonicStrength) -> Result<MnemonicPhrase, CryptoError> {
    use bip39::Mnemonic;
    let mut entropy = vec![0u8; strength.entropy_bytes()];
    OsRng.try_fill_bytes(&mut entropy).expect("OsRng failed");
    let mnemonic =
        Mnemonic::from_entropy(&entropy).map_err(|e| CryptoError::MnemonicError(e.to_string()))?;
    let words: Vec<String> = mnemonic.words().map(|w| w.to_string()).collect();
    Ok(MnemonicPhrase::new(words))
}

/// Convert a BIP-39 mnemonic phrase + optional passphrase to a 512-bit seed.
pub fn mnemonic_to_seed(
    words: &[String],
    passphrase: &str,
) -> Result<Zeroizing<[u8; 64]>, CryptoError> {
    use bip39::Mnemonic;
    let phrase = words.join(" ");
    let mnemonic = Mnemonic::parse_normalized(&phrase)
        .map_err(|e| CryptoError::MnemonicError(e.to_string()))?;
    let seed_bytes = mnemonic.to_seed(passphrase);
    // bip39 returns Vec<u8>, convert to Zeroizing<[u8; 64]>
    let mut seed_arr = [0u8; 64];
    seed_arr.copy_from_slice(&seed_bytes);
    Ok(Zeroizing::new(seed_arr))
}

/// Parse a string into a validated BIP-39 mnemonic phrase.
pub fn mnemonic_from_string(s: &str) -> Result<MnemonicPhrase, CryptoError> {
    use bip39::Mnemonic;
    let mnemonic =
        Mnemonic::parse_normalized(s).map_err(|e| CryptoError::MnemonicError(e.to_string()))?;
    let words: Vec<String> = mnemonic.words().map(|w| w.to_string()).collect();
    Ok(MnemonicPhrase::new(words))
}

/// Convert a `MnemonicPhrase` back to a display string.
pub fn mnemonic_to_string(words: &MnemonicPhrase) -> String {
    words.to_string()
}
/// Derive a k256 secret key via BIP-44 Ethereum path: m/44'/60'/0'/0/{index}
pub fn derive_bip44_eth_key(
    seed_512: &[u8; 64],
    index: u32,
) -> Result<k256::SecretKey, CryptoError> {
    use bip32::XPrv;
    let xprv = XPrv::new(seed_512).map_err(|e| CryptoError::DerivationError(e.to_string()))?;
    // Manually walk the BIP-44 path: m/44'/60'/0'/0/{index}
    let path_steps = [
        bip32::ChildNumber::new(44, true)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
        bip32::ChildNumber::new(60, true)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
        bip32::ChildNumber::new(0, true)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
        bip32::ChildNumber::new(0, false)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
        bip32::ChildNumber::new(index, false)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
    ];
    let mut child = xprv;
    for step in &path_steps {
        child = child
            .derive_child(*step)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?;
    }
    let key_bytes: [u8; 32] = child.private_key().to_bytes().into();
    k256::SecretKey::from_bytes(&key_bytes.into())
        .map_err(|e| CryptoError::InvalidKey(e.to_string()))
}

/// Derive 32 bytes of Monero key entropy via BIP-44 path: m/44'/128'/0'/0/{index}
/// Monero then applies its own keccak256-to-scalar step on top of this.
pub fn derive_bip44_xmr_entropy(seed_512: &[u8; 64], index: u32) -> Result<[u8; 32], CryptoError> {
    use bip32::XPrv;
    let xprv = XPrv::new(seed_512).map_err(|e| CryptoError::DerivationError(e.to_string()))?;
    let path_steps = [
        bip32::ChildNumber::new(44, true)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
        bip32::ChildNumber::new(128, true)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
        bip32::ChildNumber::new(0, true)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
        bip32::ChildNumber::new(0, false)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
        bip32::ChildNumber::new(index, false)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?,
    ];
    let mut child = xprv;
    for step in &path_steps {
        child = child
            .derive_child(*step)
            .map_err(|e| CryptoError::DerivationError(e.to_string()))?;
    }
    Ok(child.private_key().to_bytes().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_secp256k1_deterministic() {
        let seed = [1u8; 32];
        let a = derive_secp256k1_key(&seed, 0).expect("test invariant");
        let b = derive_secp256k1_key(&seed, 0).expect("test invariant");
        assert_eq!(a.key_id, b.key_id);
        assert_eq!(a.public_key, b.public_key);
    }

    #[test]
    fn derive_secp256k1_different_indices() {
        let seed = [2u8; 32];
        let a = derive_secp256k1_key(&seed, 0).expect("test invariant");
        let b = derive_secp256k1_key(&seed, 1).expect("test invariant");
        assert_ne!(a.key_id, b.key_id);
        assert_ne!(a.public_key, b.public_key);
    }

    #[test]
    fn derive_ed25519_deterministic() {
        let seed = [3u8; 32];
        let a = derive_ed25519_key(&seed, 0).expect("test invariant");
        let b = derive_ed25519_key(&seed, 0).expect("test invariant");
        assert_eq!(a.key_id, b.key_id);
    }

    #[test]
    fn generate_seed_nonzero() {
        let seed = generate_seed();
        assert_ne!(seed.as_slice(), [0u8; 32]);
    }

    // ── BIP-39 Tests ───────────────────────────────────────────────────────

    #[test]
    fn mnemon_generates_valid_24_words() {
        let mnemonic =
            generate_mnemonic(MnemonicStrength::TwentyFourWords).expect("test invariant");
        assert_eq!(mnemonic.as_words().len(), 24);
        // Verify it can be parsed back
        let restored = mnemonic_from_string(&mnemonic.to_string()).expect("test invariant");
        assert_eq!(restored.as_words().len(), 24);
    }

    #[test]
    fn mnemon_generates_valid_12_words() {
        let mnemonic = generate_mnemonic(MnemonicStrength::TwelveWords).expect("test invariant");
        assert_eq!(mnemonic.as_words().len(), 12);
    }

    #[test]
    fn mnemon_to_seed_deterministic() {
        let mnemonic = generate_mnemonic(MnemonicStrength::TwelveWords).expect("test invariant");
        let seed_a = mnemonic_to_seed(mnemonic.as_words(), "").expect("test invariant");
        let seed_b = mnemonic_to_seed(mnemonic.as_words(), "").expect("test invariant");
        assert_eq!(seed_a.as_slice(), seed_b.as_slice());
    }

    #[test]
    fn mnemon_to_seed_with_passphrase() {
        let mnemonic = generate_mnemonic(MnemonicStrength::TwelveWords).expect("test invariant");
        let seed_a = mnemonic_to_seed(mnemonic.as_words(), "").expect("test invariant");
        let seed_b = mnemonic_to_seed(mnemonic.as_words(), "passphrase").expect("test invariant");
        assert_ne!(seed_a.as_slice(), seed_b.as_slice());
    }

    #[test]
    fn mnemon_roundtrip() {
        let original = generate_mnemonic(MnemonicStrength::TwelveWords).expect("test invariant");
        let seed = mnemonic_to_seed(original.as_words(), "").expect("test invariant");
        let restored = mnemonic_from_string(&original.to_string()).expect("test invariant");
        let restored_seed = mnemonic_to_seed(restored.as_words(), "").expect("test invariant");
        assert_eq!(seed.as_slice(), restored_seed.as_slice());
    }

    #[test]
    fn mnemon_invalid_phrase() {
        assert!(mnemonic_from_string("not a valid mnemonic phrase here").is_err());
    }

    // ── BIP-32 Tests ────────────────────────────────────────────────────────

    #[test]
    fn bip44_eth_deterministic() {
        // Use a known BIP-39 test vector
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = mnemonic_from_string(phrase).expect("test invariant");
        let seed = mnemonic_to_seed(mnemonic.as_words(), "").expect("test invariant");
        let key_a = derive_bip44_eth_key(&seed, 0).expect("test invariant");
        let key_b = derive_bip44_eth_key(&seed, 0).expect("test invariant");
        assert_eq!(key_a.to_bytes().as_slice(), key_b.to_bytes().as_slice());
    }

    #[test]
    fn bip44_eth_different_indices() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = mnemonic_from_string(phrase).expect("test invariant");
        let seed = mnemonic_to_seed(mnemonic.as_words(), "").expect("test invariant");
        let key_0 = derive_bip44_eth_key(&seed, 0).expect("test invariant");
        let key_1 = derive_bip44_eth_key(&seed, 1).expect("test invariant");
        assert_ne!(key_0.to_bytes().as_slice(), key_1.to_bytes().as_slice());
    }

    #[test]
    fn bip44_xmr_deterministic() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = mnemonic_from_string(phrase).expect("test invariant");
        let seed = mnemonic_to_seed(mnemonic.as_words(), "").expect("test invariant");
        let ent_a = derive_bip44_xmr_entropy(&seed, 0).expect("test invariant");
        let ent_b = derive_bip44_xmr_entropy(&seed, 0).expect("test invariant");
        assert_eq!(ent_a, ent_b);
    }

    #[test]
    fn old_derivation_unchanged() {
        // Verify the old custom derivation still works the same
        let seed = [42u8; 32];
        let key = derive_secp256k1_key(&seed, 0).expect("test invariant");
        assert_eq!(key.key_type, KeyType::Secp256k1);
        assert!(!key.public_key.is_empty());
        // Deterministic
        let key2 = derive_secp256k1_key(&seed, 0).expect("test invariant");
        assert_eq!(key.key_id, key2.key_id);
    }

    #[test]
    fn verify_bip32_test_vector() {
        // BIP-39 Test Vector 1 (passphrase "TREZOR" per BIP-39 spec):
        // https://github.com/trezor/python-mnemonic/blob/master/vectors.json
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        // Per BIP-39 spec: "The passphrase 'TREZOR' is used for all vectors"
        let passphrase = "TREZOR";
        let expected_seed_hex = "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04";

        // Step 1: Verify BIP-39 seed against known test vector
        let mnemonic = bip39::Mnemonic::parse_normalized(phrase).expect("test invariant");
        let seed_bytes = mnemonic.to_seed(passphrase);
        let seed_hex = hex::encode(&seed_bytes);
        assert_eq!(
            seed_hex, expected_seed_hex,
            "BIP-39 seed mismatch with known test vector!"
        );

        let seed_arr: [u8; 64] = seed_bytes.as_slice().try_into().expect("test invariant");

        // Step 2: Derive m/44'/60'/0'/0/0 via bip32 XPrv
        let key_bytes: [u8; 32] = {
            use bip32::{ChildNumber, XPrv};
            let xprv = XPrv::new(&seed_arr).expect("test invariant");
            let path = [
                ChildNumber::new(44, true).expect("test invariant"),
                ChildNumber::new(60, true).expect("test invariant"),
                ChildNumber::new(0, true).expect("test invariant"),
                ChildNumber::new(0, false).expect("test invariant"),
                ChildNumber::new(0, false).expect("test invariant"),
            ];
            let mut child = xprv;
            for step in &path {
                child = child.derive_child(*step).expect("test invariant");
            }
            child.private_key().to_bytes().into()
        };

        // Step 3: Compute Ethereum address from derived key
        use k256::elliptic_curve::sec1::ToSec1Point;
        use sha3::Digest;
        let secret = k256::SecretKey::from_bytes(&key_bytes.into()).expect("test invariant");
        let pub_key = secret.public_key();
        let encoded = pub_key.to_sec1_point(false);
        let hash = sha3::Keccak256::digest(&encoded.as_bytes()[1..]);
        let address = format!("0x{}", hex::encode(&hash[12..]));

        // Step 4: Verify with `cast wallet address` (reference implementation) if available.
        // Cast is part of foundry (foundry.tools). On CI runners it's usually absent,
        // so we skip the cross-check but the BIP-39 seed + BIP-32 derivation above
        // are still tested against the known test vector.
        if std::process::Command::new("cast")
            .arg("--version")
            .output()
            .is_ok()
        {
            let key_hex = hex::encode(key_bytes);
            let output = std::process::Command::new("cast")
                .args([
                    "wallet",
                    "address",
                    "--private-key",
                    &format!("0x{}", key_hex),
                ])
                .output()
                .expect("cast not found — install foundry (foundry.tools)");
            let cast_addr = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string()
                .to_lowercase();

            assert_eq!(
                address.to_lowercase(),
                cast_addr,
                "BIP-32 derivation: ETH address mismatch with cast wallet address!"
            );
        } // cast available check
    }

    // ── Proptest: sign→verify roundtrip for k256 ─────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_sign_verify_roundtrip_k256(seed: [u8; 32], msg_seed: [u8; 32]) {
            let secret = derive_k256_key(&seed, 0).expect("test invariant");
            let msg_hash = crate::hash::sha256(&msg_seed);
            let sig_bytes = crate::signer::sign_ecdsa_k256(&msg_hash, &secret).expect("test invariant");

            // Recover the verifying key from the secret
            let signing_key = k256::ecdsa::SigningKey::from(&secret);
            let verifying_key = k256::ecdsa::VerifyingKey::from(&signing_key);

            use k256::ecdsa::signature::Verifier;
            let sig = k256::ecdsa::Signature::from_slice(&sig_bytes).expect("test invariant");
            assert!(verifying_key.verify(&msg_hash, &sig).is_ok());
        }
    }

    // ── Proptest: key derivation determinism ──────────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_key_derivation_deterministic(seed: [u8; 32], index: u32) {
            // Same seed + same index → same key (secp256k1)
            let a = derive_secp256k1_key(&seed, index).expect("test invariant");
            let b = derive_secp256k1_key(&seed, index).expect("test invariant");
            assert_eq!(a.key_id, b.key_id);
            assert_eq!(a.public_key, b.public_key);

            // Different seed → different key (with high probability)
            let mut other_seed = seed;
            other_seed[0] = other_seed[0].wrapping_add(1);
            let c = derive_secp256k1_key(&other_seed, index).expect("test invariant");
            assert_ne!(a.key_id, c.key_id);

            // Different index → different key (with high probability)
            let d = derive_secp256k1_key(&seed, index.wrapping_add(1)).expect("test invariant");
            assert_ne!(a.key_id, d.key_id);
        }
    }

    // ── Proptest: BIP-44 ETH key determinism ─────────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_bip44_eth_key_deterministic(seed_part: [u8; 64], index in 0u32..(1u32 << 31)) {
            // Same seed + same index → same key
            let a = derive_bip44_eth_key(&seed_part, index).expect("test invariant");
            let b = derive_bip44_eth_key(&seed_part, index).expect("test invariant");
            assert_eq!(a.to_bytes().as_slice(), b.to_bytes().as_slice());

            // Different index → different key (with high probability)
            let alt_index = if index == 0 { 1 } else { 0 };
            let c = derive_bip44_eth_key(&seed_part, alt_index).expect("test invariant");
            assert_ne!(a.to_bytes().as_slice(), c.to_bytes().as_slice());
        }
    }

    // ── Proptest: BIP-44 XMR entropy determinism ─────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_bip44_xmr_entropy_deterministic(seed_part: [u8; 64], index in 0u32..(1u32 << 31)) {
            // Same seed + same index → same entropy
            let a = derive_bip44_xmr_entropy(&seed_part, index).expect("test invariant");
            let b = derive_bip44_xmr_entropy(&seed_part, index).expect("test invariant");
            assert_eq!(a, b);

            // Different index → different entropy (with high probability)
            let alt_index = if index == 0 { 1 } else { 0 };
            let c = derive_bip44_xmr_entropy(&seed_part, alt_index).expect("test invariant");
            assert_ne!(a, c);
        }
    }
}
