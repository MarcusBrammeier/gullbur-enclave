//! Encrypted WebSocket IPC round-trip test (Phase 2.5).
//!
//! Exercises the AES-256-GCM isolation pattern end-to-end:
//!   1. Start IPC server with encryption enabled (default)
//!   2. Hello handshake → get session key
//!   3. Encrypt a request with the session key
//!   4. Send encrypted JSON-RPC → receive encrypted response
//!   5. Decrypt the response and verify the result
//!
//! Run: cargo test -p vault-core --test e2e_ipc_encrypted -- --test-threads=1

use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use auth_core::AuthManager;
use crypto_core::isolation;
use vault_core::approval::ApprovalQueue;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

const ENC_PORT: u16 = 19993;

#[tokio::test]
async fn e2e_encrypted_roundtrip() {
    // 1. Start IPC server with encryption enabled (default).
    let server = ipc_core::server::IpcServer::new(ENC_PORT).expect("create IPC server");
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
    let _handle = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 2. Connect and do hello handshake → get session key.
    let (ws, _) = connect_async(format!("ws://127.0.0.1:{ENC_PORT}"))
        .await
        .expect("WebSocket connect");
    let (mut write, mut read) = ws.split();
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
    let session_val: serde_json::Value = serde_json::from_str(&session_text).expect("valid json");
    assert_eq!(session_val["type"], "session_key");
    let key_hex = session_val["key"]
        .as_str()
        .expect("session key")
        .to_string();
    eprintln!("session key exchanged ({} chars)", key_hex.len());

    // 3. Encrypt a JSON-RPC request with the session key (AES-256-GCM).
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "vault.generate_mnemonic",
        "params": {},
        "id": 42,
    });
    let key_bytes: [u8; 32] = hex::decode(&key_hex)
        .expect("valid hex")
        .try_into()
        .expect("32 bytes");
    let enc_payload = isolation::encrypt(&key_bytes, &request).expect("encrypt");
    let wrapped = serde_json::json!({
        "__encrypted__": true,
        "__payload__": enc_payload,
    });
    write
        .send(Message::Text(wrapped.to_string().into()))
        .await
        .expect("send encrypted");

    // 4. Receive response — must be encrypted (__encrypted__: true).
    let resp_msg = tokio::time::timeout(Duration::from_secs(3), read.next())
        .await
        .expect("timeout")
        .expect("stream ended")
        .expect("ws error");
    let resp_text = match &resp_msg {
        Message::Text(t) => t.to_string(),
        o => panic!("expected text, got: {o:?}"),
    };
    let resp_val: serde_json::Value = serde_json::from_str(&resp_text).expect("valid json");
    assert!(
        resp_val.get("__encrypted__").and_then(|v| v.as_bool()) == Some(true),
        "response must be encrypted: {resp_text}"
    );

    // 5. Extract and decrypt the response.
    let ep = isolation::extract_encrypted(&resp_val).expect("extract payload");
    let decrypted = isolation::decrypt(&key_bytes, &ep).expect("decrypt");
    assert_eq!(decrypted["id"], 42, "response id must match");
    let mnemonic = decrypted["result"]["mnemonic"].as_str().expect("mnemonic");
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    assert_eq!(words.len(), 24, "24-word mnemonic, got {}", words.len());
    eprintln!("✅ Encrypted IPC round-trip: 24-word mnemonic via AES-256-GCM");
    let _ = std::fs::remove_file(&token_path);
}
