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

// ── Per-network operations tests ────────────────────────────────────

const OP_PORT: u16 = 19870;

async fn setup_op_server(port: u16) -> (String, tokio::task::JoinHandle<()>) {
    let (token, handle, _am) = spawn_test_server(port).await;
    let r = call(
        port,
        &token,
        "vault.generate_mnemonic",
        serde_json::json!({}),
    )
    .await;
    let mnemonic = assert_ok(&r, "vault.generate_mnemonic")["mnemonic"]
        .as_str()
        .expect("invariant")
        .to_string();
    let r = call(
        port,
        &token,
        "vault.initialize",
        serde_json::json!({"seed_phrase": mnemonic}),
    )
    .await;
    assert_ok(&r, "vault.initialize");
    (token, handle)
}

#[tokio::test]
async fn get_balance_btc() {
    let (token, _handle) = setup_op_server(OP_PORT).await;
    let r = call(
        OP_PORT,
        &token,
        "vault.create_account",
        serde_json::json!({"network": "bitcoin", "index": 0}),
    )
    .await;
    let addr = assert_ok(&r, "create btc")["address"]
        .as_str()
        .expect("invariant")
        .to_string();
    let r = call(
        OP_PORT,
        &token,
        "vault.get_balance",
        serde_json::json!({"network": "bitcoin", "address": &addr}),
    )
    .await;
    if let Some(err) = r.get("error") {
        let code = err["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "get_balance BTC must route");
    }
}

#[tokio::test]
async fn get_balance_xmr() {
    let (token, _handle) = setup_op_server(OP_PORT + 1).await;
    let r = call(
        OP_PORT + 1,
        &token,
        "vault.create_account",
        serde_json::json!({"network": "monero", "index": 0}),
    )
    .await;
    let addr = assert_ok(&r, "create xmr")["address"]
        .as_str()
        .expect("invariant")
        .to_string();
    let r = call(
        OP_PORT + 1,
        &token,
        "vault.get_balance",
        serde_json::json!({"network": "monero", "address": &addr}),
    )
    .await;
    if let Some(err) = r.get("error") {
        let code = err["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "get_balance XMR must route");
    }
}

#[tokio::test]
async fn get_balance_ltc() {
    let (token, _handle) = setup_op_server(OP_PORT + 2).await;
    let r = call(
        OP_PORT + 2,
        &token,
        "vault.create_account",
        serde_json::json!({"network": "litecoin", "index": 0}),
    )
    .await;
    let addr = assert_ok(&r, "create ltc")["address"]
        .as_str()
        .expect("invariant")
        .to_string();
    let r = call(
        OP_PORT + 2,
        &token,
        "vault.get_balance",
        serde_json::json!({"network": "litecoin", "address": &addr}),
    )
    .await;
    if let Some(err) = r.get("error") {
        let code = err["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "get_balance LTC must route");
    }
}

#[tokio::test]
async fn estimate_fee_eth() {
    let (token, _handle) = setup_op_server(OP_PORT + 3).await;
    let r = call(OP_PORT + 3, &token, "vault.estimate_fee",
        serde_json::json!({"network": "ethereum", "recipient": "0x1234567890123456789012345678901234567890", "amount": "0.001"})).await;
    if let Some(err) = r.get("error") {
        let code = err["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "estimate_fee ETH must route");
    }
}

#[tokio::test]
async fn estimate_fee_ltc() {
    let (token, _handle) = setup_op_server(OP_PORT + 4).await;
    let r = call(
        OP_PORT + 4,
        &token,
        "vault.estimate_fee",
        serde_json::json!({"network": "litecoin", "recipient": "ltc1qtest", "amount": "0.001"}),
    )
    .await;
    if let Some(err) = r.get("error") {
        let code = err["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "estimate_fee LTC must route");
    }
}

#[tokio::test]
async fn tx_history_btc() {
    let (token, _handle) = setup_op_server(OP_PORT + 5).await;
    let r = call(OP_PORT + 5, &token, "vault.get_transaction_history",
        serde_json::json!({"network": "bitcoin", "address": "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4", "limit": 5})).await;
    if let Some(err) = r.get("error") {
        let code = err["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "tx_history BTC must route");
    }
}

#[tokio::test]
async fn tx_history_ltc() {
    let (token, _handle) = setup_op_server(OP_PORT + 6).await;
    let r = call(
        OP_PORT + 6,
        &token,
        "vault.get_transaction_history",
        serde_json::json!({"network": "litecoin", "address": "ltc1qtest", "limit": 5}),
    )
    .await;
    if let Some(err) = r.get("error") {
        let code = err["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "tx_history LTC must route");
    }
}
