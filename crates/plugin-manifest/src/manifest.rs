use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Error types for manifest loading and validation.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(String),
    #[error("YAML parse error: {0}")]
    Yaml(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("missing field: {0}")]
    MissingField(String),
}

/// Sandbox isolation level for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxLevel {
    /// Untrusted plugin — runs in a WASM sandbox, no direct system access.
    #[default]
    Wasm,
    /// Trusted plugin — linked directly into the wallet process.
    Native,
    /// External binary — communicates via IPC over a Unix socket or stdio.
    Binary,
}

/// A supported chain entry in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainEntry {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub rpc_urls: Vec<String>,
}

/// The FPI plugin manifest (`plugin.toml`).
///
/// Every plugin must ship a `plugin.toml` at its root that describes
/// its identity, capabilities, and network connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin identifier (e.g. "litecoin", "solana"). Must match the plugin's `WalletPlugin::id()`.
    pub id: String,
    /// Human-readable name (e.g. "Litecoin").
    pub name: String,
    /// Semantic version of this plugin.
    pub version: String,
    /// FPI spec version this manifest conforms to.
    #[serde(default = "default_fpi_version")]
    pub fpi_version: String,
    /// Minimum wallet version required to load this plugin.
    #[serde(default = "default_min_wallet")]
    pub min_wallet_version: String,

    // ── Authorship ─────────────────────────────────────────────
    pub author: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,

    // ── Tags ───────────────────────────────────────────────────
    #[serde(default)]
    pub tags: Vec<String>,

    // ── Capabilities ───────────────────────────────────────────
    #[serde(default)]
    pub capabilities: Vec<String>,

    // ── Sandboxing ─────────────────────────────────────────────
    #[serde(default)]
    pub sandbox: SandboxLevel,

    // ── Supported chains ───────────────────────────────────────
    #[serde(default)]
    pub chains: Vec<ChainEntry>,

    // ── Extra metadata (pass-through, not validated) ───────────
    #[serde(default)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

fn default_fpi_version() -> String {
    "0.1.0".into()
}
fn default_min_wallet() -> String {
    "0.1.0".into()
}

impl PluginManifest {
    /// Load and validate a manifest from a TOML file at `path`.
    pub fn load_toml(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let text = std::fs::read_to_string(path.as_ref())?;
        let manifest: PluginManifest =
            toml::from_str(&text).map_err(|e| ManifestError::Toml(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Load and validate a manifest from a TOML string.
    pub fn from_toml_str(text: &str) -> Result<Self, ManifestError> {
        let manifest: PluginManifest =
            toml::from_str(text).map_err(|e| ManifestError::Toml(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Load and validate a manifest from a YAML string.
    pub fn from_yaml_str(text: &str) -> Result<Self, ManifestError> {
        let manifest: PluginManifest =
            serde_yaml::from_str(text).map_err(|e| ManifestError::Yaml(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Serialize this manifest back to TOML.
    pub fn to_toml_string(&self) -> Result<String, ManifestError> {
        toml::to_string_pretty(self).map_err(|e| ManifestError::Toml(e.to_string()))
    }

    /// Validate required fields.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.id.is_empty() {
            return Err(ManifestError::Validation("id must not be empty".into()));
        }
        if self.name.is_empty() {
            return Err(ManifestError::Validation("name must not be empty".into()));
        }
        if self.version.is_empty() {
            return Err(ManifestError::Validation(
                "version must not be empty".into(),
            ));
        }
        // Validate semver-ish — at least X.Y
        if !self.version.contains('.') {
            return Err(ManifestError::Validation(format!(
                "version '{}' is not semver (expected X.Y.Z)",
                self.version
            )));
        }
        // Validate chain entries have required fields
        for chain in &self.chains {
            if chain.id.is_empty() {
                return Err(ManifestError::Validation("chain entry has empty id".into()));
            }
            if chain.rpc_urls.is_empty() {
                return Err(ManifestError::Validation(format!(
                    "chain '{}' has no rpc_urls",
                    chain.id
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_manifest() {
        let toml = r#"
id = "litecoin"
name = "Litecoin"
version = "0.1.0"
"#;
        let m = PluginManifest::from_toml_str(toml).expect("test invariant");
        assert_eq!(m.id, "litecoin");
        assert_eq!(m.fpi_version, "0.1.0");
        assert_eq!(m.capabilities.len(), 0);
    }

    #[test]
    fn test_full_manifest() {
        let toml = r#"
id = "bitcoin"
name = "Bitcoin"
version = "1.0.0"
fpi_version = "0.1.0"
min_wallet_version = "0.1.0"
author = "Gullbúr Enclave"
description = "Bitcoin P2WPKH"
capabilities = ["signing", "broadcast", "balance"]

[[chains]]
id = "bitcoin"
name = "Bitcoin"
symbol = "BTC"
decimals = 8
rpc_urls = ["https://blockstream.info/api"]
"#;
        let m = PluginManifest::from_toml_str(toml).expect("test invariant");
        assert_eq!(m.id, "bitcoin");
        assert_eq!(m.chains.len(), 1);
        assert_eq!(m.chains[0].rpc_urls[0], "https://blockstream.info/api");
    }

    #[test]
    fn test_requires_semver() {
        let toml = r#"
id = "bad"
name = "Bad"
version = "1"
"#;
        let err = PluginManifest::from_toml_str(toml).unwrap_err();
        assert!(err.to_string().contains("semver"), "bad version: {err}");
    }

    #[test]
    fn test_requires_id() {
        let toml = r#"
id = ""
name = "Empty"
version = "0.1.0"
"#;
        let err = PluginManifest::from_toml_str(toml).unwrap_err();
        assert!(err.to_string().contains("id"), "empty id: {err}");
    }

    #[test]
    fn test_chain_must_have_rpc() {
        let toml = r#"
id = "test"
name = "Test"
version = "0.1.0"

[[chains]]
id = "no-rpc"
name = "No RPC"
symbol = "X"
decimals = 8
rpc_urls = []
"#;
        let err = PluginManifest::from_toml_str(toml).unwrap_err();
        assert!(err.to_string().contains("no rpc_urls"), "no rpc: {err}");
    }

    #[test]
    fn test_yaml_load() {
        let yaml = r#"
id: dogecoin
name: Dogecoin
version: 0.1.0
"#;
        let m = PluginManifest::from_yaml_str(yaml).expect("test invariant");
        assert_eq!(m.id, "dogecoin");
    }

    #[test]
    fn test_toml_roundtrip() {
        let m = PluginManifest {
            id: "btc".into(),
            name: "Bitcoin".into(),
            version: "0.1.0".into(),
            fpi_version: "0.1.0".into(),
            min_wallet_version: "0.1.0".into(),
            author: Some("FOSS".into()),
            description: None,
            homepage: None,
            repository: None,
            tags: vec![],
            capabilities: vec!["signing".into()],
            sandbox: SandboxLevel::Native,
            chains: vec![],
            extra: HashMap::new(),
        };
        let toml = m.to_toml_string().expect("test invariant");
        let parsed = PluginManifest::from_toml_str(&toml).expect("test invariant");
        assert_eq!(parsed.id, "btc");
        assert_eq!(parsed.capabilities, vec!["signing".to_string()]);
        assert_eq!(parsed.sandbox, SandboxLevel::Native);
    }
}
