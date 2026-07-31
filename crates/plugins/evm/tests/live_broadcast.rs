//! E2E broadcast test — signs and sends a real Sepolia transaction.
//!
//! Tests the critical untested path: crypto signing → live RPC broadcast.
//! Requires SEPOLIA_PRIVATE_KEY env var (hex, with or without 0x prefix).
//! Run:   SEPOLIA_PRIVATE_KEY=0x... cargo test -p plugin-evm --test live_broadcast -- --ignored
//!
//! If the address has no Sepolia ETH, the test fails gracefully.

use plugin_evm::EvmPlugin;
use wallet_plugin::{KeyHandle, KeyType, WalletPlugin};

// ── RLP helpers ─────────────────────────────────────────────────────────────

fn rlp_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        return data.to_vec();
    }
    if data.len() <= 55 {
        let mut out = vec![0x80 + data.len() as u8];
        out.extend(data);
        out
    } else {
        let lb = rlp_len_bytes(data.len());
        let mut out = vec![0xb7 + lb.len() as u8];
        out.extend(&lb);
        out.extend(data);
        out
    }
}

fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let total: usize = items.iter().map(|i| i.len()).sum();
    if total <= 55 {
        let mut out = vec![0xc0 + total as u8];
        for i in items {
            out.extend(i);
        }
        out
    } else {
        let lb = rlp_len_bytes(total);
        let mut out = vec![0xf7 + lb.len() as u8];
        out.extend(&lb);
        for i in items {
            out.extend(i);
        }
        out
    }
}

fn rlp_len_bytes(len: usize) -> Vec<u8> {
    let be = len.to_be_bytes();
    let s = be.iter().position(|&b| b != 0).unwrap_or(be.len() - 1);
    be[s..].to_vec()
}

fn rlp_u64(v: u64) -> Vec<u8> {
    if v == 0 {
        return vec![];
    }
    let be = v.to_be_bytes();
    let s = be.iter().position(|&b| b != 0).unwrap_or(be.len() - 1);
    be[s..].to_vec()
}

/// RLP-encode raw bytes as an INTEGER (strips leading zeros).
/// Correct for V, R, S in signed transactions.
fn rlp_int(data: &[u8]) -> Vec<u8> {
    let trimmed: Vec<u8> = data.iter().skip_while(|&&b| b == 0).copied().collect();
    if trimmed.is_empty() {
        vec![0x80]
    } else if trimmed.len() == 1 && trimmed[0] < 0x80 {
        trimmed
    } else {
        let mut out = vec![0x80 + trimmed.len() as u8];
        out.extend_from_slice(&trimmed);
        out
    }
}

const SEPOLIA_CHAIN_ID: u64 = 11155111;
const RPC: &str = "https://ethereum-sepolia-rpc.publicnode.com";

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("test invariant")
}

async fn json_rpc(endpoint: &str, method: &str, params: serde_json::Value) -> serde_json::Value {
    let client = build_client();
    let resp = client
        .post(endpoint)
        .json(&serde_json::json!({"jsonrpc":"2.0","method":method,"params":params,"id":1}))
        .send()
        .await
        .expect("RPC request failed");
    let json: serde_json::Value = resp.json().await.expect("RPC parse failed");
    if let Some(err) = json.get("error") {
        panic!("RPC error: {}", err["message"].as_str().unwrap_or("?"));
    }
    json["result"].clone()
}

fn hex_to_u256(s: &str) -> u128 {
    let h = s.strip_prefix("0x").unwrap_or(s);
    u128::from_str_radix(h, 16).expect("hex parse")
}

// ── Live broadcast test ─────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires SEPOLIA_PRIVATE_KEY env var and Sepolia ETH"]
async fn live_broadcast_sepolia() {
    let pk_hex = std::env::var("SEPOLIA_PRIVATE_KEY")
        .expect("Set SEPOLIA_PRIVATE_KEY=0x... to run this test");
    let pk_hex = pk_hex.strip_prefix("0x").unwrap_or(&pk_hex);

    // Derive address using cast (handles SEC1 encoding correctly)
    let secret_bytes = hex::decode(pk_hex).expect("Private key must be hex");
    assert_eq!(secret_bytes.len(), 32, "Private key must be 32 bytes");
    let secret_key = k256::SecretKey::from_slice(&secret_bytes).expect("Invalid key");
    let output = std::process::Command::new("cast")
        .args(["wallet", "address", "--private-key", &format!("0x{pk_hex}")])
        .output()
        .expect("cast wallet address failed");
    let address = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(
        address.starts_with("0x"),
        "cast should return a valid address"
    );
    eprintln!("Address: {address}");

    // Check balance
    let bal_hex = json_rpc(
        RPC,
        "eth_getBalance",
        serde_json::json!([address, "latest"]),
    )
    .await;
    let balance = hex_to_u256(bal_hex.as_str().unwrap_or("0x0"));
    if balance == 0 {
        panic!(
            "❌ SEPOLIA NODE REJECTED: Address {address} has 0 balance.\n   Please fund it from a Sepolia faucet."
        );
    }
    eprintln!("Balance: {balance} wei");

    // Get nonce + gas price
    let nonce_hex = json_rpc(
        RPC,
        "eth_getTransactionCount",
        serde_json::json!([address, "latest"]),
    )
    .await;
    let nonce = hex_to_u256(nonce_hex.as_str().unwrap_or("0x0")) as u64;
    eprintln!("Nonce: {nonce}");

    let gas_price_hex = json_rpc(RPC, "eth_gasPrice", serde_json::json!([])).await;
    let gas_price = hex_to_u256(gas_price_hex.as_str().unwrap_or("0x0"));
    eprintln!("Gas price: {gas_price} wei");

    let max_priority = 1_000_000_000u64; // 1 gwei
    let max_fee = (gas_price * 12 / 10) as u64;
    let max_fee = max_fee.max(max_priority);

    // Build EIP-1559 unsigned tx fields
    let to = hex::decode("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045").expect("test invariant");
    let value = 1_000_000_000_000_000u128; // 0.001 ETH
    let gas_limit = 21_000u64;

    let fields = vec![
        rlp_bytes(&rlp_u64(SEPOLIA_CHAIN_ID)),
        rlp_bytes(&rlp_u64(nonce)),
        rlp_bytes(&rlp_u64(max_priority)),
        rlp_bytes(&rlp_u64(max_fee)),
        rlp_bytes(&rlp_u64(gas_limit)),
        rlp_bytes(&to),
        rlp_bytes(&rlp_u64(value as u64)),
        vec![0x80], // empty data
        vec![0xc0], // empty access list
    ];
    let list_rlp = rlp_list(&fields);
    let mut tx_bytes = vec![0x02u8];
    tx_bytes.extend_from_slice(&list_rlp);

    // Sign: keccak256(0x02 || rlp([chain_id, ..., access_list]))
    let hash = crypto_core::hash::keccak256(&tx_bytes);
    let plugin = EvmPlugin::new();
    let signing_key = k256::ecdsa::SigningKey::from(&secret_key);

    use k256::ecdsa::signature::hazmat::PrehashSigner;
    let (sig, recid) = signing_key.sign_prehash_recoverable(&hash);
    // Verify the signature is valid for our key
    use k256::ecdsa::signature::hazmat::PrehashVerifier;
    signing_key
        .verifying_key()
        .verify_prehash(&hash, &sig)
        .expect("Signature should be valid for our key");
    eprintln!("✅ Signature valid");

    // Try both y_parity values to find which recovers our address
    use k256::ecdsa::VerifyingKey;
    let y_parity = {
        let mut found = recid.is_y_odd();
        eprintln!("recid: is_y_odd={}, raw={:?}", found, recid);
        for candidate in [false, true] {
            if let Ok(recovered) = VerifyingKey::recover_from_prehash(
                &hash[..],
                &sig,
                k256::ecdsa::RecoveryId::new(candidate, false),
            ) {
                let rbytes = recovered.to_sec1_point(false).to_bytes();
                let raddr = hex::encode(&crypto_core::hash::keccak256(&rbytes[1..])[12..]);
                if raddr == address[2..] {
                    eprintln!("✅ Recovery ID: y_parity={candidate}");
                    found = candidate;
                    break;
                }
            }
        }
        found
    };

    let r = sig.r().to_bytes().to_vec();
    let s = sig.s().to_bytes().to_vec();

    // Build signed tx
    let rlp_chain_id = rlp_u64(SEPOLIA_CHAIN_ID);
    let rlp_nonce = rlp_u64(nonce);
    let rlp_priority = rlp_u64(max_priority);
    let rlp_gas = rlp_u64(gas_limit);
    let rlp_value = rlp_u64(value as u64);
    let empty: Vec<u8> = vec![];

    let signed_fields = vec![
        rlp_bytes(&rlp_chain_id),
        rlp_bytes(&rlp_nonce),
        rlp_bytes(&rlp_priority),
        rlp_bytes(&rlp_u64(max_fee)),
        rlp_bytes(&rlp_gas),
        rlp_bytes(&to),
        rlp_bytes(&rlp_value),
        rlp_bytes(&empty),
        vec![0xc0],                                    // access list (empty list)
        rlp_int(if y_parity { &[0x01] } else { &[] }), // V
        rlp_int(&r),                                   // R
        rlp_int(&s),                                   // S
    ];
    let signed_list = rlp_list(&signed_fields);
    let mut signed_tx = vec![0x02u8];
    signed_tx.extend_from_slice(&signed_list);

    eprintln!("Signed tx: 0x{}", hex::encode(&signed_tx));

    // Broadcast
    let tx_hash = plugin
        .broadcast_transaction(&signed_tx, "sepolia")
        .await
        .expect("broadcast_transaction should succeed");
    eprintln!("Tx broadcast: {tx_hash}");

    // Poll for receipt (60s timeout)
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(60);
    let mut receipt: Option<serde_json::Value> = None;

    while start.elapsed() < timeout {
        let result = json_rpc(
            RPC,
            "eth_getTransactionReceipt",
            serde_json::json!([tx_hash]),
        )
        .await;
        if !result.is_null() {
            receipt = Some(result);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    let receipt = receipt.expect("Timeout waiting for transaction receipt (60s)");
    let status = receipt["status"]
        .as_str()
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok());
    let block = receipt["blockNumber"]
        .as_str()
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok());

    eprintln!("Block: #{block:?}");
    assert_eq!(status, Some(1), "Transaction should succeed (status=1)");

    eprintln!("\n🎉 Transaction confirmed in block #{block:?}!");
    eprintln!("   TxHash: {tx_hash}");
}
