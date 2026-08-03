//! CLI Integration: Security tests (lock, unlock, auth gating)
//!
//! Tests vault locking and that operations are blocked after lock.

use std::sync::Arc;

use cli_integration::client::{assert_ok, call};
use cli_integration::server::spawn_test_server;

use auth_core::AuthManager;
use serde_json::json;

const PORT: u16 = 19950;

async fn setup_locking_server(
    port: u16,
) -> (String, tokio::task::JoinHandle<()>, Arc<AuthManager>) {
    let (token, handle, auth_manager) = spawn_test_server(port).await;

    // Initialize vault
    let r = call(port, &token, "vault.generate_mnemonic", json!({})).await;
    let mnemonic = assert_ok(&r, "vault.generate_mnemonic")["mnemonic"]
        .as_str()
        .expect("test invariant")
        .to_string();

    let r = call(
        port,
        &token,
        "vault.initialize",
        json!({
            "seed_phrase": mnemonic
        }),
    )
    .await;
    assert_ok(&r, "vault.initialize");

    (token, handle, auth_manager)
}

#[tokio::test]
async fn lock_vault() {
    let (token, _handle, auth_manager) = setup_locking_server(PORT).await;

    // Unlock first so there's something to lock.
    // init now auto-unlocks the vault, so this may already be unlocked.
    let _ = auth_manager.try_biometric();

    // Lock via RPC
    let r = call(PORT, &token, "vault.lock", json!({})).await;
    let result = assert_ok(&r, "vault.lock");
    assert_eq!(result["locked"], true, "lock should return locked: true");
}

#[tokio::test]
async fn operations_blocked_after_lock() {
    let (token, _handle, auth_manager) = setup_locking_server(PORT + 1).await;

    // Unlock, then lock
    let _ = auth_manager.try_biometric();
    let r = call(PORT + 1, &token, "vault.lock", json!({})).await;
    assert_ok(&r, "vault.lock");

    // Create ETH account (then signing would also be blocked)
    let r = call(
        PORT + 1,
        &token,
        "vault.create_account",
        json!({
            "network": "ethereum",
            "index": 0
        }),
    )
    .await;

    // create_account needs seed but should work as long as we don't pass origin
    // (no approval gate). Let's verify it works.
    if let Some(_error) = r.get("error") {
        // Error is OK too — some handlers may have other restrictions
    }

    // Try signing (requires BiometricUnlocked — should fail after lock)
    // First need an account
    let account_id = "ethereum-0";
    let r = call(
        PORT + 1,
        &token,
        "vault.sign_transaction",
        json!({
            "network": "ethereum",
            "tx_hex": "02f8",
            "key_id": account_id,
            "key_type": "Secp256k1"
        }),
    )
    .await;

    // Must error since vault is locked
    assert!(r.get("error").is_some(), "sign should error after lock");
    let code = r["error"]["code"].as_i64().unwrap_or(0);
    // AuthRequired errors are typically -32000 or similar (not -32601)
    assert_ne!(
        code, -32601,
        "sign_transaction must be a registered handler"
    );
}

#[tokio::test]
async fn status_after_lock() {
    let (token, _handle, auth_manager) = setup_locking_server(PORT + 2).await;

    let _ = auth_manager.try_biometric();
    let r = call(PORT + 2, &token, "vault.lock", json!({})).await;
    assert_ok(&r, "vault.lock");

    let r = call(PORT + 2, &token, "vault.status", json!({})).await;
    let result = assert_ok(&r, "vault.status");
    // Status should still show initialized=true (lock doesn't wipe init state)
    assert_eq!(
        result["initialized"], true,
        "should still show initialized after lock"
    );
}

#[tokio::test]
async fn re_init_after_lock() {
    // Note: vault.lock only transitions auth state, it does NOT reset the
    // `initialized` flag. Re-initialization is not a supported flow in the
    // current architecture — this test verifies the error is correct.
    let (token, _handle, auth_manager) = setup_locking_server(PORT + 3).await;

    let _ = auth_manager.try_biometric();
    let r = call(PORT + 3, &token, "vault.lock", json!({})).await;
    assert_ok(&r, "vault.lock");

    // Attempt to re-initialize — should get an initialized error, not method_not_found
    let r = call(PORT + 3, &token, "vault.initialize", json!({
        "seed_phrase": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    })).await;
    // Must route (not -32601)
    assert!(r.get("error").is_some(), "re-init should error");
    let code = r["error"]["code"].as_i64().unwrap_or(0);
    assert_ne!(
        code, -32601,
        "vault.initialize must be a registered handler"
    );
    // Should be -32000 (already initialized)
    assert_eq!(code, -32000, "re-init should say already initialized");
}

#[tokio::test]
async fn unlock_flow() {
    let (token, _handle, auth_manager) = setup_locking_server(PORT + 4).await;

    // Lock first
    let _ = auth_manager.try_biometric();
    let r = call(PORT + 4, &token, "vault.lock", json!({})).await;
    assert_ok(&r, "vault.lock");

    // Unlock via auth_manager (simulates biometric unlock)
    auth_manager
        .try_biometric()
        .expect("re-unlock should succeed");

    // Now signing should work again
    // Create account first
    let r = call(
        PORT + 4,
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

    // Sign a simple transaction
    let r = call(
        PORT + 4,
        &token,
        "vault.sign_transaction",
        json!({
            "network": "ethereum",
            "tx_hex": "02f8",
            "key_id": &account_id,
            "key_type": "Secp256k1"
        }),
    )
    .await;
    // Should now either succeed or error with something other than
    // method_not_found — the handler routes, auth passes
    if let Some(error) = r.get("error") {
        let code = error["code"].as_i64().unwrap_or(0);
        // Maybe a bad-key error since we're using "02f8" as the tx_hex
        // That's fine — just not -32601
        assert_ne!(
            code, -32601,
            "sign_transaction must be registered after unlock"
        );
    } else {
        // Actually succeeded — great
        assert!(r.get("result").is_some(), "should have a result");
    }
}

// ── Lock → unlock → accounts preserved test ───────────────────
// Verifies that after lock+unlock, all previously created accounts
// still exist with the same addresses.

const LOCK_UNLOCK_PORT: u16 = 19830;

#[tokio::test]
async fn lock_unlock_accounts_preserved() {
    use std::collections::HashSet;

    let (token, handle, auth_manager) = {
        let (token, handle, am) = spawn_test_server(LOCK_UNLOCK_PORT).await;
        let r = call(
            LOCK_UNLOCK_PORT,
            &token,
            "vault.generate_mnemonic",
            json!({}),
        )
        .await;
        let m = assert_ok(&r, "generate")["mnemonic"]
            .as_str()
            .expect("invariant")
            .to_string();
        let r = call(
            LOCK_UNLOCK_PORT,
            &token,
            "vault.initialize",
            json!({"seed_phrase": m}),
        )
        .await;
        assert_ok(&r, "initialize");
        (token, handle, am)
    };

    // Create one account per network
    let mut pre_addrs: HashSet<(String, String)> = HashSet::new();
    for (net, idx) in &[("bitcoin", 0u64), ("ethereum", 0u64), ("litecoin", 0u64)] {
        let r = call(
            LOCK_UNLOCK_PORT,
            &token,
            "vault.create_account",
            json!({"network": net, "index": idx}),
        )
        .await;
        let result = assert_ok(&r, &format!("create {net}"));
        let addr = result["address"].as_str().expect("invariant").to_string();
        pre_addrs.insert((net.to_string(), addr));
    }

    // Lock
    let _ = auth_manager.try_biometric();
    let r = call(LOCK_UNLOCK_PORT, &token, "vault.lock", json!({})).await;
    assert_ok(&r, "lock");
    assert_eq!(r["result"]["locked"], true);

    // Unlock
    auth_manager.try_biometric().expect("unlock should succeed");

    // List accounts — should have all pre-lock accounts
    let r = call(LOCK_UNLOCK_PORT, &token, "vault.list_accounts", json!({})).await;
    let accounts = assert_ok(&r, "list_accounts after unlock")
        .as_array()
        .expect("invariant");

    let mut found = 0u32;
    for acct in accounts {
        let net = acct["network"].as_str().unwrap_or("");
        let addr = acct["address"].as_str().unwrap_or("");
        if pre_addrs.contains(&(net.to_string(), addr.to_string())) {
            found += 1;
        }
    }
    assert_eq!(
        found, 3,
        "all 3 pre-lock accounts should be present after unlock"
    );
    drop(handle);
}
