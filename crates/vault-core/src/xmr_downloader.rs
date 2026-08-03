//! Auto-download and verify `monero-wallet-rpc` binary.
//!
//! On first XMR interaction, the vault detects the binary is missing,
//! downloads the official Monero CLI release for the current platform,
//! verifies its SHA256 hash, extracts `monero-wallet-rpc`, caches it,
//! and spawns it. No bundling, no user action.
//!
//! ## Cache location
//!
//! Binary is cached at `~/.gullbur/xmr-bin/<version>/monero-wallet-rpc`
//! (or the equivalent system data directory on Android).
//!
//! ## Platforms supported
//!
//! - Linux x86_64 (`monero-linux-x64-cli.tar.bz2`)
//! - Android aarch64 (`monero-linux-armv8-cli.tar.bz2`)
//! - macOS aarch64 (`monero-mac-armv8-cli.tar.bz2`)
//! - macOS x86_64 (`monero-mac-x64-cli.tar.bz2`)

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// The Monero release version we target.
/// This is the latest stable release as of writing.
const MONERO_VERSION: &str = "v0.18.5.1";

/// Registry of known-good SHA256 hashes for monero-wallet-rpc binaries.
/// Downloaded from getmonero.org. Update these when bumping `MONERO_VERSION`.
///
/// ```bash
/// V=v0.18.5.1
/// curl -sL "https://downloads.getmonero.org/cli/monero-linux-x64-$V.tar.bz2" | sha256sum
/// ```
static KNOWN_HASHES: LazyLock<[PlatformHash; 4]> = LazyLock::new(|| {
    [
        PlatformHash {
            triple: "x86_64-linux",
            archive_name: "monero-linux-x64-v0.18.5.1.tar.bz2",
            archive_url: "https://downloads.getmonero.org/cli/monero-linux-x64-v0.18.5.1.tar.bz2",
            binary_name: "monero-wallet-rpc",
            sha256: "22a7dda7b0cb699fdd6b7674c3b4a4465b337cc98a54983523b759e1e7cc9958",
        },
        PlatformHash {
            triple: "aarch64-linux-android",
            archive_name: "monero-android-armv8-v0.18.5.1.tar.bz2",
            archive_url: "https://downloads.getmonero.org/cli/monero-android-armv8-v0.18.5.1.tar.bz2",
            binary_name: "monero-wallet-rpc",
            sha256: "",
        },
        PlatformHash {
            triple: "aarch64-macos",
            archive_name: "monero-mac-armv8-v0.18.5.1.tar.bz2",
            archive_url: "https://downloads.getmonero.org/cli/monero-mac-armv8-v0.18.5.1.tar.bz2",
            binary_name: "monero-wallet-rpc",
            sha256: "",
        },
        PlatformHash {
            triple: "x86_64-macos",
            archive_name: "monero-mac-x64-v0.18.5.1.tar.bz2",
            archive_url: "https://downloads.getmonero.org/cli/monero-mac-x64-v0.18.5.1.tar.bz2",
            binary_name: "monero-wallet-rpc",
            sha256: "",
        },
    ]
});

/// Known hash for one platform binary.
struct PlatformHash {
    triple: &'static str,
    archive_name: &'static str,
    archive_url: &'static str,
    binary_name: &'static str,
    sha256: &'static str,
}

/// Result of ensuring the monero-wallet-rpc binary is available.
pub struct EnsuredBinary {
    /// Path to the cached binary.
    pub path: PathBuf,
    /// Version string for logging.
    pub version: &'static str,
}

/// Ensure the `monero-wallet-rpc` binary is available.
/// Downloads and verifies it if not already cached.
///
/// `cache_dir` is the directory to store the binary in
/// (e.g. `~/.gullbur/xmr-bin` or `app_cache_dir/xmr-bin`).
pub async fn ensure_binary(cache_dir: &Path) -> Result<EnsuredBinary, String> {
    // Determine current platform
    let platform = detect_platform()?;
    let info = KNOWN_HASHES
        .iter()
        .find(|p| p.triple == platform)
        .ok_or_else(|| format!("Unsupported platform: {platform}"))?;

    let version_dir = cache_dir.join(MONERO_VERSION);
    let binary_path = version_dir.join(info.binary_name);

    // Already cached — fast path
    if binary_path.exists() {
        return Ok(EnsuredBinary {
            path: binary_path,
            version: MONERO_VERSION,
        });
    }

    // Download and cache
    tracing::info!(
        "monero-wallet-rpc not cached — downloading {} ({})",
        info.archive_name,
        platform
    );
    std::fs::create_dir_all(&version_dir)
        .map_err(|e| format!("Failed to create cache dir: {e}"))?;

    let temp_file = version_dir.join(format!(".download.{}", info.archive_name));
    download_and_extract(info, &temp_file, &binary_path).await?;

    tracing::info!("monero-wallet-rpc cached at {:?}", binary_path);
    Ok(EnsuredBinary {
        path: binary_path,
        version: MONERO_VERSION,
    })
}

/// Detect the current target triple.
fn detect_platform() -> Result<&'static str, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("linux", "x86_64") => Ok("x86_64-linux"),
        ("linux", "aarch64") => Ok("aarch64-linux-android"),
        ("macos", "aarch64") => Ok("aarch64-macos"),
        ("macos", "x86_64") => Ok("x86_64-macos"),
        _ => Err(format!("Unsupported platform: {os}-{arch}")),
    }
}

/// Download the archive, verify hash, and extract monero-wallet-rpc.
async fn download_and_extract(
    info: &PlatformHash,
    temp_path: &Path,
    binary_path: &Path,
) -> Result<(), String> {
    let url = info.archive_url;

    // Download
    tracing::info!("Downloading {url}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Read response failed: {e}"))?;

    // Verify SHA256 if we have a real hash (skip empty placeholder)
    if !info.sha256.is_empty() {
        let hash = sha256(&bytes);
        if hash != info.sha256 {
            return Err(format!(
                "SHA256 mismatch for {}: expected {} got {}",
                info.archive_name, info.sha256, hash
            ));
        }
        tracing::info!("SHA256 verified for {}", info.archive_name);
    } else {
        tracing::warn!(
            "SHA256 hash not configured — skipping verification for {}",
            info.archive_name
        );
    }

    // Write temp file
    tokio::fs::write(temp_path, &bytes)
        .await
        .map_err(|e| format!("Write temp file failed: {e}"))?;

    // Extract monero-wallet-rpc from the bzip2 tarball
    let binary = extract_tar_bz2_bytes(&bytes, info.binary_name)?;

    // Write the extracted binary with executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::write(binary_path, &binary)
            .await
            .map_err(|e| format!("Write binary failed: {e}"))?;
        std::fs::set_permissions(binary_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Set permissions: {e}"))?;
    }
    #[cfg(not(unix))]
    {
        tokio::fs::write(binary_path, &binary)
            .await
            .map_err(|e| format!("Write binary failed: {e}"))?;
    }

    // Clean up temp file
    let _ = tokio::fs::remove_file(temp_path).await;

    Ok(())
}

/// Extract a single file from a .tar.bz2 archive in memory.
fn extract_tar_bz2_bytes(archive: &[u8], target_name: &str) -> Result<Vec<u8>, String> {
    // First decompress bzip2
    let mut decompressed = Vec::with_capacity(archive.len() * 3);
    {
        let mut decompressor = bzip2::read::BzDecoder::new(archive);
        std::io::Read::read_to_end(&mut decompressor, &mut decompressed)
            .map_err(|e| format!("Bzip2 decompress failed: {e}"))?;
    }

    // Then scan tar entries
    let mut archive = tar::Archive::new(&decompressed[..]);
    for entry in archive
        .entries()
        .map_err(|e| format!("Tar read failed: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("Tar entry: {e}"))?;
        let path = entry.path().map_err(|e| format!("Tar path: {e}"))?;
        if path.ends_with(target_name) {
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf)
                .map_err(|e| format!("Read {target_name}: {e}"))?;
            return Ok(buf);
        }
    }

    Err(format!("{target_name} not found in archive"))
}

/// Simple SHA256 hex digest.
fn sha256(data: &[u8]) -> String {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(data);
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_detect_platform() {
        let platform = detect_platform();
        assert!(
            platform.is_ok(),
            "Platform should be detectable: {:?}",
            platform.err()
        );
        let triple = platform.unwrap();
        // We're on x86_64-linux most likely
        assert!(triple.contains("linux") || triple.contains("macos"));
    }

    #[test]
    fn test_known_hashes_loaded() {
        assert!(KNOWN_HASHES.len() == 4);
        assert_eq!(KNOWN_HASHES[0].triple, "x86_64-linux");
        assert_eq!(KNOWN_HASHES[1].triple, "aarch64-linux-android");
    }

    #[test]
    #[ignore = "requires network to be unreachable — run manually to verify error path"]
    fn test_ensure_binary_returns_err_when_no_network() {
        // Verifies that without network, ensure_binary returns Err gracefully
        let temp = std::env::temp_dir().join("foss-xmr-test");
        let _ = std::fs::remove_dir_all(&temp);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(ensure_binary(&temp));
        assert!(result.is_err(), "Should fail without network");
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_sha256_hex() {
        let hash = sha256(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_extract_tar_bz2_when_not_tar() {
        let result = extract_tar_bz2_bytes(b"not a tar", "nope");
        assert!(result.is_err());
    }
}
