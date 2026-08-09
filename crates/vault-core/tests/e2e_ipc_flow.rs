//! Full end-to-end test of the IPC server non-encrypted handshake + RPC.
//! This tests the exact flow the Svelte frontend uses.
//! Run with: cargo test --test e2e_ipc_flow -p vault-core --features plugins
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use auth_core::AuthManager;
use vault_core::approval::ApprovalQueue;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

const TEST_PORT: u16 = 19990;

#[tokio::test]
async fn e2e_plaintext_connect_and_generate_mnemonic() {
    let server =
        ipc_core::server::IpcServer::new(TEST_PORT).expect("create IPC server");
    let token_path = server.auth_token_path().to_path_buf();

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

    let (handle, _ready) = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (ws, _) = connect_async(format!("ws://127.0.0.1:{TEST_PORT}"))
        .await
        .expect("WebSocket connect");
    let (mut write, mut read) = ws.split();

    // Hello handshake (loopback trust)
    write
        .send(Message::Text(r#"{"type":"hello"}"#.into()))
        .await
        .expect("send hello");

    let session_msg = tokio::time::timeout(Duration::from_secs(3), read.next())
        .await
        .expect("timeout")
        .expect("stream ended")
        .expect("ws error");
    let session_text = match &session_msg {
        Message::Text(t) => t.to_string(),
        _ => panic!("expected text, got: {session_msg:?}"),
    };
    let session_val: serde_json::Value =
        serde_json::from_str(&session_text).expect("test invariant");
    assert_eq!(session_val["type"], "session_key");

    // Plaintext JSON-RPC call — server must NOT encrypt the response
    let rpc = r#"{"jsonrpc":"2.0","method":"vault.generate_mnemonic","params":{},"id":1}"#;
    write
        .send(Message::Text(rpc.into()))
        .await
        .expect("send rpc");

    let resp_msg = tokio::time::timeout(Duration::from_secs(3), read.next())
        .await
        .expect("timeout")
        .expect("stream ended")
        .expect("ws error");
    let resp_text = match &resp_msg {
        Message::Text(t) => t.to_string(),
        o => panic!("expected text, got: {o:?}"),
    };
    let resp_val: serde_json::Value = serde_json::from_str(&resp_text).expect("test invariant");

    assert!(
        resp_val.get("__encrypted__").is_none(),
        "plain request must get plain response: {resp_text}"
    );
    assert!(
        resp_val.get("result").is_some(),
        "response should have result: {resp_text}"
    );
    let mnemonic = resp_val["result"]["mnemonic"].as_str().expect("mnemonic");
    assert!(
        mnemonic.split_whitespace().count() >= 12,
        "at least 12 words: {mnemonic}"
    );

    handle.abort();
    let _ = std::fs::remove_file(&token_path);
}
