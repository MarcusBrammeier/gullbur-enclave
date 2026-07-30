//! CLI Integration: Account creation and validation tests

use cli_integration::client::{assert_ok, call};
use cli_integration::server::spawn_test_server;

const PORT: u16 = 19980;

async fn setup_server(port: u16) -> (String, tokio::task::JoinHandle<()>) {
    let (token, handle, _am) = spawn_test_server(port).await;

    // Initialize vault
    let r = call(port, &token, "vault.generate_mnemonic", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.generate_mnemonic");
    let mnemonic = result["mnemonic"].as_str().expect("test invariant").to_string();

    let r = call(port, &token, "vault.initialize", serde_json::json!({
        "seed_phrase": mnemonic
    })).await;
    assert_ok(&r, "vault.initialize");

    (token, handle)
}

#[tokio::test]
async fn create_btc_account() {
    let (token, _handle) = setup_server(PORT).await;

    let r = call(PORT, &token, "vault.create_account", serde_json::json!({
        "network": "bitcoin",
        "index": 0
    })).await;
    let result = assert_ok(&r, "vault.create_account (bitcoin)");
    let address = result["address"].as_str().expect("test invariant");
    // BTC addresses start with bc1 (bech32) or tb1 (testnet)
    assert!(
        address.starts_with("bc1") || address.starts_with("tb1"),
        "BTC address should start with bc1/tb1, got: {address}"
    );
}

#[tokio::test]
async fn create_eth_account() {
    let (token, _handle) = setup_server(PORT + 1).await;

    let r = call(PORT + 1, &token, "vault.create_account", serde_json::json!({
        "network": "ethereum",
        "index": 0
    })).await;
    let result = assert_ok(&r, "vault.create_account (ethereum)");
    let address = result["address"].as_str().expect("test invariant");
    assert!(address.starts_with("0x"), "ETH address should start with 0x, got: {address}");
    assert_eq!(address.len(), 42, "ETH address should be 42 chars");
}

#[tokio::test]
async fn create_xmr_account() {
    let (token, _handle) = setup_server(PORT + 2).await;

    let r = call(PORT + 2, &token, "vault.create_account", serde_json::json!({
        "network": "monero",
        "index": 0
    })).await;
    let result = assert_ok(&r, "vault.create_account (monero)");
    let address = result["address"].as_str().expect("test invariant");
    assert!(!address.is_empty(), "XMR address should be non-empty");
}

#[tokio::test]
async fn create_ltc_account() {
    let (token, _handle) = setup_server(PORT + 3).await;

    let r = call(PORT + 3, &token, "vault.create_account", serde_json::json!({
        "network": "litecoin",
        "index": 0
    })).await;
    let result = assert_ok(&r, "vault.create_account (litecoin)");
    let address = result["address"].as_str().expect("test invariant");
    assert!(!address.is_empty(), "LTC address should be non-empty");
}

#[tokio::test]
async fn create_sepolia_account() {
    let (token, _handle) = setup_server(PORT + 4).await;

    let r = call(PORT + 4, &token, "vault.create_account", serde_json::json!({
        "network": "sepolia",
        "index": 0
    })).await;
    let result = assert_ok(&r, "vault.create_account (sepolia)");
    let address = result["address"].as_str().expect("test invariant");
    assert!(address.starts_with("0x"), "Sepolia address should start with 0x");
    assert_eq!(address.len(), 42, "Sepolia address should be 42 chars");
}

#[tokio::test]
async fn list_accounts_after_creation() {
    let (token, _handle) = setup_server(PORT + 5).await;

    // Create 5 accounts across different networks
    for (i, net) in ["bitcoin", "ethereum", "monero", "litecoin", "sepolia"].iter().enumerate() {
        let r = call(PORT + 5, &token, "vault.create_account", serde_json::json!({
            "network": net,
            "index": i as u64
        })).await;
        assert_ok(&r, &format!("create_account ({net})"));
    }

    let r = call(PORT + 5, &token, "vault.list_accounts", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.list_accounts");
    let accounts = result.as_array().expect("test invariant");
    assert_eq!(accounts.len(), 5, "expected 5 accounts, got {}", accounts.len());
    for acct in accounts {
        assert!(acct.get("id").is_some(), "account missing id");
        assert!(acct.get("network").is_some(), "account missing network");
        assert!(acct.get("address").is_some(), "account missing address");
    }
}

#[tokio::test]
async fn validate_valid_address() {
    let (token, _handle) = setup_server(PORT + 6).await;

    // A well-known Bitcoin mainnet bech32 address
    let r = call(PORT + 6, &token, "vault.validate_address", serde_json::json!({
        "network": "bitcoin",
        "address": "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"
    })).await;
    let result = assert_ok(&r, "vault.validate_address (valid)");
    let valid = result["valid"].as_bool().unwrap_or(false);
    assert!(valid, "expected valid Bitcoin testnet address");
}

#[tokio::test]
async fn validate_invalid_address() {
    let (token, _handle) = setup_server(PORT + 7).await;

    let r = call(PORT + 7, &token, "vault.validate_address", serde_json::json!({
        "network": "bitcoin",
        "address": "not-a-valid-address-xyz"
    })).await;
    // Either the handler returns with invalid=false, or errors (either is acceptable
    // as long as handler routes — not -32601)
    if let Some(error) = r.get("error") {
        let code = error["code"].as_i64().unwrap_or(0);
        assert_ne!(code, -32601, "validate_address must be a registered handler");
    } else if let Some(result) = r.get("result") {
        if let Some(valid) = result.get("valid").and_then(|v| v.as_bool()) {
            assert!(!valid, "expected invalid for garbage address");
        }
    }
}