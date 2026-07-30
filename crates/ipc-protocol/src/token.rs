//! One-time auth token for IPC handshake.
//!
//! Generates a 32-byte random token, returns it as a hex string for the
//! file content and wraps it for programmatic validation.

use rand::RngCore;

/// A one-time authentication token used for IPC handshake.
///
/// The token is 32 cryptographically random bytes. The file content format
/// is the hex encoding of those bytes. Validation checks that a provided
/// hex string matches the expected token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthToken {
    /// The raw 32-byte token.
    bytes: [u8; 32],
}

impl AuthToken {
    /// Generate a new random auth token.
    ///
    /// Returns the `AuthToken` and the hex-encoded string that should be
    /// written to the token file.
    pub fn generate() -> (Self, String) {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let hex = hex::encode(&bytes);
        (Self { bytes }, hex)
    }

    /// Create an `AuthToken` from existing bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    /// Validate a hex-encoded token string against this token.
    ///
    /// The `content` should be the content of the token file — a 64-character
    /// hex string.
    pub fn validate(&self, content: &str) -> bool {
        let content = content.trim();
        if content.len() != 64 {
            return false;
        }
        let Ok(decoded) = hex::decode(content) else {
            return false;
        };
        decoded == self.bytes
    }

    /// Return the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

/// Convenience: hex encoding module (no extra dep needed — implemented inline).
mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }

    pub fn decode(hex: &str) -> Result<Vec<u8>, ()> {
        if !hex.len().is_multiple_of(2) {
            return Err(());
        }
        (0..hex.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_validate() {
        let (token, file_content) = AuthToken::generate();
        assert_eq!(file_content.len(), 64);
        assert!(token.validate(&file_content));
    }

    #[test]
    fn test_validate_wrong_token() {
        let (token, _) = AuthToken::generate();
        // 64 hex chars (32 bytes), but different from the generated one
        let wrong = "a".repeat(64);
        assert!(!token.validate(&wrong));
    }

    #[test]
    fn test_validate_malformed() {
        let (token, _) = AuthToken::generate();
        assert!(!token.validate("xyz"));
        assert!(!token.validate(""));
        assert!(!token.validate(&"gg".repeat(32))); // 'g' is not a valid hex char
    }

    #[test]
    fn test_hex_roundtrip() {
        let original = [0xde, 0xad, 0xbe, 0xef];
        let encoded = hex::encode(&original);
        assert_eq!(encoded, "deadbeef");
        let decoded = hex::decode(&encoded).expect("test invariant");
        assert_eq!(decoded, original);
    }
}