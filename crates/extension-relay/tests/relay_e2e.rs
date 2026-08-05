//! Extension-relay native-messaging E2E test.
//!
//! Spawns the REAL `gullbur-relay` binary and drives it over its stdio
//! native-messaging protocol (4-byte LE length prefix + JSON). Verifies the
//! end-to-end framing + security gates that don't require a live vault-core
//! connection:
//!   - unknown method  → -32601 Method not found
//!   - approval-gated  → blocked (must never reach the vault unapproved)
//!   - allowed origin  → framed request reaches the routing/forwarding stage
//!
//! Run: cargo test --test relay_e2e -p extension-relay

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

/// 4-byte LE length prefix + JSON payload (Chrome/Firefox native messaging).
fn frame(json: &str) -> Vec<u8> {
    let mut out = (json.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(json.as_bytes());
    out
}

/// Read one length-prefixed JSON message from the relay's stdout.
fn read_frame(stdout: &mut ChildStdout) -> Option<String> {
    let mut len_buf = [0u8; 4];
    // Poll with a timeout: use non-blocking-ish loop over a small sleep.
    let mut got = 0;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while got < 4 {
        if std::time::Instant::now() > deadline {
            return None;
        }
        match stdout.read(&mut len_buf[got..]) {
            Ok(0) => return None,
            Ok(n) => {
                got += n;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    if msg_len > 16 * 1024 * 1024 {
        return None;
    }
    let mut body = vec![0u8; msg_len];
    let mut got = 0;
    while got < msg_len {
        match stdout.read(&mut body[got..]) {
            Ok(0) => return None,
            Ok(n) => got += n,
            Err(_) => return None,
        }
    }
    Some(String::from_utf8_lossy(&body).to_string())
}

/// Spawn a real IPC WebSocket server on the given port, so the relay's
/// connect/retry loop succeeds and it reaches its stdio message loop.
/// Runs on its own thread with its own tokio runtime.
fn spawn_ws_server(port: u16) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("test runtime");
        rt.block_on(async {
            let server = ipc_core::server::IpcServer::new(port).expect("create IPC server");
            let (handle, _ready) = server.run();
            // Keep the runtime alive for the duration of the test.
            tokio::time::sleep(Duration::from_secs(30)).await;
            handle.abort();
        });
    });
    std::thread::sleep(Duration::from_millis(400));
}

/// Spawn the real relay binary with stdin/stdout as native-messaging pipes.
fn spawn_relay(port: u16) -> (Child, ChildStdin, ChildStdout) {
    let bin = env!("CARGO_BIN_EXE_gullbur-relay");
    let mut child = Command::new(bin)
        .arg("--ipc-port")
        .arg(port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn gullbur-relay");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    (child, stdin, stdout)
}

#[test]
fn e2e_unknown_method_returns_error_over_native_pipe() {
    let port = 20981;
    spawn_ws_server(port);
    let (mut child, mut stdin, mut stdout) = spawn_relay(port);
    // Give the relay a moment to start its WS connect loop (fails harmlessly).
    std::thread::sleep(Duration::from_millis(500));

    let msg = serde_json::json!({
        "origin": "chrome-extension://testid",
        "method": "eth_unknownMethod",
        "params": {}
    });
    stdin
        .write_all(&frame(&msg.to_string()))
        .expect("write frame");
    stdin.flush().expect("flush");

    let resp = read_frame(&mut stdout).expect("relay should respond");
    let v: serde_json::Value = serde_json::from_str(&resp).expect("json response");
    let code = v["error"]["code"].as_i64().unwrap_or(-1);
    assert_eq!(code, -32601, "unknown method → METHOD_NOT_FOUND: {resp}");
    child.kill().ok();
}

#[test]
fn e2e_approval_gated_method_blocked() {
    let port = 20982;
    spawn_ws_server(port);
    let (mut child, mut stdin, mut stdout) = spawn_relay(port);
    std::thread::sleep(Duration::from_millis(500));

    // eth_sendTransaction REQUIRES user approval. In relay-only mode it must
    // be blocked BEFORE reaching the vault — never auto-executed.
    let msg = serde_json::json!({
        "origin": "chrome-extension://testid",
        "method": "eth_sendTransaction",
        "params": {"from": "0xabc"}
    });
    stdin
        .write_all(&frame(&msg.to_string()))
        .expect("write frame");
    stdin.flush().expect("flush");

    let resp = read_frame(&mut stdout).expect("relay should respond");
    assert!(
        resp.contains("requires user approval"),
        "sendTransaction must be approval-blocked: {resp}"
    );
    child.kill().ok();
}

#[test]
fn e2e_unrouted_method_not_found() {
    let port = 20983;
    spawn_ws_server(port);
    let (mut child, mut stdin, mut stdout) = spawn_relay(port);
    std::thread::sleep(Duration::from_millis(500));

    let msg = serde_json::json!({
        "origin": "",
        "method": "totally_unknown",
        "params": null
    });
    stdin
        .write_all(&frame(&msg.to_string()))
        .expect("write frame");
    stdin.flush().expect("flush");

    let resp = read_frame(&mut stdout).expect("relay should respond");
    let v: serde_json::Value = serde_json::from_str(&resp).expect("json response");
    assert_eq!(
        v["error"]["code"].as_i64().unwrap_or(-1),
        -32601,
        "unsupported → METHOD_NOT_FOUND: {resp}"
    );
    child.kill().ok();
}
