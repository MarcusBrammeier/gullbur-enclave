//! CLI Integration: Init/Status tests
//!
//! Tests vault initialization, mnemonic generation, and status queries.
//! Each test uses a unique port to avoid port conflicts from lingering servers.

use cli_integration::client::{assert_ok, call};
use cli_integration::server::spawn_test_server;

#[tokio::test]
async fn generate_mnemonic_returns_24_words() {
    let (token, _handle, _am) = spawn_test_server(19991).await;

    let r = call(
        19991,
        &token,
        "vault.generate_mnemonic",
        serde_json::json!({}),
    )
    .await;
    let result = assert_ok(&r, "vault.generate_mnemonic");
    let mnemonic = result["mnemonic"].as_str().expect("test invariant");
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    assert_eq!(words.len(), 24, "expected 24 words, got {}", words.len());

    for w in &words {
        assert!(!w.is_empty(), "empty word in mnemonic");
        assert!(
            w.chars().all(|c| c.is_ascii_lowercase()),
            "non-lowercase word: {w}"
        );
    }
}

#[tokio::test]
async fn initialize_with_mnemonic() {
    let (token, _handle, _am) = spawn_test_server(19992).await;

    let r = call(
        19992,
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
        19992,
        &token,
        "vault.initialize",
        serde_json::json!({
            "seed_phrase": mnemonic
        }),
    )
    .await;
    let result = assert_ok(&r, "vault.initialize");
    assert_eq!(result["initialized"], true, "should be initialized");
    assert!(
        result.get("master_key").is_some(),
        "master_key should be present"
    );
}

#[tokio::test]
async fn initialize_with_passphrase() {
    let (token, _handle, _am) = spawn_test_server(19993).await;
    let (token2, _handle2, _am2) = spawn_test_server(19994).await;

    let r = call(
        19993,
        &token,
        "vault.generate_mnemonic",
        serde_json::json!({}),
    )
    .await;
    let mnemonic = assert_ok(&r, "vault.generate_mnemonic")["mnemonic"]
        .as_str()
        .expect("test invariant")
        .to_string();

    let r = call(
        19993,
        &token,
        "vault.initialize",
        serde_json::json!({
            "seed_phrase": &mnemonic,
            "passphrase": ""
        }),
    )
    .await;
    let mk_no_pp = assert_ok(&r, "vault.initialize (no passphrase)")["master_key"].clone();

    let r = call(
        19994,
        &token2,
        "vault.initialize",
        serde_json::json!({
            "seed_phrase": &mnemonic,
            "passphrase": "satoshi"
        }),
    )
    .await;
    let mk_with_pp = assert_ok(&r, "vault.initialize (with passphrase)")["master_key"].clone();

    assert_ne!(
        mk_no_pp, mk_with_pp,
        "passphrase should produce different master key"
    );
}

#[tokio::test]
async fn status_pre_init() {
    let (token, _handle, _am) = spawn_test_server(19995).await;

    let r = call(19995, &token, "vault.status", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.status (pre-init)");
    assert_eq!(
        result["initialized"], false,
        "should not be initialized yet"
    );
}

#[tokio::test]
async fn status_post_init() {
    let (token, _handle, _am) = spawn_test_server(19996).await;

    let r = call(
        19996,
        &token,
        "vault.generate_mnemonic",
        serde_json::json!({}),
    )
    .await;
    let mnemonic = assert_ok(&r, "vault.generate_mnemonic")["mnemonic"]
        .as_str()
        .expect("test invariant")
        .to_string();

    let r = call(
        19996,
        &token,
        "vault.initialize",
        serde_json::json!({
            "seed_phrase": mnemonic
        }),
    )
    .await;
    assert_ok(&r, "vault.initialize");

    let r = call(19996, &token, "vault.status", serde_json::json!({})).await;
    let result = assert_ok(&r, "vault.status (post-init)");
    assert_eq!(result["initialized"], true, "should be initialized");
    assert!(
        !result["plugin_ids"]
            .as_array()
            .expect("test invariant")
            .is_empty(),
        "plugin_ids non-empty"
    );
    assert!(
        !result["networks"]
            .as_array()
            .expect("test invariant")
            .is_empty(),
        "networks non-empty"
    );
}

// ── Deterministic seed restoration test ──────────────────────────────
// Proves that initializing two separate servers with the same BIP-39
// seed phrase produces identical addresses.

const DET_PORT: u16 = 19840;

#[tokio::test]
async fn deterministic_seed_restoration() {
    let seed = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

    // Session A: init with known seed, create accounts
    let (addrs_a, _) = {
        let (token, handle, _am) = spawn_test_server(DET_PORT).await;
        let r = call(
            DET_PORT,
            &token,
            "vault.initialize",
            serde_json::json!({"seed_phrase": seed}),
        )
        .await;
        assert_ok(&r, "vault.initialize (A)");

        let b = assert_ok(
            &call(
                DET_PORT,
                &token,
                "vault.create_account",
                serde_json::json!({"network":"bitcoin-testnet","index":0}),
            )
            .await,
            "btc A",
        )["address"]
            .clone();
        let e = assert_ok(
            &call(
                DET_PORT,
                &token,
                "vault.create_account",
                serde_json::json!({"network":"sepolia","index":0}),
            )
            .await,
            "eth A",
        )["address"]
            .clone();
        let l = assert_ok(
            &call(
                DET_PORT,
                &token,
                "vault.create_account",
                serde_json::json!({"network":"litecoin-testnet","index":0}),
            )
            .await,
            "ltc A",
        )["address"]
            .clone();
        let x = assert_ok(
            &call(
                DET_PORT,
                &token,
                "vault.create_account",
                serde_json::json!({"network":"monero-stagenet","index":0}),
            )
            .await,
            "xmr A",
        )["address"]
            .clone();
        (serde_json::json!({"btc":b,"eth":e,"ltc":l,"xmr":x}), handle)
    };

    // Session B: new server port, SAME seed
    let (addrs_b, _) = {
        let (token, handle, _am) = spawn_test_server(DET_PORT + 1).await;
        let r = call(
            DET_PORT + 1,
            &token,
            "vault.initialize",
            serde_json::json!({"seed_phrase": seed}),
        )
        .await;
        assert_ok(&r, "vault.initialize (B)");

        let b = assert_ok(
            &call(
                DET_PORT + 1,
                &token,
                "vault.create_account",
                serde_json::json!({"network":"bitcoin-testnet","index":0}),
            )
            .await,
            "btc B",
        )["address"]
            .clone();
        let e = assert_ok(
            &call(
                DET_PORT + 1,
                &token,
                "vault.create_account",
                serde_json::json!({"network":"sepolia","index":0}),
            )
            .await,
            "eth B",
        )["address"]
            .clone();
        let l = assert_ok(
            &call(
                DET_PORT + 1,
                &token,
                "vault.create_account",
                serde_json::json!({"network":"litecoin-testnet","index":0}),
            )
            .await,
            "ltc B",
        )["address"]
            .clone();
        let x = assert_ok(
            &call(
                DET_PORT + 1,
                &token,
                "vault.create_account",
                serde_json::json!({"network":"monero-stagenet","index":0}),
            )
            .await,
            "xmr B",
        )["address"]
            .clone();
        (serde_json::json!({"btc":b,"eth":e,"ltc":l,"xmr":x}), handle)
    };

    assert_eq!(
        addrs_a["btc"], addrs_b["btc"],
        "BTC must match from same seed"
    );
    assert_eq!(
        addrs_a["eth"], addrs_b["eth"],
        "ETH must match from same seed"
    );
    assert_eq!(
        addrs_a["ltc"], addrs_b["ltc"],
        "LTC must match from same seed"
    );
    assert_eq!(
        addrs_a["xmr"], addrs_b["xmr"],
        "XMR must match from same seed"
    );
}
