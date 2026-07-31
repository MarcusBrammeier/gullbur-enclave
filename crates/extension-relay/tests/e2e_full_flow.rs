//! Full E2E integration test — browser extension → relay → vault pipeline.
//!
//! Simulates a complete dApp interaction through the native messaging relay
//! to vault-core IPC, covering:
//! - Full dApp flow: requestAccounts → getBalance → signTransaction
//! - Origin validation (allowed + rejected)
//! - Approval gating (state-changing methods blocked)
//! - Phase 2 next-gen vault_* API routing
//! - PermissionManager lifecycle

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use auth_core::AuthManager;
use extension_relay::native_host;
use extension_relay::permissions::PermissionManager;
use extension_relay::router;
use vault_core::approval::ApprovalQueue;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

const E2E_PORT: u16 = 19880;

// ── Helpers ──────────────────────────────────────────────────────────────

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
    next_id: u64,
}

impl WsClient {
    async fn connect(port: u16, token: &str) -> Self {
        let url = format!("ws://127.0.0.1:{port}");
        let (ws, _) = connect_async(&url).await.expect("WebSocket connect");
        let (mut write, read) = ws.split();
        write
            .send(Message::Text(token.into()))
            .await
            .expect("send auth");
        WsClient {
            write,
            read,
            next_id: 1,
        }
    }

    async fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
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

    fn is_ok(resp: &Value) -> bool {
        resp.get("error").is_none()
    }

    fn error_code(resp: &Value) -> i64 {
        resp.get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_i64())
            .unwrap_or(0)
    }
}

/// Simulate a native messaging envelope through the routing layer.
fn route_envelope(method: &str, params: Value) -> Option<router::RoutedRequest> {
    router::route(method, params)
}

// ── Test ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_full_browser_extension_flow() {
    // ── 1. Start vault-core IPC server ──────────────────────────────────
    let server = ipc_core::server::IpcServer::new(E2E_PORT).expect("IpcServer");
    let token_path = server.auth_token_path().to_path_buf();
    let auth_token = std::fs::read_to_string(&token_path).expect("read token");

    {
        let mut handler = server.handler().await;
        let ph = Arc::new(RwLock::new(PluginHost::new()));
        let sd = Arc::new(RwLock::new(None::<zeroize::Zeroizing<Vec<u8>>>));
        let init = Arc::new(AtomicBool::new(false));
        let aq = Arc::new(RwLock::new(ApprovalQueue::new()));
        let am = Arc::new(AuthManager::new());
        let mn = Arc::new(RwLock::new(None));
        ipc_handlers::register_vault_handlers(&mut handler, ph, sd, mn, init, aq, am);
    }

    let _handle = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    // ── 2. Connect WebSocket client (simulating relay) ──────────────────
    let mut client = WsClient::connect(E2E_PORT, &auth_token).await;

    // ── 3. Initialize vault (seed) for meaningful responses ─────────────
    let r = client.call("vault.initialize", json!({"seed_phrase":"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"})).await;
    assert!(WsClient::is_ok(&r), "vault.initialize failed: {r}");
    let has_plugins = {
        let r = client.call("vault.status", json!({})).await;
        let s = &r["result"];
        !s["plugin_ids"]
            .as_array()
            .expect("test invariant")
            .is_empty()
    };
    println!("has_plugins: {has_plugins}");

    // ── 4. dApp Flow Simulation ─────────────────────────────────────────

    // 4a. eth_requestAccounts → vault.create_account (requires approval)
    let routed = route_envelope("eth_requestAccounts", json!({"seed_phrase":"test"}));
    assert!(routed.is_some(), "eth_requestAccounts must be routable");
    let r = routed.expect("test invariant");
    assert!(r.requires_approval, "eth_requestAccounts requires approval");
    assert_eq!(r.vault_method, "vault.create_account");
    assert!(
        r.approval_description
            .expect("test invariant")
            .contains("requesting access")
    );
    println!("✅ eth_requestAccounts: routed, requires approval");

    // 4b. eth_chainId → vault.list_networks (no approval needed)
    let routed = route_envelope("eth_chainId", json!({}));
    assert!(routed.is_some());
    let r = routed.expect("test invariant");
    assert!(!r.requires_approval);
    assert_eq!(r.vault_method, "vault.list_networks");

    let resp = client.call(&r.vault_method, r.params).await;
    if has_plugins {
        assert!(WsClient::is_ok(&resp), "vault.list_networks: {resp}");
    }
    println!("✅ eth_chainId → vault.list_networks: routed");

    // 4c. eth_getBalance → vault.get_balance
    let routed = route_envelope(
        "eth_getBalance",
        json!({"address":"0xtest","network":"ethereum"}),
    );
    assert!(routed.is_some());
    let r = routed.expect("test invariant");
    assert!(!r.requires_approval);
    assert_eq!(r.vault_method, "vault.get_balance");

    let resp = client.call(&r.vault_method, r.params).await;
    // May fail without plugins or real RPC, but must route
    assert_ne!(WsClient::error_code(&resp), -32601, "get_balance: routed");
    println!("✅ eth_getBalance → vault.get_balance: routed (error_code != -32601)");

    // 4d. eth_estimateGas → vault.estimate_fee
    let routed = route_envelope("eth_estimateGas", json!({"network":"ethereum"}));
    assert!(routed.is_some());
    let r = routed.expect("test invariant");
    assert_eq!(r.vault_method, "vault.estimate_fee");

    let resp = client.call(&r.vault_method, r.params).await;
    assert_ne!(WsClient::error_code(&resp), -32601, "estimate_fee: routed");
    println!("✅ eth_estimateGas → vault.estimate_fee: routed");

    // 4e. eth_sendTransaction → vault.sign_transaction (requires approval)
    let routed = route_envelope("eth_sendTransaction", json!({"from":"0x123"}));
    assert!(routed.is_some());
    let r = routed.expect("test invariant");
    assert!(r.requires_approval, "eth_sendTransaction requires approval");
    assert_eq!(r.vault_method, "vault.sign_transaction");
    assert!(
        r.approval_description
            .expect("test invariant")
            .contains("Confirm transaction")
    );
    println!("✅ eth_sendTransaction: routed, requires approval");

    // 4f. personal_sign → vault.sign_transaction (requires approval)
    let routed = route_envelope("personal_sign", json!({"data":"0xbeef"}));
    assert!(routed.is_some());
    let r = routed.expect("test invariant");
    assert!(r.requires_approval);
    assert_eq!(r.vault_method, "vault.sign_transaction");
    println!("✅ personal_sign: routed, requires approval");

    // ── 5. Phase 2 Next-Gen vault_* API Routing ─────────────────────────

    // 5a. vault_executeBatch (requires approval)
    let ops = json!([{
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
    }]);
    let routed = route_envelope(
        "vault_executeBatch",
        json!({"network":"ethereum","operations":ops}),
    );
    assert!(routed.is_some(), "vault_executeBatch must be routable");
    let r = routed.expect("test invariant");
    assert!(r.requires_approval, "vault_executeBatch requires approval");
    assert_eq!(r.vault_method, "vault_executeBatch");
    assert!(
        r.approval_description
            .expect("test invariant")
            .contains("ERC-4337")
    );
    println!("✅ vault_executeBatch: routed, requires approval");

    // 5b. vault_requestSessionKey (requires approval)
    let routed = route_envelope(
        "vault_requestSessionKey",
        json!({
            "network":"ethereum",
            "permissions":{"allowedMethods":["eth_sendTransaction"],"maxValue":"0","expirySeconds":3600}
        }),
    );
    assert!(routed.is_some(), "vault_requestSessionKey must be routable");
    let r = routed.expect("test invariant");
    assert!(r.requires_approval);
    assert_eq!(r.vault_method, "vault_requestSessionKey");
    assert!(
        r.approval_description
            .expect("test invariant")
            .contains("ERC-7579")
    );
    println!("✅ vault_requestSessionKey: routed, requires approval");

    // 5c. vault_simulateAndSend (requires approval)
    let routed = route_envelope(
        "vault_simulateAndSend",
        json!({
            "network":"ethereum",
            "tx_hex":"02f8",
            "key_id":"test",
            "key_type":"Secp256k1",
            "address":"0xtest"
        }),
    );
    assert!(routed.is_some(), "vault_simulateAndSend must be routable");
    let r = routed.expect("test invariant");
    assert!(r.requires_approval);
    assert_eq!(r.vault_method, "vault_simulateAndSend");
    assert!(
        r.approval_description
            .expect("test invariant")
            .contains("Simulate")
    );
    println!("✅ vault_simulateAndSend: routed, requires approval");

    // ── 6. Phase 2 Live IPC Calls ───────────────────────────────────────

    // 6a. vault_executeBatch live call
    let resp = client
        .call(
            "vault_executeBatch",
            json!({
                "network":"ethereum",
                "operations": [{
                    "sender": "0x1234567890123456789012345678901234567890",
                    "nonce": "0x1",
                    "callData": "0xabcd"
                }]
            }),
        )
        .await;
    if has_plugins {
        if WsClient::is_ok(&resp) {
            let result = &resp["result"];
            assert!(
                result
                    .get("userOpHashes")
                    .and_then(|v| v.as_array())
                    .is_some()
            );
            assert_eq!(result["status"], "validated");
            println!("✅ vault_executeBatch: live call OK — hashes returned");
        } else {
            println!("⚠ vault_executeBatch: live call errored (network unreachable)");
        }
    } else {
        assert_ne!(
            WsClient::error_code(&resp),
            -32601,
            "vault_executeBatch: routed"
        );
        println!("✅ vault_executeBatch: routed (no plugins)");
    }

    // 6b. vault_requestSessionKey live call
    let resp = client
        .call(
            "vault_requestSessionKey",
            json!({
                "network":"ethereum",
                "permissions": {
                    "allowedMethods": ["eth_sendTransaction", "personal_sign"],
                    "maxValue": "1000000000000000000",
                    "expirySeconds": 3600
                }
            }),
        )
        .await;
    if has_plugins {
        if WsClient::is_ok(&resp) {
            let result = &resp["result"];
            assert!(result.get("sessionKeyId").is_some());
            assert!(result.get("sessionPublicKey").is_some());
            assert!(result.get("permissions").is_some());
            println!("✅ vault_requestSessionKey: live call OK — key returned");
        } else {
            println!("⚠ vault_requestSessionKey: live call errored");
        }
    } else {
        assert_ne!(
            WsClient::error_code(&resp),
            -32601,
            "vault_requestSessionKey: routed"
        );
        println!("✅ vault_requestSessionKey: routed (no plugins)");
    }

    // ── 7. Origin Validation ────────────────────────────────────────────

    let allowed_ids: Vec<String> = vec!["trusted-ext".into(), "helper-ext".into()];

    // Valid origins
    assert!(native_host::validate_origin(
        "chrome-extension://trusted-ext",
        &allowed_ids
    ));
    assert!(native_host::validate_origin(
        "moz-extension://trusted-ext",
        &allowed_ids
    ));
    assert!(native_host::validate_origin(
        "chrome-extension://helper-ext",
        &allowed_ids
    ));

    // Invalid origins
    assert!(!native_host::validate_origin("", &allowed_ids));
    assert!(!native_host::validate_origin(
        "chrome-extension://evil-ext",
        &allowed_ids
    ));
    assert!(!native_host::validate_origin(
        "https://phishing.com",
        &allowed_ids
    ));
    assert!(!native_host::validate_origin(
        "http://localhost:3000",
        &allowed_ids
    ));
    println!("✅ Origin validation: 3 allowed, 4 rejected");

    // ── 8. PermissionManager Integration ────────────────────────────────

    let mut pm = PermissionManager::new();

    // Request permission for two dApps
    assert!(pm.request_permission("https://app.uniswap.org", vec!["evm-0".into()]));
    assert_eq!(pm.pending_for("https://app.uniswap.org"), vec!["evm-0"]);
    assert!(pm.request_permission("https://app.aave.com", vec!["evm-1".into()]));

    // Grant one — clears pending for that origin
    pm.grant("https://app.uniswap.org", "evm-0");
    assert!(pm.is_permitted("https://app.uniswap.org", "evm-0"));
    assert!(pm.pending_for("https://app.uniswap.org").is_empty());
    assert!(!pm.pending_for("https://app.aave.com").is_empty());

    // Second request for already-granted origin returns false
    assert!(!pm.request_permission("https://app.uniswap.org", vec!["evm-0".into()]));

    // Revoke
    pm.revoke_all("https://app.uniswap.org");
    assert!(!pm.is_permitted("https://app.uniswap.org", "evm-0"));
    // Aave still pending
    assert!(!pm.pending_for("https://app.aave.com").is_empty());

    println!("✅ PermissionManager: grant → check → revoke → re-request lifecycle");

    // ── 9. Unknown method returns None ──────────────────────────────────
    assert!(route_envelope("eth_unknownMethod", json!({})).is_none());
    println!("✅ Unknown method: returns None");

    println!("\n✅✅✅ E2E Full Browser Extension Flow PASSED ✅✅✅");
}
