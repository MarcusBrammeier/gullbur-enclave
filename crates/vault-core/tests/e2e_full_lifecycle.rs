//! Comprehensive E2E test: full vault lifecycle via WebSocket IPC.
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

const PORT: u16 = 19993;

#[tokio::test(flavor = "multi_thread")]
async fn e2e_full_vault_lifecycle() {
    // ── 1. Start IPC server ──────────────────────────────────────────────
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

    // ── 2. Connect WebSocket + hello handshake ───────────────────────────
    let (ws, _) = connect_async(format!("ws://127.0.0.1:{PORT}"))
        .await
        .expect("test invariant");
    let (mut write, mut read) = ws.split();
    write
        .send(Message::Text(r#"{"type":"hello"}"#.into()))
        .await
        .expect("test invariant");
    let msg = tokio::time::timeout(Duration::from_secs(5), read.next())
        .await
        .expect("timeout")
        .expect("test invariant")
        .expect("test invariant");
    let v: serde_json::Value =
        serde_json::from_str(msg.to_text().expect("test invariant")).expect("test invariant");
    assert_eq!(v["type"], "session_key");
    println!("[e2e] Connected ✓");

    // ── Helper macro for RPC calls ───────────────────────────────────────
    macro_rules! rpc {
        ($method:expr, $params:tt) => {{
            let id: u64 = rand::random();
            let req = serde_json::json!({"jsonrpc":"2.0","method":$method,"params":$params,"id":id});
            write.send(Message::Text(req.to_string().into())).await.expect("test invariant");
            let msg = tokio::time::timeout(Duration::from_secs(30), read.next())
                .await.expect("rpc timeout").expect("test invariant").expect("test invariant");
            let p: serde_json::Value = serde_json::from_str(msg.to_text().expect("test invariant")).expect("test invariant");
            if let Some(e) = p.get("error") {
                panic!("RPC {} error: {}", $method, e["message"]);
            }
            p.get("result").cloned().unwrap_or(p)
        }};
    }

    // ── 3. Generate mnemonic ─────────────────────────────────────────────
    let r = rpc!("vault.generate_mnemonic", {});
    let mnemonic = r["mnemonic"].as_str().expect("test invariant").to_string();
    assert_eq!(mnemonic.split_whitespace().count(), 24);
    println!("[e2e] 1. Generated 24-word mnemonic ✓");

    // ── 4. Initialize vault ──────────────────────────────────────────────
    let r = rpc!("vault.initialize", {"seed_phrase": mnemonic, "passphrase": ""});
    assert!(r["initialized"].as_bool().expect("test invariant"));
    println!("[e2e] 2. Vault initialized ✓");

    // ── 5. Status ────────────────────────────────────────────────────────
    let s = rpc!("vault.status", {});
    assert!(s["initialized"].as_bool().expect("test invariant"));
    assert!(
        !s["plugin_ids"]
            .as_array()
            .expect("test invariant")
            .is_empty()
    );
    assert!(!s["networks"].as_array().expect("test invariant").is_empty());
    assert!(
        s["testnet_only"].as_bool().expect("test invariant"),
        "testnet-only should default on"
    );
    println!("[e2e] 3. Status OK ✓");

    // ── 5b. Disable testnet-only (this E2E exercises real mainnet ETH) ──
    let _ = rpc!("vault.set_testnet_only", {"enabled": false});
    println!("[e2e] 3b. Testnet-only disabled for mainnet lifecycle ✓");

    // ── 6. Create ETH account ────────────────────────────────────────────
    let r = rpc!("vault.create_account", {"network":"ethereum","index":0});
    let addr = r["address"].as_str().expect("test invariant").to_string();
    assert!(
        addr.starts_with("0x") && addr.len() == 42,
        "bad ETH address: {addr}"
    );
    println!("[e2e] 4. ETH account {addr} ✓");

    // ── 7. Create BTC account ────────────────────────────────────────────
    let r = rpc!("vault.create_account", {"network":"bitcoin","index":0});
    let btc = r["address"].as_str().expect("test invariant").to_string();
    println!("[e2e] 5. BTC account {btc} ✓");

    // ── 8. ETH balance (hits live RPC) ──────────────────────────────────
    let r = rpc!("vault.get_balance", {"network":"ethereum","address":addr});
    println!("[e2e] 6. ETH balance: {r}");

    // ── 9. Sepolia account ───────────────────────────────────────────────
    let _ = rpc!("vault.create_account", {"network":"sepolia","index":0});
    println!("[e2e] 7. Sepolia account ✓");

    // ──10. Fee estimate ──────────────────────────────────────────────────
    let r = rpc!("vault.estimate_fee", {"network":"ethereum"});
    println!("[e2e] 8. Fee estimate: {r}");

    // ── Cleanup ──────────────────────────────────────────────────────────
    sh.abort();
    let _ = std::fs::remove_file(&token);
    println!("[e2e] ✅ ALL 8 STEPS PASSED");
}
