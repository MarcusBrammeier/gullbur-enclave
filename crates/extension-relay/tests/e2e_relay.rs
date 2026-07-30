//! Integration test: native messaging envelope → WebSocket round-trip.
//!
//! Tests the full relay pipeline: envelope parsing → method routing →
//! vault-core IPC → response formatting, all in-process (no subprocess).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};

use extension_relay::native_host;
use extension_relay::router;
use auth_core::AuthManager;
use vault_core::approval::ApprovalQueue;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

const RELAY_PORT: u16 = 19879;

#[tokio::test]
async fn e2e_relay_envelope_to_websocket() {
    // ── 1. Start vault-core IpcServer ──────────────────────────────────
    let server = ipc_core::server::IpcServer::new(RELAY_PORT).expect("IpcServer");
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

    // ── 2. Connect WebSocket client ────────────────────────────────────
    let url = format!("ws://127.0.0.1:{RELAY_PORT}");
    let (ws, _) = connect_async(&url).await.expect("connect");
    let (mut ws_write, mut ws_read) = ws.split();

    // Auth
    ws_write.send(Message::Text(auth_token.into())).await.expect("auth");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // ── 3. Simulate native messaging envelopes ─────────────────────────

    // Test 1: vault.status via eth_accounts routing
    let routed = router::route("eth_accounts", json!({})).expect("test invariant");
    assert_eq!(routed.vault_method, "vault.status");
    assert!(!routed.requires_approval);

    let rpc_id = 1u64;
    let rpc = json!({"jsonrpc":"2.0","method":"vault.status","params":{},"id":rpc_id});
    ws_write.send(Message::Text(rpc.to_string().into())).await.expect("send");

    let resp = read_ws_response(&mut ws_read, rpc_id).await;
    assert!(resp.get("error").is_none(), "vault.status: {resp}");
    assert_eq!(resp["result"]["initialized"], false);

    // Test 2: vault.initialize via eth_requestAccounts (requires approval)
    let routed = router::route("eth_requestAccounts", json!({"seed_phrase":"test"})).expect("test invariant");
    assert!(routed.requires_approval);
    assert_eq!(routed.vault_method, "vault.create_account");

    // Test 3: eth_chainId → vault.list_networks
    let routed = router::route("eth_chainId", json!({})).expect("test invariant");
    assert_eq!(routed.vault_method, "vault.list_networks");
    assert!(!routed.requires_approval);

    let rpc_id = 2u64;
    let rpc = json!({"jsonrpc":"2.0","method":"vault.list_networks","params":{},"id":rpc_id});
    ws_write.send(Message::Text(rpc.to_string().into())).await.expect("send");

    let resp = read_ws_response(&mut ws_read, rpc_id).await;
    assert!(resp.get("error").is_none(), "vault.list_networks: {resp}");

    // Test 4: eth_estimateGas → vault.estimate_fee (should route, may fail without plugins)
    let routed = router::route("eth_estimateGas", json!({"network":"ethereum"})).expect("test invariant");
    assert_eq!(routed.vault_method, "vault.estimate_fee");
    assert!(!routed.requires_approval);

    let rpc_id = 3u64;
    let rpc = json!({"jsonrpc":"2.0","method":"vault.estimate_fee","params":{"network":"ethereum"},"id":rpc_id});
    ws_write.send(Message::Text(rpc.to_string().into())).await.expect("send");

    let resp = read_ws_response(&mut ws_read, rpc_id).await;
    // May fail (no plugins), but should route
    if resp.get("error").is_some() {
        assert_ne!(resp["error"]["code"], -32601, "estimate_fee routed");
    }

    // Test 5: eth_sendTransaction → approval required
    let routed = router::route("eth_sendTransaction", json!({"from":"0x123"})).expect("test invariant");
    assert!(routed.requires_approval);
    assert_eq!(routed.vault_method, "vault.sign_transaction");

    // Test 6: personal_sign → approval required
    let routed = router::route("personal_sign", json!({"data":"0xbeef"})).expect("test invariant");
    assert!(routed.requires_approval);
    assert_eq!(routed.vault_method, "vault.sign_transaction");

    // Test 7: origin validation
    assert!(native_host::validate_origin(
        "chrome-extension://abcdef",
        &["abcdef".into()]
    ));
    assert!(!native_host::validate_origin(
        "https://evil.com",
        &["abcdef".into()]
    ));

    println!("✅ E2E relay envelope-to-WebSocket round-trips passed");
}

async fn read_ws_response(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    expected_id: u64,
) -> Value {
    loop {
        match read.next().await {
            Some(Ok(Message::Text(text))) => {
                let v: Value = serde_json::from_str(&text).expect("test invariant");
                if v.get("id").and_then(|i| i.as_u64()) == Some(expected_id) {
                    return v;
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => panic!("WS error: {e}"),
            None => panic!("WS closed"),
        }
    }
}
