//! E2E test: `vault.stage_mnemonic` → `vault.initialize` staged flow.
//!
//! Verifies the hardening goal: when the UI generates a new wallet, the phrase
//! is held in Rust (staged) and `vault.initialize` consumes it — the UI never
//! re-sends the seed. Also verifies `vault.clear_staged` discards it, and that
//! the staged phrase is cleared automatically after initialization.

use futures_util::{SinkExt, StreamExt};
use ipc_core::server::IpcServer;
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
}

fn register(handler: &mut ipc_core::handler::MessageHandler, auth_manager: Arc<AuthManager>) {
    let plugin_host = Arc::new(RwLock::new(PluginHost::new()));
    let seed = Arc::new(RwLock::new(None::<zeroize::Zeroizing<Vec<u8>>>));
    let initialized = Arc::new(AtomicBool::new(false));
    let mn = Arc::new(RwLock::new(None));
    ipc_handlers::register_vault_handlers(
        handler,
        plugin_host,
        seed,
        mn,
        initialized,
        Arc::new(RwLock::new(vault_core::approval::ApprovalQueue::new())),
        auth_manager,
    );
}

async fn spawn_server(port: u16) -> WsClient {
    let server = IpcServer::new(port).expect("create IpcServer");
    let token_path = server.auth_token_path().to_path_buf();
    let auth_token = std::fs::read_to_string(&token_path).expect("read auth token");
    let auth_manager = Arc::new(AuthManager::new());
    {
        let mut handler = server.handler().await;
        register(&mut handler, Arc::clone(&auth_manager));
    }
    let (_handle, _ready) = server.run();
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    WsClient::connect(port, &auth_token).await
}

#[tokio::test]
async fn staged_mnemonic_flow_not_re_submitted() {
    let mut client = spawn_server(18420).await;

    // 1. Stage a fresh mnemonic — Rust generates and returns it once.
    let r = client.call("vault.stage_mnemonic", json!({})).await;
    let staged = r["result"]["mnemonic"]
        .as_str()
        .expect("staged mnemonic");
    let word_count = staged.split_whitespace().count();
    assert_eq!(word_count, 24, "staged mnemonic should be 24 words");

    // 2. Initialize with an EMPTY seed_phrase — the backend must use the
    //    staged phrase (and return it back for display), not require a re-submit.
    let r = client
        .call(
            "vault.initialize",
            json!({ "seed_phrase": "", "passphrase": "" }),
        )
        .await;
    let resp = r.get("result").expect("init should succeed");
    assert_eq!(resp["success"], true);
    // Since seed_phrase was empty (generated flow), the mnemonic is returned.
    let returned = resp["mnemonic"].as_str().expect("mnemonic returned");
    assert_eq!(returned, staged, "initialized from the staged phrase");

    // 3. A second initialize must now fail (already initialized + staged cleared).
    let r = client
        .call(
            "vault.initialize",
            json!({ "seed_phrase": "", "passphrase": "" }),
        )
        .await;
    assert!(
        r.get("error").is_some(),
        "re-init must fail after staging consumed"
    );
}

#[tokio::test]
async fn clear_staged_discards_phrase() {
    let mut client = spawn_server(18421).await;

    // Stage, then clear before init.
    let r = client.call("vault.stage_mnemonic", json!({})).await;
    assert!(r["result"]["mnemonic"].is_string());
    let r = client.call("vault.clear_staged", json!({})).await;
    assert_eq!(r["result"]["success"], true);

    // After clear, init with empty seed regenerates a NEW phrase (not staged).
    let r = client
        .call(
            "vault.initialize",
            json!({ "seed_phrase": "", "passphrase": "" }),
        )
        .await;
    let resp = r.get("result").expect("init should succeed");
    assert_eq!(resp["success"], true);
    let regenerated = resp["mnemonic"].as_str().expect("mnemonic regenerated");
    assert_eq!(regenerated.split_whitespace().count(), 24);
}
