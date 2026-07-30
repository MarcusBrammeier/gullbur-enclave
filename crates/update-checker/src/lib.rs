//! GitHub release update checker.
//!
//! Queries the GitHub Releases API for the latest published release
//! and compares it against the local compile-time version string.
//!
//! # Usage
//!
//! ```rust,ignore
//! use update_checker::check_for_updates;
//!
//! let result = check_for_updates("gullbur/gullbur").await;
//! match result {
//!     Ok(Some(release)) => println!("New version: {}", release.tag_name),
//!     Ok(None) => println!("Up to date"),
//!     Err(e) => println!("Check failed: {e}"),
//! }
//! ```

use serde::Deserialize;
use thiserror::Error;

/// Errors from the update checker.
#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Failed to parse release response: {0}")]
    Parse(String),

    #[error("No releases found for {0}")]
    NoReleases(String),

    #[error("Rate limited by GitHub API")]
    RateLimited,
}

/// A single GitHub release (latest).
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    /// Tag name, e.g. "v0.1.0-beta"
    pub tag_name: String,
    /// Release name, e.g. "v0.1.0 Beta"
    pub name: Option<String>,
    /// Body / release notes (markdown)
    pub body: Option<String>,
    /// URL to the release page
    pub html_url: String,
    /// Whether this is a pre-release
    pub prerelease: bool,
    /// Published timestamp
    pub published_at: Option<String>,
    /// Download URLs for assets
    pub assets: Vec<ReleaseAsset>,
}

/// A downloadable asset attached to a release.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    /// Display name, e.g. "gullbur-x86_64.AppImage"
    pub name: String,
    /// Download URL
    pub browser_download_url: String,
    /// Size in bytes
    pub size: u64,
    /// Content type
    pub content_type: Option<String>,
}

/// The result of an update check.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// The latest release from GitHub
    pub release: Release,
    /// Whether the user is currently up to date
    pub up_to_date: bool,
    /// The parsed local version
    pub local_version: String,
}

/// Compile-time package version — set by Cargo.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check the GitHub releases API for a newer version.
///
/// `repo` should be in the format `"owner/repo"`, e.g. `"gullbur/gullbur"`.
///
/// Returns:
/// - `Ok(Some(UpdateInfo))` — a newer release exists or info on current release
/// - `Ok(None)` — no releases found
/// - `Err(UpdateError)` — the check failed
pub async fn check_for_updates(repo: &str) -> Result<Option<UpdateInfo>, UpdateError> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");

    let client = reqwest::Client::builder()
        .user_agent("gullbur/0.1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let response = client.get(&url).send().await?;

    match response.status().as_u16() {
        200 => {
            let release: Release = response.json().await.map_err(|e| {
                UpdateError::Parse(e.to_string())
            })?;

            let up_to_date = compare_versions(CURRENT_VERSION, &release.tag_name);

            Ok(Some(UpdateInfo {
                release,
                up_to_date,
                local_version: CURRENT_VERSION.to_string(),
            }))
        }
        404 => {
            // No releases yet — fall back to listing tags
            let tags_url = format!("https://api.github.com/repos/{repo}/releases?per_page=1");
            let resp = client.get(&tags_url).send().await?;
            if resp.status().as_u16() == 200 {
                let releases: Vec<Release> = resp.json().await.map_err(|e| {
                    UpdateError::Parse(e.to_string())
                })?;
                Ok(releases.into_iter().next().map(|release| {
                    let up_to_date = compare_versions(CURRENT_VERSION, &release.tag_name);
                    UpdateInfo {
                        release,
                        up_to_date,
                        local_version: CURRENT_VERSION.to_string(),
                    }
                }))
            } else {
                Err(UpdateError::NoReleases(repo.to_string()))
            }
        }
        403 | 429 => Err(UpdateError::RateLimited),
        status => {
            let body = response.text().await.unwrap_or_default();
            Err(UpdateError::Parse(format!(
                "Unexpected HTTP {status}: {body}"
            )))
        }
    }
}

/// Compare local version string against a release tag.
///
/// Tags can be `"v0.1.0"`, `"0.1.0-beta"`, etc. Strips leading 'v'.
/// Returns `true` if local version is >= release version (up to date).
fn compare_versions(local: &str, release_tag: &str) -> bool {
    let tag = release_tag.trim_start_matches('v');
    // Simple semantic version comparison: split on '.' and compare numerically
    let local_parts: Vec<u64> = local
        .split('.')
        .filter_map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
        .collect();
    let release_parts: Vec<u64> = tag
        .split('.')
        .filter_map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
        .collect();

    for (l, r) in local_parts.iter().zip(release_parts.iter()) {
        if l > r {
            return true;
        }
        if l < r {
            return false;
        }
    }
    // All compared parts equal — local is at least as many parts as release
    local_parts.len() >= release_parts.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_equal() {
        assert!(compare_versions("0.1.0", "v0.1.0"));
    }

    #[test]
    fn test_compare_newer_local() {
        assert!(compare_versions("0.2.0", "v0.1.0"));
    }

    #[test]
    fn test_compare_older_local() {
        assert!(!compare_versions("0.1.0", "v0.2.0"));
    }

    #[test]
    fn test_compare_patch_ahead() {
        assert!(compare_versions("0.1.1", "v0.1.0"));
    }

    #[test]
    fn test_compare_major_ahead() {
        assert!(compare_versions("1.0.0", "v0.9.9"));
    }

    #[test]
    fn test_compare_same_different_prefix() {
        assert!(compare_versions("0.1.0", "0.1.0"));
    }

    #[test]
    fn test_compare_beta_same() {
        // 0.1.0 and v0.1.0-beta have the same numeric base version
        assert!(compare_versions("0.1.0", "v0.1.0-beta"));
    }

    #[test]
    fn test_compare_prerelease() {
        assert!(compare_versions("0.2.0-alpha", "v0.1.0"));
    }

    #[test]
    fn test_compare_with_build_meta() {
        assert!(compare_versions("0.1.0", "v0.1.0+build123"));
    }
}