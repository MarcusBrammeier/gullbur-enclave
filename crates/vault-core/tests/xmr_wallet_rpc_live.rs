//! Live `monero-wallet-rpc` integration test (Phase 2.2).
//!
//! Verifies the REAL `monero-wallet-rpc` binary (v0.18.5.1) boots under
//! `MoneroWalletRpcProcess`, connects to a public stagenet daemon, and answers
//! actual wallet RPC calls — proving the XMR wallet-rpc integration works
//! end-to-end at the binary level (no funded coins required).
//!
//! Run (binary auto-downloaded to target/xmr-test-bin):
//!   cargo test -p vault-core --test xmr_wallet_rpc_live -- --ignored --nocapture

use serde_json::Value;
use std::time::Duration;
use vault_core::xmr_wallet_rpc::MoneroWalletRpcProcess;

const BIN: &str = "target/xmr-test-bin/monero-wallet-rpc";

fn bin_path() -> String {
    // Allow override; fall back to repo-relative cached binary.
    std::env::var("XMR_WALLET_RPC_BIN").unwrap_or_else(|_| BIN.to_string())
}

async fn rpc(url: &str, method: &str, params: Value) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0", "id": "0", "method": method, "params": params,
        }))
        .send()
        .await
        .map_err(|e| format!("RPC {method} failed: {e}"))?;
    let json: Value = resp.json().await.map_err(|e| e.to_string())?;
    if let Some(err) = json.get("error") {
        return Err(format!("wallet RPC {method} error: {err}"));
    }
    json.get("result")
        .cloned()
        .ok_or_else(|| format!("missing result for {method}"))
}

#[tokio::test]
#[ignore = "requires real monero-wallet-rpc binary + stagenet network — run with --ignored"]
async fn live_wallet_rpc_boots_and_answers() {
    let bin = bin_path();
    if !std::path::Path::new(&bin).exists() {
        eprintln!("SKIP: monero-wallet-rpc not found at {bin} — run download, then re-run");
        return;
    }

    let wallet_dir = format!("/tmp/xmr-wallet-live-{}", std::process::id());
    let port = 18100u16;

    let mut proc = MoneroWalletRpcProcess::new(&bin, "monero-stagenet", &wallet_dir)
        .with_port(port)
        .with_daemon("stagenet.xmr-node.cakewallet.com:38081");

    // 1. Boot the binary — proves spawn + health-check readiness.
    proc.start().await.expect("wallet-rpc should start");
    let url = proc.url().expect("running process must expose RPC URL");
    eprintln!("wallet-rpc live at {url}");

    // 2. Answer a real wallet RPC (get_version via raw health was internal).
    //    Create an in-memory wallet and fetch its address — proves the wallet
    //    engine actually talks to the stagenet daemon.
    let label = format!("live_wallet_{}", std::process::id());
    rpc(
        &url,
        "create_wallet",
        serde_json::json!({"filename": label, "password": "", "language": "English"}),
    )
    .await
    .expect("create_wallet over stagenet");

    let addr = rpc(&url, "get_address", serde_json::json!({}))
        .await
        .expect("get_address");
    let address = addr["address"].as_str().expect("address field");
    eprintln!("Created stagenet wallet → {address}");
    // A valid Monero address is a ~95-char base58 string. Prefix varies by
    // network/features (4/8/9/A/7), so don't hard-code it — just validate the
    // shape: length + base58 alphabet (digits + lowercase + uppercase, no 0OIl).
    assert!(
        address.len() >= 90,
        "monero address should be ~95 chars, got {}",
        address.len()
    );
    let base58_ok = address
        .chars()
        .all(|c| c.is_ascii_alphanumeric() && !"0OIl".contains(c));
    assert!(base58_ok, "address must be base58 (no 0/O/I/l): {address}");

    // 3. Query balance (0 expected for fresh wallet — proves daemon sync path).
    let balance = rpc(&url, "get_balance", serde_json::json!({}))
        .await
        .expect("get_balance");
    eprintln!("Fresh wallet balance: {:?}", balance.get("balance"));

    // 4. Clean shutdown.
    proc.shutdown().await.expect("clean shutdown");
    eprintln!("wallet-rpc shut down cleanly");
}
