//! End-to-end integration test for the vault-core IPC handler pipeline.
//!
//! Exercises the full flow: initialize → create account → get balance →
//! sign transaction → broadcast transaction, using the JSON-RPC
//! dispatch mechanism directly (no WebSocket).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;

use auth_core::AuthManager;
use ipc_core::handler::{DispatchResult, MessageHandler};
use ipc_protocol::JsonRpcRequest;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

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

#[tokio::test]
async fn e2e_signing_flow() {
    // ── Setup: shared state -------------------------------------------------
    let mut handler = MessageHandler::new();
    let plugin_host = Arc::new(RwLock::new(PluginHost::new()));
    let seed = Arc::new(RwLock::new(None::<zeroize::Zeroizing<Vec<u8>>>));
    let initialized = Arc::new(AtomicBool::new(false));
    let approval_queue = Arc::new(RwLock::new(vault_core::approval::ApprovalQueue::new()));
    let mn = Arc::new(RwLock::new(None));

    // Register EVM plugin manually before initialize so it's available even
    // when the `plugins` feature is not enabled.
    {
        let mut host = plugin_host.write().await;
        host.register(Arc::new(plugin_evm::EvmPlugin::new()));
    }

    // Register all IPC handlers
    let auth_manager = Arc::new(AuthManager::new());
    let am_for_test = Arc::clone(&auth_manager);
    ipc_handlers::register_vault_handlers(
        &mut handler,
        plugin_host,
        seed,
        mn,
        initialized,
        approval_queue,
        auth_manager,
    );

    // ── Step 1: vault.initialize ───────────────────────────────────────────
    let mnemonic = crypto_core::keys::generate_mnemonic(crypto_core::MnemonicStrength::TwelveWords)
        .expect("test invariant");
    let result = handler
        .dispatch(JsonRpcRequest::new(
            "vault.initialize",
            Some(serde_json::json!({ "seed_phrase": mnemonic.to_string() })),
            1,
        ))
        .await;

    let init_val = match &result {
        DispatchResult::Success(resp) => {
            assert_eq!(resp.id, 1);
            assert!(
                resp.result
                    .get("initialized")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                "vault.initialize should set initialized=true"
            );
            resp.result.clone()
        }
        DispatchResult::Error(err) => {
            panic!("vault.initialize returned an error: {:?}", err);
        }
    };
    assert!(
        init_val.get("master_key").is_some(),
        "initialize response should contain master_key"
    );

    // ── Step 2: vault.status ───────────────────────────────────────────────
    let result = handler
        .dispatch(JsonRpcRequest::new("vault.status", None, 2))
        .await;

    let status_val = match &result {
        DispatchResult::Success(resp) => {
            assert_eq!(resp.id, 2);
            resp.result.clone()
        }
        DispatchResult::Error(err) => {
            panic!("vault.status returned an error: {:?}", err);
        }
    };
    assert_eq!(
        status_val.get("initialized").and_then(|v| v.as_bool()),
        Some(true),
        "vault.status should report initialized=true"
    );
    let plugin_ids = status_val
        .get("plugin_ids")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        plugin_ids >= 1,
        "Expected at least 1 registered plugin, got {plugin_ids}"
    );

    // ── Step 3: vault.create_account ───────────────────────────────────────
    let result = handler
        .dispatch(JsonRpcRequest::new(
            "vault.create_account",
            Some(serde_json::json!({
                "network": "ethereum",
                "index": 0,
            })),
            3,
        ))
        .await;

    let account_val = match &result {
        DispatchResult::Success(resp) => {
            assert_eq!(resp.id, 3);
            resp.result.clone()
        }
        DispatchResult::Error(err) => {
            panic!("vault.create_account returned an error: {:?}", err);
        }
    };
    let address = account_val
        .get("address")
        .and_then(|v| v.as_str())
        .expect("account should have an address")
        .to_string();
    assert!(
        address.starts_with("0x"),
        "Ethereum address should start with 0x, got: {address}"
    );
    assert_eq!(address.len(), 42, "Ethereum address should be 42 chars");
    let account_id = account_val
        .get("id")
        .and_then(|v| v.as_str())
        .expect("account should have an id")
        .to_string();
    assert_eq!(account_id, "ethereum-0", "expected network-index id format");

    // ── Step 4: vault.get_balance ──────────────────────────────────────────
    let result = handler
        .dispatch(JsonRpcRequest::new(
            "vault.get_balance",
            Some(serde_json::json!({
                "network": "ethereum",
                "address": address,
            })),
            4,
        ))
        .await;

    // get_balance may succeed (network available) or fail with an operational
    // error (no network / RPC unreachable). Either is acceptable as long as
    // the handler was found (not -32601).
    match &result {
        DispatchResult::Success(resp) => {
            assert_eq!(resp.id, 4);
            let bal = &resp.result;
            // Balance response should at least have a unit field
            assert!(
                bal.get("unit").and_then(|v| v.as_str()).is_some(),
                "balance should have a unit field: {bal}"
            );
        }
        DispatchResult::Error(err) => {
            assert_ne!(
                err.error.code, -32601,
                "vault.get_balance must be a registered handler"
            );
            // Operational error (timeout, network unreachable) is expected
            // when running without live RPC endpoints.
        }
    }

    // ── Authenticate before signing ────────────────────────────────────────
    let _ = am_for_test.try_biometric();

    // ── Step 5: vault.sign_transaction ─────────────────────────────────────
    // Build a minimal valid EIP-1559 transaction:
    //   0x02 || RLP([chain_id, nonce, max_priority_fee, max_fee,
    //               gas_limit, to, value, data, access_list])
    let chain_id = vec![0x01u8]; // Ethereum mainnet
    let zero_str = rlp_string(&[]); // RLP-encoded empty byte string (0x80)
    let fee_21000 = rlp_string(&[0x52, 0x08]); // 21000 in big-endian
    let to = rlp_string(&[0u8; 20]); // zero address
    let empty_data = rlp_string(&[]); // empty calldata
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

    let result = handler
        .dispatch(JsonRpcRequest::new(
            "vault.sign_transaction",
            Some(serde_json::json!({
                "network": "ethereum",
                "tx_hex": tx_hex,
                "key_id": account_id,
                "key_type": "Secp256k1",
            })),
            5,
        ))
        .await;

    let sign_val = match &result {
        DispatchResult::Success(resp) => {
            assert_eq!(resp.id, 5);
            resp.result.clone()
        }
        DispatchResult::Error(err) => {
            panic!("vault.sign_transaction returned an error: {:?}", err);
        }
    };
    let signed_tx_hex = sign_val
        .get("signed_tx_hex")
        .and_then(|v| v.as_str())
        .expect("sign response should contain signed_tx_hex")
        .to_string();
    assert!(
        signed_tx_hex.len() > tx_hex.len(),
        "signed transaction should be longer than unsigned (contains v,r,s)"
    );

    // ── Step 6: vault.broadcast_transaction ────────────────────────────────
    let result = handler
        .dispatch(JsonRpcRequest::new(
            "vault.broadcast_transaction",
            Some(serde_json::json!({
                "network": "ethereum",
                "signed_tx_hex": signed_tx_hex,
            })),
            6,
        ))
        .await;

    // broadcast may fail without network access; just verify handler exists.
    match &result {
        DispatchResult::Success(resp) => {
            assert_eq!(resp.id, 6);
            assert!(
                resp.result.get("txid").is_some(),
                "broadcast response should contain txid"
            );
        }
        DispatchResult::Error(err) => {
            assert_ne!(
                err.error.code, -32601,
                "vault.broadcast_transaction must be a registered handler"
            );
        }
    }
}
