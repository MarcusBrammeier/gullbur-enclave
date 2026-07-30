//! HTTP JSON-RPC bridge — serves vault-core operations over HTTP
//! with response shapes matching the desktop Tauri `commands.rs` interface.
//!
//! This crate defines the HTTP server and response types. The concrete
//! `VaultBridge` trait implementation is provided by the consumer
//! (vault-core or a standalone binary).
//!
//! ## Architecture
//!
//! ```text
//! Browser / Extension → HTTP POST /json-rpc → http_bridge::run()
//!   → VaultBridge trait → concrete impl (vault-core) → response
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use async_trait::async_trait;

use axum::{Router, routing::post, Json, extract::State, response::IntoResponse, http::StatusCode};

// ── VaultBridge trait ──────────────────────────────────────

/// Trait that the vault engine implements so the HTTP bridge
/// can dispatch requests without coupling to vault-core directly.
#[async_trait]
pub trait VaultBridge: Send + Sync {
    /// Get vault status.
    async fn status(&self) -> Result<Value, String>;
    /// Initialize the vault with an optional seed phrase and passphrase.
    async fn initialize(&self, seed: Option<&str>, passphrase: Option<&str>) -> Result<Value, String>;
    /// Create a wallet account on the given network.
    async fn create_account(&self, network: &str, index: u32) -> Result<Value, String>;
    /// Get balance for an address on a network.
    async fn get_balance(&self, network: &str, address: &str) -> Result<Value, String>;
    /// Sign a transaction.
    async fn sign_transaction(&self, network: &str, tx_hex: &str, key_id: &str) -> Result<Value, String>;
    /// Broadcast a signed transaction.
    async fn broadcast_transaction(&self, network: &str, signed_tx_hex: &str) -> Result<Value, String>;
    /// List all registered networks.
    async fn list_networks(&self) -> Result<Value, String>;
    /// Validate an address.
    async fn validate_address(&self, network: &str, address: &str) -> Result<Value, String>;
}

// ── Response types (matching commands.rs shapes) ───────────

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeStatus {
    pub initialized: bool,
    pub connected: bool,
    pub tor_enabled: bool,
    pub active_plugins: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeAccount {
    pub id: String,
    pub network: String,
    pub address: String,
    pub balance: String,
}

// ── HTTP server ───────────────────────────────────────────

/// Start the HTTP JSON-RPC bridge on the given port.
pub async fn run(bridge: Arc<dyn VaultBridge>, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;

    let shared = bridge;
    let app = Router::new()
        .route("/json-rpc", post(handle_rpc))
        .with_state(shared);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("HTTP bridge listening on {}/json-rpc", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_rpc(
    State(bridge): State<Arc<dyn VaultBridge>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let method = body
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let params = body.get("params").cloned().unwrap_or(Value::Null);

    let result = match method.as_str() {
        "status" => bridge.status().await,
        "initialize" => {
            let seed = params.get("seed").and_then(|v| v.as_str());
            let passphrase = params.get("passphrase").and_then(|v| v.as_str());
            bridge.initialize(seed, passphrase).await
        }
        "create_account" => {
            let network = params.get("network").and_then(|v| v.as_str()).unwrap_or("");
            let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            bridge.create_account(network, index).await
        }
        "get_balance" => {
            let network = params.get("network").and_then(|v| v.as_str()).unwrap_or("");
            let address = params.get("address").and_then(|v| v.as_str()).unwrap_or("");
            bridge.get_balance(network, address).await
        }
        "sign_transaction" => {
            let network = params.get("network").and_then(|v| v.as_str()).unwrap_or("");
            let tx_hex = params.get("tx_hex").and_then(|v| v.as_str()).unwrap_or("");
            let key_id = params.get("key_id").and_then(|v| v.as_str()).unwrap_or("");
            bridge.sign_transaction(network, tx_hex, key_id).await
        }
        "broadcast_transaction" => {
            let network = params.get("network").and_then(|v| v.as_str()).unwrap_or("");
            let signed_tx_hex = params.get("signed_tx_hex").and_then(|v| v.as_str()).unwrap_or("");
            bridge.broadcast_transaction(network, signed_tx_hex).await
        }
        "list_networks" => bridge.list_networks().await,
        "validate_address" => {
            let network = params.get("network").and_then(|v| v.as_str()).unwrap_or("");
            let address = params.get("address").and_then(|v| v.as_str()).unwrap_or("");
            bridge.validate_address(network, address).await
        }
        _ => Err(format!("Unknown method: {method}")),
    };

    match result {
        Ok(val) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "result": val,
                "id": body.get("id").cloned().unwrap_or(Value::Null),
            })),
        ),
        Err(msg) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "error": { "code": -32000, "message": msg },
                "id": body.get("id").cloned().unwrap_or(Value::Null),
            })),
        ),
    }
}