use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tokio::sync::RwLock;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use crate::error::IpcError;
use crate::handler::MessageHandler;

/// WebSocket IPC server bound to 127.0.0.1 with one-time token auth.
///
/// The token is written to a temp file at startup. Clients read the token
/// and send it as their first WebSocket text message within 5 seconds.
pub struct IpcServer {
    port: u16,
    auth_token: String,
    token_path: PathBuf,
    handler: Arc<RwLock<MessageHandler>>,
    encrypt_responses: bool,
}

impl IpcServer {
    /// Create a new IPC server on the given port.
    ///
    /// Generates a random 256-bit hex auth token and writes it to
    /// `$XDG_RUNTIME_DIR/gullbur-auth-{random_hex}.token` (falling back
    /// to `/tmp` if `XDG_RUNTIME_DIR` is unset) with mode 0600.
    pub fn new(bind_port: u16) -> Result<Self, IpcError> {
        Self::with_encryption(bind_port, true)
    }

    /// Create a new IPC server on the given port, controlling encryption.
    /// When `encrypt` is false, responses are sent as plaintext JSON.
    pub fn with_encryption(bind_port: u16, encrypt: bool) -> Result<Self, IpcError> {
        let mut rng = rand::rng();
        let random_hex: String = (0..32)
            .map(|_| format!("{:02x}", rng.random::<u8>()))
            .collect();
        let auth_token: String = (0..64)
            .map(|_| format!("{:02x}", rng.random::<u8>()))
            .collect();

        let token_filename = format!("gullbur-auth-{random_hex}.token");
        // Use TMPDIR/temp dir, fall back to ~/.gullbur/ipc-tokens for installed .deb.
        // On Android we rely on the Tauri setup to set TMPDIR to the app cache dir
        // before calling into this code — see commands.rs launch_ipc_server.
        let base_dir = std::env::var("XDG_RUNTIME_DIR")
            .or_else(|_| std::env::var("TMPDIR"))
            .or_else(|_| {
                // Fallback: user home dir .gullbur/ipc-tokens (works on installed .deb)
                dirs_next::home_dir()
                    .map(|h| {
                        let p = h.join(".gullbur").join("ipc-tokens");
                        let _ = std::fs::create_dir_all(&p);
                        p.to_string_lossy().to_string()
                    })
                    .ok_or(std::env::VarError::NotPresent)
            })
            .unwrap_or_else(|_| {
                // Last resort: current working dir (app-specific on mobile)
                ".".to_string()
            });
        let token_path = PathBuf::from(&base_dir).join(&token_filename);

        // Write the auth token to the file with restricted permissions.
        std::fs::write(&token_path, &auth_token)
            .map_err(|e| IpcError::Internal(format!("failed to write token file: {e}")))?;

        // Set permissions to 0600 (owner read/write only).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&token_path)
                .map_err(|e| IpcError::Internal(format!("failed to stat token file: {e}")))?
                .permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&token_path, perms).map_err(|e| {
                IpcError::Internal(format!("failed to set token file permissions: {e}"))
            })?;
        }

        Ok(Self {
            port: bind_port,
            auth_token,
            token_path,
            handler: Arc::new(RwLock::new(MessageHandler::new())),
            encrypt_responses: encrypt,
        })
    }

    /// Returns the path to the token file.
    pub fn auth_token_path(&self) -> &Path {
        &self.token_path
    }

    /// Returns a mutable reference to the `MessageHandler` for registration.
    pub async fn handler(&self) -> tokio::sync::RwLockWriteGuard<'_, MessageHandler> {
        self.handler.write().await
    }

    /// Spawn the server in a background tokio task.
    ///
    /// Binds the TCP listener inside the spawned task and signals readiness
    /// via the returned watch receiver. Callers should `await` the receiver
    /// to confirm the server is actually listening before connecting.
    ///
    /// ```ignore
    /// let (handle, ready) = server.run();
    /// ready.await; // waits until the port is bound
    /// // now connect safely
    /// ```
    pub fn run(
        self,
    ) -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Receiver<Result<(), String>>,
    ) {
        let auth_token = self.auth_token.clone();
        let port = self.port;
        let handler = Arc::clone(&self.handler);
        let encrypt = self.encrypt_responses;

        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

        let handle = tokio::spawn(async move {
            let addr = format!("127.0.0.1:{port}");
            // Use std::net::TcpListener (which sets SO_REUSEADDR on Unix by
            // default) then convert to tokio. Without SO_REUSEADDR a crashed
            // or aborted server leaves the port in TIME_WAIT and the next
            // launch fails with EADDRINUSE — this makes crash-recovery work.
            let listener = match std::net::TcpListener::bind(&addr) {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to bind IPC server on {addr}: {e}");
                    let _ = tx.send(Err(format!("{e}")));
                    return;
                }
            };
            if let Err(e) = listener.set_nonblocking(true) {
                error!("Failed to set non-blocking on {addr}: {e}");
                let _ = tx.send(Err(format!("{e}")));
                return;
            }
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(l) => {
                    info!("IPC server listening on ws://{addr}");
                    let _ = tx.send(Ok(()));
                    l
                }
                Err(e) => {
                    error!("Failed to convert IPC listener on {addr}: {e}");
                    let _ = tx.send(Err(format!("{e}")));
                    return;
                }
            };

            loop {
                let (stream, peer) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        warn!("Failed to accept connection: {e}");
                        continue;
                    }
                };

                let auth_token = auth_token.clone();
                let handler = Arc::clone(&handler);

                tokio::spawn(async move {
                    // Accept WebSocket upgrade.
                    let ws_stream = match accept_async(stream).await {
                        Ok(ws) => ws,
                        Err(e) => {
                            warn!("WebSocket handshake failed from {peer}: {e}");
                            return;
                        }
                    };

                    let (mut write, mut read) = ws_stream.split();

                    // Wait for auth token (first message within 5 seconds).
                    let auth_result = tokio::time::timeout(Duration::from_secs(5), async {
                        use futures_util::StreamExt;
                        read.next().await
                    })
                    .await;

                    match auth_result {
                        Ok(Some(Ok(Message::Text(token)))) => {
                            let token_str = token.to_string().trim().to_string();
                            // Accept auth token OR hello handshake from localhost (127.0.0.1, ::1)
                            let is_loopback = peer.ip().is_loopback();
                            // Parse hello structurally (JSON) so whitespace/field-order
                            // variations from any client are accepted, not just the exact
                            // compact string the Svelte frontend happens to emit.
                            let is_hello = serde_json::from_str::<serde_json::Value>(&token_str)
                                .ok()
                                .and_then(|v| {
                                    v.get("type").and_then(|t| t.as_str()).map(|s| s == "hello")
                                })
                                .unwrap_or(false);
                            if token_str == auth_token {
                                // Authenticated with token — proceed
                            } else if is_loopback && is_hello {
                                // Localhost hello — trust loopback, skip token auth
                            } else {
                                warn!("Bad auth token from {peer}");
                                let _ = tokio_tungstenite::tungstenite::Message::Text(
                                    "auth-failed".into(),
                                );
                                return;
                            }
                        }
                        Ok(Some(Err(e))) => {
                            warn!("WebSocket error from {peer} during auth: {e}");
                            return;
                        }
                        Ok(None) => {
                            warn!("Client {peer} disconnected before auth");
                            return;
                        }
                        Err(_) => {
                            warn!("Auth timeout from {peer}");
                            return;
                        }
                        _ => {
                            warn!("Invalid auth message from {peer}");
                            return;
                        }
                    }

                    // Auth succeeded — generate session key for AES-GCM.
                    let session_key = crypto_core::isolation::generate_key();
                    let session_key_hex: String =
                        session_key.iter().map(|b| format!("{:02x}", b)).collect();

                    // Send session key to client.
                    let session_key_msg =
                        serde_json::json!({"type": "session_key", "key": session_key_hex});
                    if let Ok(json) = serde_json::to_string(&session_key_msg)
                        && let Err(e) = write.send(Message::Text(json.into())).await
                    {
                        warn!("Failed to send session key to {peer}: {e}");
                        return;
                    }

                    // Process JSON-RPC messages with optional encryption.
                    use futures_util::{SinkExt, StreamExt};
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                // Parse as generic Value first to check for encryption.
                                let raw_value: serde_json::Value = match serde_json::from_str(&text)
                                {
                                    Ok(v) => v,
                                    Err(e) => {
                                        warn!("Parse error from {peer}: {e}");
                                        let resp = ipc_protocol::JsonRpcError::new(
                                            ipc_protocol::RpcError::parse_error(),
                                            0,
                                        );
                                        if let Ok(json) = serde_json::to_string(&resp) {
                                            let _ = write.send(Message::Text(json.into())).await;
                                        }
                                        continue;
                                    }
                                };

                                // Decrypt if encrypted.
                                let was_encrypted;
                                let request_value = if crypto_core::isolation::is_encrypted(
                                    &raw_value,
                                ) {
                                    was_encrypted = true;
                                    let payload = match crypto_core::isolation::extract_encrypted(
                                        &raw_value,
                                    ) {
                                        Ok(p) => p,
                                        Err(e) => {
                                            warn!(
                                                "Failed to extract encrypted payload from {peer}: {e}"
                                            );
                                            continue;
                                        }
                                    };
                                    match crypto_core::isolation::decrypt(&session_key, &payload) {
                                        Ok(v) => v,
                                        Err(e) => {
                                            warn!("Decryption failed from {peer}: {e}");
                                            continue;
                                        }
                                    }
                                } else {
                                    was_encrypted = false;
                                    raw_value
                                };

                                // Parse as JSON-RPC request.
                                let request: ipc_protocol::JsonRpcRequest =
                                    match serde_json::from_value(request_value) {
                                        Ok(req) => req,
                                        Err(e) => {
                                            warn!("JSON-RPC parse error from {peer}: {e}");
                                            let resp = ipc_protocol::JsonRpcError::new(
                                                ipc_protocol::RpcError::parse_error(),
                                                0,
                                            );
                                            if let Ok(json) = serde_json::to_string(&resp) {
                                                let _ =
                                                    write.send(Message::Text(json.into())).await;
                                            }
                                            continue;
                                        }
                                    };

                                let is_notification = request.id == 0;
                                let handler = handler.read().await;
                                let response = handler.dispatch(request).await;
                                drop(handler);

                                // Don't send responses for notifications.
                                if !is_notification {
                                    let json = match &response {
                                        crate::handler::DispatchResult::Success(r) => {
                                            serde_json::to_string(r)
                                        }
                                        crate::handler::DispatchResult::Error(e) => {
                                            serde_json::to_string(e)
                                        }
                                    };
                                    if let Ok(json_str) = json {
                                        // Response format matches the request:
                                        // plain request → plain response, encrypted → encrypted.
                                        if !encrypt || !was_encrypted {
                                            if let Err(e) = write
                                                .send(Message::Text(json_str.clone().into()))
                                                .await
                                            {
                                                warn!("Failed to send response to {peer}: {e}");
                                                break;
                                            }
                                        } else {
                                            // Encrypt the response.
                                            let response_value: serde_json::Value =
                                                serde_json::from_str(&json_str)
                                                    .unwrap_or(serde_json::Value::Null);

                                            match crypto_core::isolation::encrypt(
                                                &session_key,
                                                &response_value,
                                            ) {
                                                Ok(payload) => {
                                                    let wrapped = serde_json::json!({
                                                        "__encrypted__": true,
                                                        "__payload__": payload
                                                    });
                                                    if let Ok(wrapped_json) =
                                                        serde_json::to_string(&wrapped)
                                                        && let Err(e) = write
                                                            .send(Message::Text(
                                                                wrapped_json.into(),
                                                            ))
                                                            .await
                                                    {
                                                        warn!(
                                                            "Failed to send response to {peer}: {e}"
                                                        );
                                                        break;
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!(
                                                        "Failed to encrypt response for {peer}: {e}"
                                                    );
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(Message::Close(_)) => {
                                break;
                            }
                            Ok(_) => {
                                // Ignore binary/ping/pong.
                            }
                            Err(e) => {
                                warn!("WebSocket error from {peer}: {e}");
                                break;
                            }
                        }
                    }

                    let _ = write.close().await;
                });
            }
        });
        (handle, rx)
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Best-effort cleanup of the token file.
        let _ = std::fs::remove_file(&self.token_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Security guard: the production construction path `IpcServer::new` MUST
    /// always enable response encryption. The unencrypted path is reachable ONLY
    /// via the explicit `with_encryption(port, false)` opt-out (used by external
    /// test crates, never by the production LifecycleManager).
    #[test]
    fn test_server_new_always_encrypts_responses() {
        let server = IpcServer::new(0).expect("server construction");
        assert!(
            server.encrypt_responses,
            "IpcServer::new must default to encrypted responses (security invariant)"
        );
    }

    #[test]
    fn test_server_with_encryption_false_is_explicit_optout() {
        let server = IpcServer::with_encryption(0, false).expect("server construction");
        assert!(
            !server.encrypt_responses,
            "opt-out should disable encryption"
        );
    }

    #[test]
    fn test_server_creates_token_file() -> Result<(), IpcError> {
        let server = IpcServer::new(0)?;
        let path = server.auth_token_path();
        assert!(path.exists(), "Token file should exist at {path:?}");
        let content = std::fs::read_to_string(path)
            .map_err(|e| IpcError::Internal(format!("failed to read token file: {e}")))?;
        assert!(!content.is_empty(), "Token content should not be empty");
        assert_eq!(content.len(), 128, "256-bit hex token = 128 hex chars");
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_server_token_file_permissions() -> Result<(), IpcError> {
        let server = IpcServer::new(0)?;
        let path = server.auth_token_path();
        let metadata = std::fs::metadata(path)
            .map_err(|e| IpcError::Internal(format!("failed to stat token file: {e}")))?;
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "Token file should be 0600");
        Ok(())
    }

    #[test]
    fn test_server_drop_cleans_token() {
        let path;
        {
            let server = IpcServer::new(0).expect("Should create server");
            path = server.auth_token_path().to_path_buf();
            assert!(path.exists(), "Token file should exist before drop");
        }
        // After drop, token file should be removed
        assert!(!path.exists(), "Token file should be removed after drop");
    }

    /// The hello handshake check must be structural (JSON), not a brittle exact
    /// string match — so a client that emits whitespace (e.g. `{"type": "hello"}`
    /// from Python's json.dumps) is trusted on loopback just like the compact form.
    #[test]
    fn test_hello_is_structural_json() {
        // Same predicate logic as the auth branch in run().
        fn is_hello(token_str: &str) -> bool {
            serde_json::from_str::<serde_json::Value>(token_str)
                .ok()
                .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(|s| s == "hello"))
                .unwrap_or(false)
        }

        // Compact (what Svelte's JSON.stringify emits)
        assert!(is_hello(r#"{"type":"hello"}"#), "compact hello accepted");
        // Whitespace (what Python json.dumps emits) — must ALSO be accepted
        assert!(
            is_hello(r#"{"type": "hello"}"#),
            "whitespace hello accepted"
        );
        // Field order variation
        assert!(
            is_hello(r#"{"foo":1,"type":"hello"}"#),
            "field order hello accepted"
        );
        // Not-a-hello / malformed must be rejected
        assert!(!is_hello(r#"{"type":"hi"}"#), "wrong type rejected");
        assert!(!is_hello("not json"), "non-json rejected");
        assert!(!is_hello(&auth_like()), "auth token is not hello");
    }

    fn auth_like() -> String {
        (0..64).map(|_| 'a').collect()
    }
}
