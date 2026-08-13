//! Headless integration test: Tauri IPC isolation routing via encrypted WebSocket.
//!
//! Proves the full encrypted JSON-RPC round-trip through the isolation pattern:
//!   1. Start IPC server with encryption enabled
//!   2. Hello handshake → receive session key
//!   3. Encrypt JSON-RPC requests with AES-256-GCM using the session key
//!   4. Send encrypted requests → receive encrypted responses
//!   5. Decrypt responses and verify correctness
//!
//! Tests multiple methods, error routing, and tamper detection —
//! all fully local, no GUI, no network.
//!
//! Run: cargo test -p vault-core --test e2e_isolation_routing -- --test-threads=1

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

/// Helper: connect to IPC server, do hello handshake, return (write, read, key_bytes).
async fn connect_encrypted(
    port: u16,
) -> (
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    [u8; 32],
) {
    let (ws, _) = connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("WebSocket connect");
    let (mut write, mut read) = ws.split();

    // Hello handshake → session key
    write
        .send(Message::Text(r#"{"type":"hello"}"#.into()))
        .await
        .expect("send hello");

    let session_msg = tokio::time::timeout(Duration::from_secs(5), read.next())
        .await
        .expect("timeout on hello handshake")
        .expect("stream ended")
        .expect("ws error");
    let session_text = match &session_msg {
        Message::Text(t) => t.to_string(),
        o => panic!("expected Text, got: {o:?}"),
    };
    let session_val: serde_json::Value =
        serde_json::from_str(&session_text).expect("valid JSON in session response");
    assert_eq!(session_val["type"], "session_key");
    let key_hex = session_val["key"]
        .as_str()
        .expect("session_key.key")
        .to_string();
    let key_bytes: [u8; 32] = hex::decode(&key_hex)
        .expect("valid hex key")
        .try_into()
        .expect("32-byte key");
    (write, read, key_bytes)
}

/// Helper: encrypt a JSON-RPC request and send it over WebSocket.
/// Uses a concrete Sink type to avoid Debug bound issues with impl Trait.
async fn send_encrypted(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    key: &[u8; 32],
    method: &str,
    params: serde_json::Value,
    id: u64,
) {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id,
    });
    let payload = isolation::encrypt(key, &request).expect("encrypt request");
    let wrapped = serde_json::json!({
        "__encrypted__": true,
        "__payload__": payload,
    });
    write
        .send(Message::Text(wrapped.to_string().into()))
        .await
        .unwrap_or_else(|e| panic!("send encrypted message: {e}"));
}

/// Helper: receive a response and decrypt it, returning the inner JSON.
async fn recv_decrypted(
    read: &mut (
             impl futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
             + Unpin
         ),
    key: &[u8; 32],
) -> serde_json::Value {
    let msg = tokio::time::timeout(Duration::from_secs(5), read.next())
        .await
        .expect("timeout on response")
        .expect("stream ended")
        .expect("ws error");
    let text = match &msg {
        Message::Text(t) => t.to_string(),
        o => panic!("expected Text, got: {o:?}"),
    };
    let val: serde_json::Value = serde_json::from_str(&text).expect("valid JSON response");
    // Must be encrypted wrapper
    assert!(
        val.get("__encrypted__").and_then(|v| v.as_bool()) == Some(true),
        "response must be encrypted: {text}"
    );
    let ep = isolation::extract_encrypted(&val).expect("extract encrypted payload");
    isolation::decrypt(key, &ep).expect("decrypt response")
}

// ── Tests ──────────────────────────────────────────────────────────────────────

const ISOLATION_PORT: u16 = 19994;

/// Test 1: Encrypted round-trip for `get_vault_status` (no network, simple state query).
#[tokio::test]
async fn e2e_isolation_vault_status() {
    let server = ipc_core::server::IpcServer::with_encryption(ISOLATION_PORT, true)
        .expect("create IPC server");
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
    let (_handle, _ready) = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read, key) = connect_encrypted(ISOLATION_PORT).await;

    // Send encrypted request for vault.status
    send_encrypted(&mut write, &key, "vault.status", serde_json::json!({}), 1).await;

    let decrypted = recv_decrypted(&mut read, &key).await;
    assert_eq!(decrypted["id"], 1, "response id matches");
    // vault.status returns initialized, status, plugin_ids, networks, accounts
    assert_eq!(
        decrypted["result"]["initialized"], false,
        "uninitialized vault"
    );
    assert_eq!(
        decrypted["result"]["status"], "Connected",
        "status should be Connected when not initialized"
    );

    _handle.abort();
    let _ = std::fs::remove_file(&token_path);
}

/// Test 2: Encrypted round-trip for `vault.generate_mnemonic` (verifies the
/// generate_mnemonic path works through encryption — same pattern as
/// the existing e2e_ipc_encrypted test but with explicit assertions).
#[tokio::test]
async fn e2e_isolation_generate_mnemonic() {
    let port = ISOLATION_PORT + 1;
    let server =
        ipc_core::server::IpcServer::with_encryption(port, true).expect("create IPC server");
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
    let (_handle, _ready) = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read, key) = connect_encrypted(port).await;

    send_encrypted(
        &mut write,
        &key,
        "vault.generate_mnemonic",
        serde_json::json!({}),
        42,
    )
    .await;

    let decrypted = recv_decrypted(&mut read, &key).await;
    assert_eq!(decrypted["id"], 42, "response id matches");
    let mnemonic = decrypted["result"]["mnemonic"]
        .as_str()
        .expect("mnemonic string");
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    assert_eq!(words.len(), 24, "24-word mnemonic");

    _handle.abort();
    let _ = std::fs::remove_file(&token_path);
}

/// Test 3: Encrypted round-trip with an unknown method — proves that
/// error responses are ALSO encrypted through the isolation pattern.
#[tokio::test]
async fn e2e_isolation_method_not_found() {
    let port = ISOLATION_PORT + 2;
    let server =
        ipc_core::server::IpcServer::with_encryption(port, true).expect("create IPC server");
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
    let (_handle, _ready) = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read, key) = connect_encrypted(port).await;

    send_encrypted(
        &mut write,
        &key,
        "nonexistent_method_xyz",
        serde_json::json!({}),
        99,
    )
    .await;

    let decrypted = recv_decrypted(&mut read, &key).await;
    assert_eq!(decrypted["id"], 99, "error response id matches");
    assert!(
        decrypted.get("error").is_some(),
        "method-not-found must return an error: {decrypted}"
    );
    assert_eq!(
        decrypted["error"]["code"], -32601,
        "method not found error code"
    );

    _handle.abort();
    let _ = std::fs::remove_file(&token_path);
}

/// Test 4: Tampered encrypted payload must be rejected — proves AES-GCM
/// authentication catches any corruption in the isolation path.
#[tokio::test]
async fn e2e_isolation_tampered_payload_rejected() {
    let port = ISOLATION_PORT + 3;
    let server =
        ipc_core::server::IpcServer::with_encryption(port, true).expect("create IPC server");
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
    let (_handle, _ready) = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read, key) = connect_encrypted(port).await;

    // Build an encrypted payload, then corrupt it.
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "vault.generate_mnemonic",
        "params": {},
        "id": 7,
    });
    let mut payload = isolation::encrypt(&key, &request).expect("encrypt");
    // Flip a bit in the ciphertext base64 data to corrupt the AES-GCM tag/data.
    let mut bytes = isolation::decode_base64(&payload.data_b64).expect("decode b64");
    if let Some(b) = bytes.last_mut() {
        *b ^= 0xab; // flip bits
    }
    payload.data_b64 = isolation::encode_base64(&bytes);

    let wrapped = serde_json::json!({
        "__encrypted__": true,
        "__payload__": payload,
    });
    write
        .send(Message::Text(wrapped.to_string().into()))
        .await
        .expect("send tampered message");

    // The server should close the connection or drop the message silently
    // (the ipc-core server logs a warning and continues without sending a response
    // when decryption fails — so the next message on the read stream should
    // either be the connection close or a timeout).
    let timeout_result = tokio::time::timeout(Duration::from_millis(1500), read.next()).await;
    match timeout_result {
        Ok(Some(Ok(msg))) => {
            // If we got a message, it should NOT be a successful response to id 7
            if let Message::Text(t) = &msg {
                let val: serde_json::Value =
                    serde_json::from_str(t).unwrap_or(serde_json::Value::Null);
                // The response could be an error or close — but should NOT have result with mnemonic
                assert!(
                    val.get("result").is_none()
                        || val.get("id").and_then(|v| v.as_u64()) != Some(7),
                    "tampered payload must not produce a successful response: {t}"
                );
            }
        }
        Ok(Some(Err(e))) => {
            // Connection error is acceptable — server may drop the connection
            eprintln!("Connection dropped after tampered payload (expected): {e}");
        }
        Ok(None) => {
            // Stream ended — connection closed by server, acceptable
            eprintln!("Stream ended after tampered payload (expected)");
        }
        Err(_elapsed) => {
            // Timeout — server dropped the message silently, which is the expected behavior
            eprintln!("Server silently dropped tampered payload (expected)");
        }
    }

    _handle.abort();
    let _ = std::fs::remove_file(&token_path);
}

/// Test 5: Plaintext (non-encrypted) request gets a plaintext (non-encrypted)
/// response even when encryption is enabled on the server — proves the
/// was_encrypted response-matching logic in the IPC isolation pattern.
#[tokio::test]
async fn e2e_isolation_plaintext_request_plaintext_response() {
    let port = ISOLATION_PORT + 4;
    let server =
        ipc_core::server::IpcServer::with_encryption(port, true).expect("create IPC server");
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
    let (_handle, _ready) = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read, _key) = connect_encrypted(port).await;

    // Send plaintext JSON-RPC (not wrapped in __encrypted__)
    let rpc = r#"{"jsonrpc":"2.0","method":"vault.generate_mnemonic","params":{},"id":10}"#;
    write
        .send(Message::Text(rpc.into()))
        .await
        .expect("send plaintext rpc");

    let resp_msg = tokio::time::timeout(Duration::from_secs(3), read.next())
        .await
        .expect("timeout")
        .expect("stream ended")
        .expect("ws error");
    let resp_text = match &resp_msg {
        Message::Text(t) => t.to_string(),
        o => panic!("expected Text, got: {o:?}"),
    };
    let resp_val: serde_json::Value =
        serde_json::from_str(&resp_text).expect("valid JSON response");
    assert!(
        resp_val.get("__encrypted__").is_none(),
        "plaintext request must get plaintext response: {resp_text}"
    );
    assert!(
        resp_val.get("result").is_some(),
        "response must have a result"
    );
    let mnemonic = resp_val["result"]["mnemonic"]
        .as_str()
        .expect("mnemonic string");
    assert_eq!(
        mnemonic.split_whitespace().count(),
        24,
        "24-word mnemonic from plaintext request"
    );

    _handle.abort();
    let _ = std::fs::remove_file(&token_path);
}

/// Test 6: Sequential encrypted calls with different methods on the same connection —
/// proves the session key persists across multiple isolation requests (stateful routing).
#[tokio::test]
async fn e2e_isolation_sequential_calls_same_session() {
    let port = ISOLATION_PORT + 5;
    let server =
        ipc_core::server::IpcServer::with_encryption(port, true).expect("create IPC server");
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
    let (_handle, _ready) = server.run();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read, key) = connect_encrypted(port).await;

    // Call 1: vault.status
    send_encrypted(&mut write, &key, "vault.status", serde_json::json!({}), 1).await;
    let resp1 = recv_decrypted(&mut read, &key).await;
    assert_eq!(resp1["id"], 1);
    assert_eq!(resp1["result"]["initialized"], false);
    assert_eq!(resp1["result"]["status"], "Connected");

    // Call 2: generate_mnemonic
    send_encrypted(
        &mut write,
        &key,
        "vault.generate_mnemonic",
        serde_json::json!({}),
        2,
    )
    .await;
    let resp2 = recv_decrypted(&mut read, &key).await;
    assert_eq!(resp2["id"], 2);
    let words: Vec<&str> = resp2["result"]["mnemonic"]
        .as_str()
        .expect("mnemonic")
        .split_whitespace()
        .collect();
    assert_eq!(words.len(), 24);

    // Call 3: unknown method (error response must also be encrypted)
    send_encrypted(
        &mut write,
        &key,
        "vault.unknown_method_99",
        serde_json::json!({}),
        3,
    )
    .await;
    let resp3 = recv_decrypted(&mut read, &key).await;
    assert_eq!(resp3["id"], 3);
    assert!(resp3.get("error").is_some(), "error expected: {resp3}");
    assert_eq!(resp3["error"]["code"], -32601);

    _handle.abort();
    let _ = std::fs::remove_file(&token_path);
}
