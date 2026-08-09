use async_trait::async_trait;

use crate::error::PluginError;
use crate::types::{
    Account, Balance, Capability, FeeEstimate, NetworkSpec, PluginMetadata, TxRecord,
};

/// The core trait that every blockchain plugin must implement.
/// Methods are async so plugins can perform network I/O (RPC calls, etc.)
/// without blocking the async runtime.
#[async_trait]
pub trait WalletPlugin: Send + Sync {
    /// FPI (FOSS Plugin Interface) spec version this plugin conforms to.
    fn fpi_version(&self) -> &'static str {
        "0.1.0"
    }

    /// Plugin version in semver format.
    fn plugin_version(&self) -> &'static str {
        "0.1.0"
    }

    /// Unique identifier for this plugin (e.g. "bitcoin", "ethereum").
    fn id(&self) -> &'static str;

    /// Human-readable name for this plugin.
    fn name(&self) -> &'static str;

    /// Metadata for discovery and registry use.
    fn plugin_metadata(&self) -> PluginMetadata;

    /// Bitmask-style capabilities this plugin advertises.
    fn capabilities(&self) -> Vec<Capability>;

    /// Networks this plugin supports (mainnet + testnets).
    fn supported_networks(&self) -> &[NetworkSpec];

    /// Derive an account from a seed at the given BIP index.
    async fn create_account(
        &self,
        seed: &[u8],
        index: u32,
        network: &str,
    ) -> Result<Account, PluginError>;

    /// Sign a raw transaction using the master seed and account index.
    async fn sign_transaction(
        &self,
        tx: &[u8],
        seed: &[u8],
        account_index: u32,
        network: &str,
    ) -> Result<Vec<u8>, PluginError>;

    /// Broadcast a signed transaction to the network.
    /// Returns the transaction ID (txid / hash).
    async fn broadcast_transaction(
        &self,
        signed_tx: &[u8],
        network: &str,
    ) -> Result<String, PluginError>;

    /// Get the balance of an account.
    async fn get_balance(&self, account: &Account, network: &str) -> Result<Balance, PluginError>;

    /// Get transaction history for an account.
    async fn get_transaction_history(
        &self,
        account: &Account,
        network: &str,
        limit: u32,
    ) -> Result<Vec<TxRecord>, PluginError>;

    /// Estimate the fee for a transaction.
    async fn estimate_fee(&self, tx: &[u8], network: &str) -> Result<FeeEstimate, PluginError>;

    /// Validate an address on the given network.
    async fn validate_address(&self, address: &str, network: &str) -> Result<bool, PluginError>;
}
