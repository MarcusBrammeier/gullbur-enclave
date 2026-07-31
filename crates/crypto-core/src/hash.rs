use ripemd::Ripemd160;
use sha2::{Digest, Sha256};
use sha3::Keccak256;

/// Compute SHA-256 hash of `data` and return the 32-byte digest.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Compute Keccak-256 hash of `data` and return the 32-byte digest.
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Compute RIPEMD-160 hash of `data` and return the 20-byte digest.
pub fn ripemd160(data: &[u8]) -> [u8; 20] {
    let mut hasher = Ripemd160::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Compute HASH160 — SHA-256 followed by RIPEMD-160.
/// Returns a 20-byte digest.
pub fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = sha256(data);
    ripemd160(&sha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_known() {
        let result = sha256(b"hello");
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_keccak256_empty() {
        let result = keccak256(b"");
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_hash160() {
        let result = hash160(b"hello");
        assert_eq!(result.len(), 20);
    }

    // ── Proptest: hash determinism ────────────────────────────────────

    proptest::proptest! {
        #[test]
        fn proptest_sha256_deterministic(data: Vec<u8>) {
            let a = sha256(&data);
            let b = sha256(&data);
            assert_eq!(a, b);
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_keccak256_deterministic(data: Vec<u8>) {
            let a = keccak256(&data);
            let b = keccak256(&data);
            assert_eq!(a, b);
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_hash160_20_bytes(data: Vec<u8>) {
            let result = hash160(&data);
            assert_eq!(result.len(), 20);
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_sha256_keccak256_diff(data: Vec<u8>) {
            if data.len() >= 1 {
                let s = sha256(&data);
                let k = keccak256(&data);
                assert_ne!(s, k, "SHA-256 and Keccak-256 must differ on non-empty input");
            }
        }
    }
}
