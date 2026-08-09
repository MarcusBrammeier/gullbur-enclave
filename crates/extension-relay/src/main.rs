//! gullbur-relay — Native Messaging Host binary.
//!
//! Bridges Chrome/Firefox native messaging (stdio JSON) to vault-core
//! WebSocket IPC. All logging goes to stderr; stdout is reserved for
//! the 4-byte LE length-prefixed JSON protocol.

use clap::Parser;
use extension_relay::native_host::{self, NativeMessage, NativeResponse};
use extension_relay::rate_limiter::RateLimiter;
use extension_relay::router;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Gullbúr Enclave — Native Messaging Relay
#[derive(Parser)]
#[command(name = "gullbur-relay")]
struct Args {
    /// vault-core IPC WebSocket port (default: 19876)
    #[arg(long, default_value = "19876")]
    ipc_port: u16,

    /// Allowed extension IDs (comma-separated)
    #[arg(long, default_value = "")]
    allowed_ids: String,

    /// IPC auth token (reads from token file if not provided)
    #[arg(long, default_value = "")]
    auth_token: String,
}

#[tokio::main]
async fn main() {
    // ── Logging to stderr only ─────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let args = Args::parse();

    let allowed_ids: Vec<String> = if args.allowed_ids.is_empty() {
        vec![]
    } else {
        args.allowed_ids
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };

    tracing::info!(
        "gullbur-relay starting — IPC port {}, {} allowed origin(s)",
        args.ipc_port,
        allowed_ids.len()
    );

    // ── Connect to vault-core WebSocket (with retry) ──────────────────
    let ws_url = format!("ws://127.0.0.1:{}", args.ipc_port);
    tracing::info!("Connecting to vault IPC at {ws_url}");

    let (ws, _resp) = {
        let mut attempts = 0u32;
        loop {
            match connect_async(&ws_url).await {
                Ok(c) => break c,
                Err(e) => {
                    attempts += 1;
                    if attempts > 50 {
                        tracing::error!("Failed to connect after {attempts} attempts: {e}");
                        std::process::exit(1);
                    }
                    tracing::warn!("IPC connection retry ({attempts}/50): {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    };

    let (mut ws_write, mut ws_read) = ws.split();

    // Send auth token for IPC
    let token = if args.auth_token.is_empty() {
        "foss-relay-dev-token" // development fallback
    } else {
        &args.auth_token
    };
    if let Err(e) = ws_write.send(Message::Text(token.into())).await {
        tracing::error!("Auth failed: {e}");
        std::process::exit(1);
    }
    tracing::info!("IPC authenticated");

    // Wrap write half in mutex for shared access
    let ws_write = Arc::new(Mutex::new(ws_write));

    // Spawn task to forward WebSocket responses to stdout
    let ws_write_clone = Arc::clone(&ws_write);
    let ws_to_stdout = tokio::spawn(async move {
        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Parse the JSON-RPC response from vault-core
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        let id = value.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                        let result = value.get("result").cloned();
                        let error = value.get("error").map(|e| native_host::NativeError {
                            code: e.get("code").and_then(|c| c.as_i64()).unwrap_or(-32000) as i32,
                            message: e
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                        });
                        let response = NativeResponse { id, result, error };
                        native_host::write_response(&response).await;
                    }
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("Vault IPC connection closed");
                    break;
                }
                Ok(_) => {} // Skip binary/ping/pong
                Err(e) => {
                    tracing::error!("WebSocket error: {e}");
                    break;
                }
            }
        }
    });

    // ── Main stdio loop ────────────────────────────────────────────────
    let mut request_counter: u64 = 0;
    let mut limiter = RateLimiter::new();

    loop {
        let msg: NativeMessage = match native_host::read_message().await {
            Some(m) => m,
            None => {
                tracing::info!("stdin closed, shutting down");
                break;
            }
        };

        request_counter += 1;
        let id = request_counter;

        // Security: rate-limit per origin
        if let Err(e) = limiter.check(&msg.origin) {
            tracing::warn!("{e} for origin: {}", msg.origin);
            let response = NativeResponse {
                id,
                result: None,
                error: Some(native_host::NativeError {
                    code: -32000,
                    message: e,
                }),
            };
            native_host::write_response(&response).await;
            continue;
        }

        // Security: validate origin
        if !allowed_ids.is_empty() && !native_host::validate_origin(&msg.origin, &allowed_ids) {
            tracing::warn!("Rejected origin: {}", msg.origin);
            let response = NativeResponse {
                id,
                result: None,
                error: Some(native_host::NativeError {
                    code: -32000,
                    message: format!("Origin not allowed: {}", msg.origin),
                }),
            };
            native_host::write_response(&response).await;
            continue;
        }

        // Route dApp method to vault IPC method
        let routed = match router::route(&msg.method, msg.params.unwrap_or(serde_json::Value::Null))
        {
            Some(r) => r,
            None => {
                let response = NativeResponse {
                    id,
                    result: None,
                    error: Some(native_host::NativeError {
                        code: -32601,
                        message: format!("Method not found: {}", msg.method),
                    }),
                };
                native_host::write_response(&response).await;
                continue;
            }
        };

        // Security: gate approval-required methods
        if routed.requires_approval {
            tracing::info!(
                "Blocked {} — requires user approval (no UI available in relay)",
                msg.method
            );
            let response = NativeResponse {
                id,
                result: None,
                error: Some(native_host::NativeError {
                    code: -32000,
                    message: format!(
                        "{} requires user approval — not available in relay mode",
                        msg.method
                    ),
                }),
            };
            native_host::write_response(&response).await;
            continue;
        }

        // Build JSON-RPC request for vault-core
        let rpc_id = id;
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": routed.vault_method,
            "params": routed.params,
            "id": rpc_id,
        });

        // Forward to vault-core via WebSocket
        tracing::debug!("→ vault: {} (id={rpc_id})", routed.vault_method);
        if let Err(e) = ws_write_clone
            .lock()
            .await
            .send(Message::Text(rpc_request.to_string().into()))
            .await
        {
            tracing::error!("Failed to send to vault IPC: {e}");
            let response = NativeResponse {
                id,
                result: None,
                error: Some(native_host::NativeError {
                    code: -32000,
                    message: "Vault IPC connection error".into(),
                }),
            };
            native_host::write_response(&response).await;
            break;
        }
    }

    // Cleanup
    ws_to_stdout.abort();
    tracing::info!("gullbur-relay shutting down");
}
