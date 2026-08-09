//! Tests disconnect-reconnect semantics: verify the server handles
//! a fresh WebSocket connection after one was closed mid-flow.
//! This simulates the Svelte wizard: connect → generateMnemonic → (close)
//! → connect → initialize.
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use auth_core::AuthManager;
use vault_core::approval::ApprovalQueue;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

const PORT: u16 = 19994;

async fn rpc_call<W, R>(
    write: &mut W,
    read: &mut R,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String>
where
    W: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    R: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let id: u64 = rand::random();
    let req = serde_json::json!({"jsonrpc":"2.0","method":method,"params":params,"id":id});
    write
        .send(Message::Text(req.to_string().into()))
        .await
        .map_err(|e| e.to_string())?;
    let msg = tokio::time::timeout(Duration::from_secs(15), read.next())
        .await
        .map_err(|_| "timeout".to_string())?
        .ok_or("closed".to_string())?
        .map_err(|e| e.to_string())?;
    let p: serde_json::Value =
        serde_json::from_str(msg.to_text().map_err(|_| "bin")?).expect("test invariant");
    if let Some(e) = p.get("error") {
        Err(e["message"].as_str().unwrap_or("rpc error").to_string())
    } else {
        Ok(p.get("result").cloned().unwrap_or(p))
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_disconnect_reconnect() {
    // Start server
    let server = ipc_core::server::IpcServer::new(PORT).expect("test invariant");
    let token = server.auth_token_path().to_path_buf();
    {
        let mut h = server.handler().await;
        let ph = Arc::new(RwLock::new(PluginHost::new()));
        let sd = Arc::new(RwLock::new(None::<zeroize::Zeroizing<Vec<u8>>>));
        let init = Arc::new(AtomicBool::new(false));
        let aq = Arc::new(RwLock::new(ApprovalQueue::new()));
        let am = Arc::new(AuthManager::new());
        let mn = Arc::new(RwLock::new(None));
        ipc_handlers::register_vault_handlers(&mut h, ph, sd, mn, init, aq, am);
    }
    let (sh, _ready) = server.run();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // ── Connection 1: hello + generate mnemonic ─────────────────────────
    let (ws, _) = connect_async(format!("ws://127.0.0.1:{PORT}"))
        .await
        .expect("test invariant");
    let (mut w1, mut r1) = ws.split();
    w1.send(Message::Text(r#"{"type":"hello"}"#.into()))
        .await
        .expect("test invariant");
    let msg = tokio::time::timeout(Duration::from_secs(5), r1.next())
        .await
        .expect("test invariant")
        .expect("test invariant")
        .expect("test invariant");
    let v: serde_json::Value =
        serde_json::from_str(msg.to_text().expect("test invariant")).expect("test invariant");
    assert_eq!(v["type"], "session_key");
    let r = rpc_call(
        &mut w1,
        &mut r1,
        "vault.generate_mnemonic",
        serde_json::json!({}),
    )
    .await
    .expect("test invariant");
    let mnemonic = r["mnemonic"].as_str().expect("test invariant").to_string();
    assert_eq!(mnemonic.split_whitespace().count(), 24);
    println!("[dr] Connection 1: generated mnemonic ✓");

    // Close connection 1
    w1.close().await.expect("test invariant");
    // Small delay to ensure server processes the close
    tokio::time::sleep(Duration::from_millis(100)).await;
    println!("[dr] Connection 1 closed ✓");

    // ── Connection 2: fresh session, call initialize ────────────────────
    let (ws, _) = connect_async(format!("ws://127.0.0.1:{PORT}"))
        .await
        .expect("test invariant");
    let (mut w2, mut r2) = ws.split();
    w2.send(Message::Text(r#"{"type":"hello"}"#.into()))
        .await
        .expect("test invariant");
    let msg = tokio::time::timeout(Duration::from_secs(5), r2.next())
        .await
        .expect("test invariant")
        .expect("test invariant")
        .expect("test invariant");
    let v: serde_json::Value =
        serde_json::from_str(msg.to_text().expect("test invariant")).expect("test invariant");
    assert_eq!(v["type"], "session_key");
    println!("[dr] Connection 2: new session key ✓");

    // Initialize with the mnemonic from connection 1
    let r = rpc_call(
        &mut w2,
        &mut r2,
        "vault.initialize",
        serde_json::json!({"seed_phrase": mnemonic, "passphrase": ""}),
    )
    .await
    .expect("test invariant");
    assert!(
        r["initialized"].as_bool().expect("test invariant"),
        "initialize should succeed on fresh connection"
    );
    println!("[dr] Connection 2: initialized vault ✓");

    // Verify status
    let r = rpc_call(&mut w2, &mut r2, "vault.status", serde_json::json!({}))
        .await
        .expect("test invariant");
    assert!(r["initialized"].as_bool().expect("test invariant"));
    assert!(
        !r["plugin_ids"]
            .as_array()
            .expect("test invariant")
            .is_empty()
    );
    assert!(!r["networks"].as_array().expect("test invariant").is_empty());
    println!("[dr] Connection 2: status OK ✓");

    w2.close().await.expect("test invariant");
    sh.abort();
    let _ = std::fs::remove_file(&token);
    println!("[dr] ✅ disconnect → reconnect → initialize PASSED");
}
