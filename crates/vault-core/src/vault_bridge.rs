//! VaultBridge trait implementation — exposes vault operations through HTTP JSON-RPC.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

use crate::host::PluginHost;
use ipc_core::http_bridge::VaultBridge;

pub struct VaultBridgeImpl {
    plugin_host: Arc<RwLock<PluginHost>>,
    initialized: Arc<AtomicBool>,
    seed: Arc<RwLock<Option<zeroize::Zeroizing<Vec<u8>>>>>,
}

impl VaultBridgeImpl {
    pub fn new(
        plugin_host: Arc<RwLock<PluginHost>>,
        initialized: Arc<AtomicBool>,
        seed: Arc<RwLock<Option<zeroize::Zeroizing<Vec<u8>>>>>,
    ) -> Self {
        Self {
            plugin_host,
            initialized,
            seed,
        }
    }
}

#[async_trait]
impl VaultBridge for VaultBridgeImpl {
    async fn status(&self) -> Result<Value, String> {
        let init = self.initialized.load(Ordering::SeqCst);
        let host = self.plugin_host.read().await;
        let accounts = host.list_accounts();
        Ok(serde_json::json!({
            "initialized": init,
            "status": if init { "Initialized" } else { "Connected" },
            "connected": true,
            "tor_enabled": false,
            "active_plugins": host.plugin_ids(),
            "accounts": accounts,
        }))
    }

    async fn initialize(
        &self,
        _seed: Option<&str>,
        _passphrase: Option<&str>,
    ) -> Result<Value, String> {
        Err("use WebSocket IPC for initialize — HTTP bridge is read-only for initialization".into())
    }

    async fn create_account(&self, network: &str, index: u32) -> Result<Value, String> {
        let seed_guard = self.seed.read().await;
        let seed = seed_guard.as_ref().ok_or("vault not initialized")?.clone();
        drop(seed_guard);
        let host = self.plugin_host.read().await;
        let account = host
            .create_account(&seed, index, network)
            .await
            .map_err(|e| e.to_string())?;
        Ok(serde_json::json!(account))
    }

    async fn get_balance(&self, network: &str, address: &str) -> Result<Value, String> {
        let host = self.plugin_host.read().await;
        let acc = wallet_plugin::Account {
            id: format!("{network}-{address}"),
            network: network.into(),
            address: address.into(),
            path: None,
            label: None,
        };
        let balance = host
            .get_balance(&acc, network)
            .await
            .map_err(|e| e.to_string())?;
        Ok(serde_json::json!(balance))
    }

    async fn sign_transaction(
        &self,
        network: &str,
        tx_hex: &str,
        key_id: &str,
    ) -> Result<Value, String> {
        let tx_bytes = hex::decode(tx_hex).map_err(|e| format!("hex decode: {e}"))?;
        let key = wallet_plugin::KeyHandle {
            key_id: key_id.to_string(),
            key_type: wallet_plugin::KeyType::Secp256k1,
            public_key: vec![],
        };
        let host = self.plugin_host.read().await;
        let signed = host
            .sign_transaction(&tx_bytes, &key, network)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Value::String(hex::encode(signed)))
    }

    async fn broadcast_transaction(
        &self,
        network: &str,
        signed_tx_hex: &str,
    ) -> Result<Value, String> {
        let tx_bytes = hex::decode(signed_tx_hex).map_err(|e| format!("hex decode: {e}"))?;
        let host = self.plugin_host.read().await;
        let txid = host
            .broadcast_transaction(&tx_bytes, network)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Value::String(txid))
    }

    async fn list_networks(&self) -> Result<Value, String> {
        let host = self.plugin_host.read().await;
        let networks: Vec<Value> = host
            .plugin_ids()
            .into_iter()
            .map(|id| serde_json::json!({"plugin": id}))
            .collect();
        Ok(Value::Array(networks))
    }

    async fn validate_address(&self, network: &str, address: &str) -> Result<Value, String> {
        let host = self.plugin_host.read().await;
        let valid = host
            .validate_address(address, network)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Value::Bool(valid))
    }
}
