//! Ad-hoc verification: IPC handler registration smoke test.
//! Ensures all 13 vault methods are registered (not 404) when
//! register_vault_handlers is called.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;

use auth_core::AuthManager;
use ipc_core::handler::{DispatchResult, MessageHandler};
use ipc_protocol::JsonRpcRequest;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

const METHODS: &[&str] = &[
    "vault.initialize",
    "vault.status",
    "vault.create_account",
    "vault.get_balance",
    "vault.sign_transaction",
    "vault.broadcast_transaction",
    "vault.get_transaction_history",
    "vault.estimate_fee",
    "vault.list_networks",
    "vault.validate_address",
    "vault_executeBatch",
    "vault_requestSessionKey",
    "vault_simulateAndSend",
];

#[tokio::test]
async fn all_13_methods_registered() {
    let mut handler = MessageHandler::new();
    let plugin_host = Arc::new(RwLock::new(PluginHost::new()));
    let seed = Arc::new(RwLock::new(None::<zeroize::Zeroizing<Vec<u8>>>));
    let initialized = Arc::new(AtomicBool::new(false));
    let approval_queue = Arc::new(RwLock::new(vault_core::approval::ApprovalQueue::new()));

    let auth_manager = Arc::new(AuthManager::new());
    let mn = Arc::new(RwLock::new(None));
    ipc_handlers::register_vault_handlers(
        &mut handler,
        plugin_host,
        seed,
        mn,
        initialized,
        approval_queue,
        auth_manager,
    );

    let mut passed = 0u32;

    for method in METHODS {
        let req = JsonRpcRequest::new(*method, None, 1);
        match handler.dispatch(req).await {
            DispatchResult::Success(_) => passed += 1,
            DispatchResult::Error(err) if err.error.code == -32601 => {
                panic!("Method {method} returned method_not_found — not registered");
            }
            DispatchResult::Error(_) => {
                // Operational error (e.g. invalid params) means method WAS found
                passed += 1;
            }
        }
    }

    assert_eq!(
        passed,
        METHODS.len() as u32,
        "All {} methods should be registered",
        METHODS.len()
    );
}
