//! WebSocket JSON-RPC client for CLI integration tests.
//!
//! Simple per-call connect/disconnect pattern to avoid stream-split borrowing.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Send one JSON-RPC call and return the matched response.
/// Opens a fresh WebSocket connection per call (lightweight for localhost).
pub async fn call(port: u16, token: &str, method: &str, params: Value) -> Value {
    let url = format!("ws://127.0.0.1:{port}");
    let (ws, _) = connect_async(&url).await.expect("WebSocket connect");
    let (mut write, mut read) = ws.split();

    // Authenticate
    write
        .send(Message::Text(token.into()))
        .await
        .expect("send auth token");

    // Send RPC request
    let id: u64 = rand::random();
    let req = json!({"jsonrpc":"2.0","method": method,"params": params,"id": id});
    write
        .send(Message::Text(req.to_string().into()))
        .await
        .expect("send rpc");

    // Read until we get a matching response (ignore other messages)
    loop {
        match read.next().await {
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

/// Assert that the response contains no `error` field, returning
/// a reference to the `result`. Panics with `context` on failure.
pub fn assert_ok<'a>(resp: &'a Value, context: &str) -> &'a Value {
    if resp.get("error").is_some() {
        let code = resp["error"]["code"].as_i64().unwrap_or(0);
        let msg = resp["error"]["message"].as_str().unwrap_or("?");
        panic!("{context}: error {code}: {msg} (expected success)");
    }
    &resp["result"]
}