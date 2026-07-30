//! CLI Integration: Init/Status tests
//!
//! Tests vault initialization, mnemonic generation, and status queries.
//! Each test uses a unique port to avoid port conflicts from lingering servers.

use cli_integration::client::{assert_ok, call};
use cli_integration::server::spawn_test_server;

#[tokio::test]
async fn generate_mnemonic_returns_24_words() {
    let (token, _handle, _am) = spawn_test_server(19991).await;

    let r = call(19991, &token, "vault.generate_mnemonic", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.generate_mnemonic");
    let mnemonic = result["mnemonic"].as_str().expect("test invariant");
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    assert_eq!(words.len(), 24, "expected 24 words, got {}", words.len());

    for w in &words {
        assert!(!w.is_empty(), "empty word in mnemonic");
        assert!(w.chars().all(|c| c.is_ascii_lowercase()), "non-lowercase word: {w}");
    }
}

#[tokio::test]
async fn initialize_with_mnemonic() {
    let (token, _handle, _am) = spawn_test_server(19992).await;

    let r = call(19992, &token, "vault.generate_mnemonic", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.generate_mnemonic");
    let mnemonic = result["mnemonic"].as_str().expect("test invariant").to_string();

    let r = call(19992, &token, "vault.initialize", serde_json::json!({
        "seed_phrase": mnemonic
    })).await;
    let result = assert_ok(&r, "vault.initialize");
    assert_eq!(result["initialized"], true, "should be initialized");
    assert!(result.get("master_key").is_some(), "master_key should be present");
}

#[tokio::test]
async fn initialize_with_passphrase() {
    let (token, _handle, _am) = spawn_test_server(19993).await;
    let (token2, _handle2, _am2) = spawn_test_server(19994).await;

    let r = call(19993, &token, "vault.generate_mnemonic", serde_json::json!({})).await;
    let mnemonic = assert_ok(&r, "vault.generate_mnemonic")["mnemonic"]
        .as_str().expect("test invariant").to_string();

    let r = call(19993, &token, "vault.initialize", serde_json::json!({
        "seed_phrase": &mnemonic,
        "passphrase": ""
    })).await;
    let mk_no_pp = assert_ok(&r, "vault.initialize (no passphrase)")["master_key"]
        .clone();

    let r = call(19994, &token2, "vault.initialize", serde_json::json!({
        "seed_phrase": &mnemonic,
        "passphrase": "satoshi"
    })).await;
    let mk_with_pp = assert_ok(&r, "vault.initialize (with passphrase)")["master_key"]
        .clone();

    assert_ne!(mk_no_pp, mk_with_pp, "passphrase should produce different master key");
}

#[tokio::test]
async fn status_pre_init() {
    let (token, _handle, _am) = spawn_test_server(19995).await;

    let r = call(19995, &token, "vault.status", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.status (pre-init)");
    assert_eq!(result["initialized"], false, "should not be initialized yet");
}

#[tokio::test]
async fn status_post_init() {
    let (token, _handle, _am) = spawn_test_server(19996).await;

    let r = call(19996, &token, "vault.generate_mnemonic", serde_json::json!({})).await;
    let mnemonic = assert_ok(&r, "vault.generate_mnemonic")["mnemonic"]
        .as_str().expect("test invariant").to_string();

    let r = call(19996, &token, "vault.initialize", serde_json::json!({
        "seed_phrase": mnemonic
    })).await;
    assert_ok(&r, "vault.initialize");

    let r = call(19996, &token, "vault.status", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.status (post-init)");
    assert_eq!(result["initialized"], true, "should be initialized");
    assert!(!result["plugin_ids"].as_array().expect("test invariant").is_empty(), "plugin_ids non-empty");
    assert!(!result["networks"].as_array().expect("test invariant").is_empty(), "networks non-empty");
}