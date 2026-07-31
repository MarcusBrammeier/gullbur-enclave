//! CLI Integration: Operations tests (balance, fees, history, networks, plugins)
//!
//! These verify handler routing — actual network calls may fail without
//! live RPC endpoints, but the handler must be registered (not -32601).

use cli_integration::client::{assert_ok, call};
use cli_integration::server::spawn_test_server;

const PORT: u16 = 19970;

async fn setup_server(port: u16) -> (String, tokio::task::JoinHandle<()>) {
    let (token, handle, _am) = spawn_test_server(port).await;

    // Initialize vault
    let r = call(
        port,
        &token,
        "vault.generate_mnemonic",
        serde_json::json!({}),
    )
    .await;
    let result = assert_ok(&r, "vault.generate_mnemonic");
    let mnemonic = result["mnemonic"]
        .as_str()
        .expect("test invariant")
        .to_string();

    let r = call(
        port,
        &token,
        "vault.initialize",
        serde_json::json!({
            "seed_phrase": mnemonic
        }),
    )
    .await;
    assert_ok(&r, "vault.initialize");

    (token, handle)
}

#[tokio::test]
async fn get_balance_routes() {
    let (token, _handle) = setup_server(PORT).await;

    let r = call(
        PORT,
        &token,
        "vault.get_balance",
        serde_json::json!({
            "network": "ethereum",
            "address": "0x1234567890123456789012345678901234567890"
        }),
    )
    .await;

    // Must route (not -32601). May error from network, may succeed with 0.
    if let Some(error) = r.get("error") {
        let code = error["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "get_balance must be a registered handler");
    } else {
        // Success — at minimum should have a unit field
        let result = r.get("result").expect("test invariant");
        assert!(
            result.get("unit").is_some() || result.get("balance").is_some(),
            "balance response should contain unit or balance: {result}"
        );
    }
}

#[tokio::test]
async fn estimate_fee_routes() {
    let (token, _handle) = setup_server(PORT + 1).await;

    let r = call(
        PORT + 1,
        &token,
        "vault.estimate_fee",
        serde_json::json!({
            "network": "bitcoin"
        }),
    )
    .await;

    // Must route
    if let Some(error) = r.get("error") {
        let code = error["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "estimate_fee must be a registered handler");
    }
}

#[tokio::test]
async fn transaction_history_routes() {
    let (token, _handle) = setup_server(PORT + 2).await;

    let r = call(
        PORT + 2,
        &token,
        "vault.get_transaction_history",
        serde_json::json!({
            "network": "ethereum",
            "address": "0x1234567890123456789012345678901234567890",
            "limit": 5
        }),
    )
    .await;

    // Must route
    if let Some(error) = r.get("error") {
        let code = error["code"].as_i64().unwrap_or(0);
        assert_ne!(
            code, -32601,
            "get_transaction_history must be a registered handler"
        );
    }
}

#[tokio::test]
async fn list_networks() {
    let (token, _handle) = setup_server(PORT + 3).await;

    let r = call(
        PORT + 3,
        &token,
        "vault.list_networks",
        serde_json::json!({}),
    )
    .await;
    let result = assert_ok(&r, "vault.list_networks");
    let networks = result.as_array().expect("test invariant");
    // With 4 plugins (BTC, EVM, XMR, LTC), we expect at least 5+ networks
    assert!(
        networks.len() >= 5,
        "expected >=5 networks, got {}",
        networks.len()
    );
}

#[tokio::test]
async fn list_plugins() {
    let (token, _handle) = setup_server(PORT + 4).await;

    let r = call(PORT + 4, &token, "vault.status", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.status (list_plugins)");
    let plugin_ids = result["plugin_ids"].as_array().expect("test invariant");
    let ids: Vec<&str> = plugin_ids
        .iter()
        .map(|v| v.as_str().expect("test invariant"))
        .collect();
    assert!(ids.contains(&"btc"), "expected btc plugin, got: {ids:?}");
    assert!(ids.contains(&"evm"), "expected evm plugin, got: {ids:?}");
    assert!(ids.contains(&"xmr"), "expected xmr plugin, got: {ids:?}");
    assert!(ids.contains(&"ltc"), "expected ltc plugin, got: {ids:?}");
}
