//! Builds the Gullbúr Enclave provider identity and EIP-6963
//! announce event for injection into dApp pages.

use ipc_protocol::eip6963::{Eip6963AnnounceEvent, Eip6963ProviderInfo};

/// The Gullbúr Enclave EIP-6963 provider identity.
pub struct GullburProvider {
    pub info: Eip6963ProviderInfo,
}

impl GullburProvider {
    pub fn new() -> Self {
        Self {
            info: Eip6963ProviderInfo::new(
                Self::generate_uuid(),
                "Gullbúr Enclave",
                "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMzIiIGhlaWdodD0iMzIiIHZpZXdCb3g9IjAgMCAzMiAzMiIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iMzIiIGhlaWdodD0iMzIiIHJ4PSI2IiBmaWxsPSIjMUExQjJFIi8+PHRleHQgeD0iMTYiIHk9IjIyIiBmb250LXNpemU9IjE4IiB0ZXh0LWFuY2hvcj0ibWlkZGxlIiBmaWxsPSJ3aGl0ZSIgZm9udC1mYW1pbHk9Im1vbm9zcGFjZSI+8J+SuzwvdGV4dD48L3N2Zz4=",
                "io.gullbur.wallet",
            ),
        }
    }

    /// Generate a v4 UUID from random bytes (no external deps).
    fn generate_uuid() -> String {
        let mut buf = [0u8; 16];
        getrandom_fill(&mut buf);

        // Set version (4) and variant (10xx)
        buf[6] = (buf[6] & 0x0f) | 0x40;
        buf[8] = (buf[8] & 0x3f) | 0x80;

        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            buf[0], buf[1], buf[2], buf[3],
            buf[4], buf[5],
            buf[6], buf[7],
            buf[8], buf[9],
            buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
        )
    }

    /// Build the EIP-6963 announce event payload.
    pub fn announce_event(&self) -> Eip6963AnnounceEvent {
        Eip6963AnnounceEvent {
            info: self.info.clone(),
        }
    }
}

impl Default for GullburProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Fill buffer with OS random bytes. Falls back to simple entropy if /dev/urandom unavailable.
fn getrandom_fill(buf: &mut [u8]) {
    #[cfg(unix)]
    {
        use std::fs::File;
        use std::io::Read;
        if let Ok(mut f) = File::open("/dev/urandom") {
            let _ = f.read_exact(buf);
            return;
        }
    }
    // Fallback: time-based pseudo-random
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    for (i, b) in buf.iter_mut().enumerate() {
        *b = nanos.wrapping_mul((i + 1) as u32) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_identity() {
        let provider = GullburProvider::new();
        assert_eq!(provider.info.name, "Gullbúr Enclave");
        assert_eq!(provider.info.rdns, "io.gullbur.wallet");
        assert!(!provider.info.uuid.is_empty());
        assert!(provider.info.icon.starts_with("data:image/svg"));
    }

    #[test]
    fn test_uuid_format() {
        let uuid = GullburProvider::generate_uuid();
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.chars().filter(|c| *c == '-').count(), 4);
        // Version nibble must be 4
        assert_eq!(&uuid[14..15], "4");
    }

    #[test]
    fn test_announce_event() {
        let provider = GullburProvider::new();
        let event = provider.announce_event();
        assert_eq!(event.info.name, "Gullbúr Enclave");
    }

    #[test]
    fn test_default_constructor() {
        let provider = GullburProvider::default();
        assert_eq!(provider.info.name, "Gullbúr Enclave");
    }
}
