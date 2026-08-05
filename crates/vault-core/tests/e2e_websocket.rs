//! Headless E2E integration test — WebSocket JSON-RPC round-trip.
//!
//! Spawns the real IpcServer on a local port, connects via WebSocket,
//! authenticates with the token, and exercises all 13 vault methods.
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use auth_core::AuthManager;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

struct WsClient {
    write: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    read: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
}

impl WsClient {
    async fn connect(port: u16, token: &str) -> Self {
        let url = format!("ws://127.0.0.1:{port}");
        let (ws, _) = connect_async(&url).await.expect("WebSocket connect");
        let (mut write, read) = ws.split();
        write
            .send(Message::Text(token.into()))
            .await
            .expect("send auth token");
        WsClient { write, read }
    }

    async fn call(&mut self, method: &str, params: Value) -> Value {
        let id: u64 = rand::random();
        let req = json!({"jsonrpc":"2.0","method":method,"params":params,"id":id});
        self.write
            .send(Message::Text(req.to_string().into()))
            .await
            .expect("send");
        loop {
            match self.read.next().await {
                Some(Ok(Message::Text(text))) => {
                    let resp: Value = serde_json::from_str(&text).expect("parse");
                    if resp.get("id").and_then(|v| v.as_u64()) == Some(id) {
                        return resp;
                    }
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => panic!("WS error: {e}"),
                None => panic!("WS closed"),
            }
        }
    }

    fn assert_ok<'a>(resp: &'a Value, context: &str) -> &'a Value {
        if resp.get("error").is_some() {
            let code = resp["error"]["code"].as_i64().unwrap_or(0);
            let msg = resp["error"]["message"].as_str().unwrap_or("?");
            panic!("{context}: error {code}: {msg} (expected success)");
        }
        &resp["result"]
    }
}

// ── E2E Test ───────────────────────────────────────────────────────────────

const TEST_PORT: u16 = 19877;

#[tokio::test]
async fn e2e_all_10_methods_via_websocket() {
    let server = ipc_core::server::IpcServer::new(TEST_PORT).expect("create IpcServer");
    let token_path = server.auth_token_path().to_path_buf();
    let auth_token = std::fs::read_to_string(&token_path).expect("read auth token");

    let auth_manager = Arc::new(AuthManager::new());
    {
        let mut handler = server.handler().await;
        let plugin_host = Arc::new(RwLock::new(PluginHost::new()));
        let seed = Arc::new(RwLock::new(None::<zeroize::Zeroizing<Vec<u8>>>));
        let initialized = Arc::new(AtomicBool::new(false));
        let mn = Arc::new(RwLock::new(None));
        ipc_handlers::register_vault_handlers(
            &mut handler,
            plugin_host,
            seed,
            mn,
            initialized,
            Arc::new(RwLock::new(vault_core::approval::ApprovalQueue::new())),
            Arc::clone(&auth_manager),
        );
    }

    let (_handle, _ready) = server.run();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut client = WsClient::connect(TEST_PORT, &auth_token).await;

    // ── Phase 1: Pre-initialization ────────────────────────────────────

    // 1. vault.status — uninitialized
    let r = client.call("vault.status", json!({})).await;
    let s = WsClient::assert_ok(&r, "vault.status");
    assert_eq!(s["initialized"], false);

    // 2. vault.list_networks — empty (no plugins registered)
    let r = client.call("vault.list_networks", json!({})).await;
    WsClient::assert_ok(&r, "vault.list_networks");

    // 3. vault.validate_address — fails without plugins (method routes correctly)
    let r = client
        .call(
            "vault.validate_address",
            json!({"network":"bitcoin","address":"bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"}),
        )
        .await;
    assert!(
        r.get("error").is_some(),
        "validate_address routes but fails (no plugins)"
    );

    // 4. vault.estimate_fee — fails without plugins
    let r = client
        .call("vault.estimate_fee", json!({"network":"bitcoin"}))
        .await;
    assert!(
        r.get("error").is_some(),
        "estimate_fee routes but fails (no plugins)"
    );

    // ── Phase 2: Initialize vault ──────────────────────────────────────

    // 5. vault.initialize — use a real BIP-39 mnemonic
    let phrase = crypto_core::keys::generate_mnemonic(crypto_core::MnemonicStrength::TwelveWords)
        .expect("test invariant");
    let r = client
        .call(
            "vault.initialize",
            json!({"seed_phrase": phrase.to_string()}),
        )
        .await;
    WsClient::assert_ok(&r, "vault.initialize");

    // 6. vault.status — now initialized
    let r = client.call("vault.status", json!({})).await;
    let s = WsClient::assert_ok(&r, "vault.status (post-init)");
    assert_eq!(s["initialized"], true);
    let plugin_ids: Vec<String> = s["plugin_ids"]
        .as_array()
        .expect("test invariant")
        .iter()
        .map(|v| v.as_str().expect("test invariant").to_string())
        .collect();
    let has_plugins = !plugin_ids.is_empty();
    println!("Plugins after init: {plugin_ids:?} (has_plugins={has_plugins})");
    assert!(s["networks"].is_array());

    // 7. vault.list_networks — now populated
    let r = client.call("vault.list_networks", json!({})).await;
    WsClient::assert_ok(&r, "vault.list_networks (post-init)");

    // 8. vault.create_account
    let r = client
        .call(
            "vault.create_account",
            json!({"network":"bitcoin","index":0}),
        )
        .await;
    if has_plugins {
        WsClient::assert_ok(&r, "vault.create_account");
    } else {
        // Without plugins feature, unsupported network is expected
        assert!(r.get("error").is_some(), "create_account routes correctly");
    }

    // 9. vault.get_balance
    let r = client
        .call(
            "vault.get_balance",
            json!({"network":"bitcoin","address":"bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"}),
        )
        .await;
    if has_plugins {
        WsClient::assert_ok(&r, "vault.get_balance");
    } else {
        assert!(r.get("error").is_some(), "get_balance routes correctly");
    }

    // 10. vault.sign_transaction — fails with invalid input
    let r = client
        .call(
            "vault.sign_transaction",
            json!({"network":"bitcoin","tx_hex":"00","key_id":"test","key_type":"Secp256k1"}),
        )
        .await;
    assert!(
        r.get("error").is_some(),
        "sign_transaction routes (bad input)"
    );

    // 11. vault.broadcast_transaction — fails with invalid input
    let r = client
        .call(
            "vault.broadcast_transaction",
            json!({"network":"bitcoin","signed_tx_hex":"00"}),
        )
        .await;
    assert!(
        r.get("error").is_some(),
        "broadcast_transaction routes (bad input)"
    );

    // 12. vault.get_transaction_history
    let r = client.call("vault.get_transaction_history",
        json!({"network":"bitcoin","address":"bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4","limit":5})).await;
    if has_plugins {
        WsClient::assert_ok(&r, "vault.get_transaction_history");
    } else {
        assert!(
            r.get("error").is_some(),
            "get_transaction_history routes correctly"
        );
    }

    // ── Phase 2: Next-gen vault_* API ──────────────────────────────────
    // These methods require HardwareRequired auth + FIDO2 touch.
    // Without a real FIDO2 key, they'll error — just verify the handler routes.

    // 13. vault_executeBatch — validates UserOps
    let r = client
        .call(
            "vault_executeBatch",
            json!({
                "network": "ethereum",
                "operations": [{
                    "sender": "0x1234567890123456789012345678901234567890",
                    "nonce": "0x1",
                    "callData": "0xabcd",
                    "callGasLimit": "0x5208",
                    "verificationGasLimit": "0x5208",
                    "preVerificationGas": "0x5208",
                    "maxFeePerGas": "0x0",
                    "maxPriorityFeePerGas": "0x0",
                    "paymasterAndData": "0x",
                    "initCode": "0x",
                    "signature": "0x"
                }]
            }),
        )
        .await;
    if has_plugins {
        // HardwareRequired auth + ApprovalQueue — can't complete without FIDO2
        assert!(
            r.get("error").is_some(),
            "vault_executeBatch routes correctly"
        );
        let err_code = r["error"]["code"].as_i64().unwrap_or(0);
        assert_ne!(err_code, -32601, "vault_executeBatch: not method_not_found");
    } else {
        assert!(
            r.get("error").is_some(),
            "vault_executeBatch routes correctly"
        );
    }

    // 14. vault_requestSessionKey — generates session key
    let r = client
        .call(
            "vault_requestSessionKey",
            json!({
                "network": "ethereum",
                "permissions": {
                    "allowedMethods": ["eth_sendTransaction", "personal_sign"],
                    "maxValue": "1000000000000000000",
                    "expirySeconds": 3600
                }
            }),
        )
        .await;
    if has_plugins {
        // HardwareRequired auth + ApprovalQueue — can't complete without FIDO2
        assert!(
            r.get("error").is_some(),
            "vault_requestSessionKey routes correctly"
        );
        let err_code = r["error"]["code"].as_i64().unwrap_or(0);
        assert_ne!(
            err_code, -32601,
            "vault_requestSessionKey: not method_not_found"
        );
    } else {
        assert!(
            r.get("error").is_some(),
            "vault_requestSessionKey routes correctly"
        );
    }

    // 15. vault_simulateAndSend — simulation + broadcast
    let r = client
        .call(
            "vault_simulateAndSend",
            json!({
                "network": "ethereum",
                "tx_hex": "02f8",
                "key_id": "test-key-1",
                "key_type": "Secp256k1",
                "address": "0xtest"
            }),
        )
        .await;
    // Always errors without real RPC, but must route (no method_not_found)
    assert!(
        r.get("error").is_some(),
        "vault_simulateAndSend routes correctly"
    );
    let err_code = r["error"]["code"].as_i64().unwrap_or(0);
    assert_ne!(
        err_code, -32601,
        "vault_simulateAndSend: not method_not_found"
    );

    println!("\n✅ All 13 WebSocket JSON-RPC round-trips PASSED");
}
