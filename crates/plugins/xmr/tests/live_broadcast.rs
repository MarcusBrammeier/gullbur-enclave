//! E2E Monero stagenet broadcast test — via monero-wallet-rpc container.
//!
//! 1. Restores wallet from seed in monero-wallet-rpc (localhost:18082)
//! 2. Prints the wallet address for funding
//! 3. Waits for confirmed balance (up to 600s)
//! 4. Sends a transfer via wallet-rpc (handles ring signatures internally)
//! 5. Polls for confirmation via daemon RPC
//!
//! Requires: `xmr-light` container running on localhost:18082
//! Run: XMR_STAGENET_SEED=0x... cargo test -p plugin-xmr --test live_broadcast -- --ignored --nocapture

use plugin_xmr::SpendKey;
use plugin_xmr::XmrPlugin;
use wallet_plugin::WalletPlugin;

use std::time::{SystemTime, UNIX_EPOCH};

fn wallet_rpc_url() -> &'static str {
    "http://127.0.0.1:18082/json_rpc"
}

fn daemon_url(network: &str) -> &str {
    match network {
        "monero-stagenet" => "http://node.sethforprivacy.com:38089/json_rpc",
        _ => panic!("unsupported network: {network}"),
    }
}

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .expect("test invariant")
}

async fn wallet_rpc(
    client: &reqwest::Client,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let url = wallet_rpc_url();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "0",
        "method": method,
        "params": params,
    });
    let resp = client.post(url).json(&body).send().await.expect("wallet RPC POST failed");
    let json: serde_json::Value = resp.json().await.expect("wallet RPC JSON parse failed");
    if let Some(err) = json.get("error") {
        panic!("wallet RPC error: {err}");
    }
    json.get("result").cloned().expect("missing 'result' in wallet RPC response")
}

async fn wait_for_balance(client: &reqwest::Client, min_unlocked: u64, timeout_secs: u64) {
    let start = std::time::Instant::now();
    loop {
        let info = wallet_rpc(client, "get_balance", serde_json::json!({})).await;
        let unlocked = info.get("unlocked_balance").and_then(|v| v.as_u64()).unwrap_or(0);
        eprintln!("  Balance: {unlocked} piconero unlocked (need {min_unlocked})");
        if unlocked >= min_unlocked {
            return;
        }
        if start.elapsed() > std::time::Duration::from_secs(timeout_secs) {
            panic!("Timeout waiting for {min_unlocked} piconero balance ({timeout_secs}s)");
        }
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
}

/// Best-effort check — stagenet daemon may not expose get_transactions.
async fn check_confirmation(client: &reqwest::Client, network: &str, tx_hash: &str) -> Option<u64> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": "0", "method": "get_transactions",
        "params": { "txs_hashes": [tx_hash], "decode_as_json": false },
    });
    let resp = client.post(daemon_url(network)).json(&body).send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("result")?.get("txs")?.as_array()?.first()?.get("confirmations")?.as_u64()
}

#[tokio::test]
#[ignore = "requires XMR_STAGENET_SEED env var and stagenet XMR from faucet"]
async fn live_broadcast_stagenet() {
    let seed_hex = std::env::var("XMR_STAGENET_SEED").expect("Set XMR_STAGENET_SEED=0x...");
    let seed_hex = seed_hex.strip_prefix("0x").unwrap_or(&seed_hex);
    let seed = hex::decode(seed_hex).expect("Seed must be hex");
    assert_eq!(seed.len(), 64, "XMR seed must be 64 bytes (BIP-39)");
    let seed_512: [u8; 64] = seed[..64].try_into().expect("test invariant");
    let entropy = crypto_core::keys::derive_bip44_xmr_entropy(&seed_512, 0)
        .expect("BIP-44 entropy derivation");
    let spend_key = hex::encode(&entropy);
    let view_sk = SpendKey::from_seed(&entropy, 0).view_key();
    let view_key = hex::encode(view_sk.to_bytes());

    let client = build_client();

    // Step 1: Derive stagenet address from seed via plugin
    let network = "monero-stagenet";
    let plugin = XmrPlugin::new();
    let account = plugin.create_account(&seed, 0, network).await.expect("test invariant");

    // Step 2: Restore wallet from keys in wallet-rpc
    eprintln!("  Restoring wallet from keys in monero-wallet-rpc...");
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).expect("test invariant").as_nanos();
    let wallet_file = format!("test_wallet_{ts}");
    wallet_rpc(&client, "generate_from_keys", serde_json::json!({
        "filename": wallet_file, "password": "",
        "spend_key": spend_key, "viewkey": view_key,
        "restore_height": 2167700, "address": account.address,
    })).await;
    eprintln!("  Wallet restored ✅");

    // Step 3: Get wallet address
    let addr_info = wallet_rpc(&client, "get_address", serde_json::json!({})).await;
    let wallet_addr = addr_info["address"].as_str().unwrap_or("unknown");
    eprintln!("\n📌 Wallet stagenet address: {wallet_addr}\n");

    // Step 4: Wait for confirmed balance
    eprintln!("  Polling every 15s for up to 10min...");
    wait_for_balance(&client, 10_000_000_000, 600).await;
    eprintln!("\n✅ Funded! Sending transfer...\n");

    // Step 5: Transfer via wallet-rpc
    let transfer = wallet_rpc(&client, "transfer", serde_json::json!({
        "destinations": [{ "amount": 8_000_000_000u64, "address": wallet_addr }],
        "priority": 0, "ring_size": 11, "get_tx_key": true, "do_not_relay": false,
    })).await;

    let tx_hash = transfer["tx_hash"].as_str().expect("tx_hash").to_string();
    eprintln!("\n🎉 Transaction broadcast! Hash: {tx_hash}\n");
    assert_eq!(tx_hash.len(), 64, "tx_hash should be 64 hex chars");

    // Step 6: Best-effort confirmation poll
    eprintln!("  Checking confirmation...");
    let start = std::time::Instant::now();
    loop {
        match check_confirmation(&client, network, &tx_hash).await {
            Some(n) if n > 0 => {
                eprintln!("\n🎉 Confirmed! ({n} confirmations) Hash: {tx_hash}");
                return;
            }
            _ => {}
        }
        if start.elapsed() > std::time::Duration::from_secs(120) {
            eprintln!("\n⚠️  Tx sent but confirmation poll timed out (120s)");
            eprintln!("   Hash: {tx_hash}");
            eprintln!("   Check: https://stagenet.xmrchain.net/tx/{tx_hash}");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
}