//! CLI Integration: Signing and broadcast tests
//!
//! Signs a real EIP-1559 Ethereum transaction via the WebSocket JSON-RPC
//! interface.

use std::sync::Arc;

use cli_integration::client::{assert_ok, call};
use cli_integration::server::spawn_test_server;

use auth_core::AuthManager;
use serde_json::json;

const PORT: u16 = 19960;

/// RLP-encode a byte string (string item, not list).
fn rlp_string(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        return data.to_vec();
    }
    let mut out = vec![0x80u8 + data.len() as u8];
    out.extend_from_slice(data);
    out
}

/// RLP-encode a list from pre-encoded items.
fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let total: usize = items.iter().map(|i| i.len()).sum();
    let mut out = vec![0xc0u8 + total as u8];
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

async fn setup_signing_server(
    port: u16,
) -> (
    String,
    tokio::task::JoinHandle<()>,
    String,
    Arc<AuthManager>,
) {
    let (token, handle, auth_manager) = spawn_test_server(port).await;

    // Generate mnemonic and initialize
    let r = call(port, &token, "vault.generate_mnemonic", json!({})).await;
    let result = assert_ok(&r, "vault.generate_mnemonic");
    let mnemonic = result["mnemonic"]
        .as_str()
        .expect("test invariant")
        .to_string();

    let r = call(
        port,
        &token,
        "vault.initialize",
        json!({
            "seed_phrase": &mnemonic
        }),
    )
    .await;
    let result = assert_ok(&r, "vault.initialize");

    // Extract the seed hex from initialization — the master_key key_id
    // "bip44-eth-0" is not the seed. Instead, derive the seed locally
    // from the mnemonic so we can pass it as the signing key_id.
    let mnemonic_obj = bip39::Mnemonic::parse_normalized(&mnemonic).expect("test invariant");
    let seed_bytes = mnemonic_obj.to_seed("");
    let seed_hex = hex::encode(&seed_bytes);

    // Create ETH account (for reference, signing uses the seed directly)
    let r = call(
        port,
        &token,
        "vault.create_account",
        json!({
            "network": "ethereum",
            "index": 0
        }),
    )
    .await;
    let result = assert_ok(&r, "vault.create_account");
    let account_id = result["id"].as_str().expect("test invariant").to_string();

    (token, handle, seed_hex, auth_manager)
}

#[tokio::test]
async fn sign_eth_transaction() {
    let (token, _handle, account_id, auth_manager) = setup_signing_server(PORT).await;

    // Authenticate (biometric unlock) before signing
    auth_manager
        .try_biometric()
        .expect("biometric unlock should succeed");

    // Build a minimal valid EIP-1559 transaction envelope
    let chain_id = vec![0x01u8]; // Ethereum mainnet
    let zero_str = rlp_string(&[]); // RLP-encoded empty byte string (0x80)
    let fee_21000 = rlp_string(&[0x52, 0x08]); // 21000 in big-endian
    let to = rlp_string(&[0u8; 20]); // zero address
    let empty_data = rlp_string(&[]);
    let access_list = vec![0xc0u8]; // empty list

    let tx_items = vec![
        chain_id,
        zero_str.clone(),
        zero_str.clone(),
        fee_21000.clone(),
        fee_21000,
        to,
        zero_str,
        empty_data,
        access_list,
    ];
    let encoded_list = rlp_list(&tx_items);
    let mut tx_bytes = vec![0x02u8];
    tx_bytes.extend_from_slice(&encoded_list);
    let tx_hex = hex::encode(&tx_bytes);

    let r = call(
        PORT,
        &token,
        "vault.sign_transaction",
        json!({
            "network": "ethereum",
            "tx_hex": tx_hex,
            "key_id": &account_id,
            "key_type": "Secp256k1"
        }),
    )
    .await;
    let result = assert_ok(&r, "vault.sign_transaction");
    let signed_tx_hex = result["signed_tx_hex"].as_str().expect("test invariant");
    assert!(
        signed_tx_hex.len() > tx_hex.len(),
        "signed transaction should be longer than unsigned (contains v,r,s)"
    );
}

#[tokio::test]
async fn sign_with_bad_key_id() {
    let (token, _handle, _account_id, auth_manager) = setup_signing_server(PORT + 1).await;
    auth_manager.try_biometric().expect("biometric unlock");

    let r = call(
        PORT + 1,
        &token,
        "vault.sign_transaction",
        json!({
            "network": "ethereum",
            "tx_hex": "02f8",
            "key_id": "nonexistent-key",
            "key_type": "Secp256k1"
        }),
    )
    .await;
    assert!(
        r.get("error").is_some(),
        "sign with bad key_id should error"
    );
    let code = r["error"]["code"].as_i64().unwrap_or(0);
    assert_ne!(code, -32601, "must be a registered handler");
}

#[tokio::test]
async fn broadcast_invalid_tx() {
    let (token, _handle, _account_id, auth_manager) = setup_signing_server(PORT + 2).await;
    auth_manager.try_biometric().expect("biometric unlock");

    let r = call(
        PORT + 2,
        &token,
        "vault.broadcast_transaction",
        json!({
            "network": "ethereum",
            "signed_tx_hex": "00"
        }),
    )
    .await;
    assert!(
        r.get("error").is_some(),
        "broadcast with garbage hex should error"
    );
    let code = r["error"]["code"].as_i64().unwrap_or(0);
    assert_ne!(
        code, -32601,
        "broadcast_transaction must be a registered handler"
    );
}

#[tokio::test]
async fn broadcast_error_routes() {
    let (token, _handle, _account_id, auth_manager) = setup_signing_server(PORT + 3).await;
    auth_manager.try_biometric().expect("biometric unlock");

    let r = call(
        PORT + 3,
        &token,
        "vault.broadcast_transaction",
        json!({
            "network": "bitcoin",
            "signed_tx_hex": "001122"
        }),
    )
    .await;
    // Handler must route (not -32601) — accept success or operational error
    if let Some(error) = r.get("error") {
        let code = error["code"].as_i64().unwrap_or(0);
        assert_ne!(
            code, -32601,
            "broadcast_transaction must be registered (got method_not_found)"
        );
    } else {
        // Handler routed and succeeded — e.g. BTC plugin returned simulated txid
        assert!(r.get("result").is_some(), "should have result on success");
    }
}
