//! Test server factory — spawns a vault IPC server with all 4 plugins.
//!
//! Mirrors `apps/cli/src/vault.rs` plugin registration.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::RwLock;

use auth_core::AuthManager;
use vault_core::approval::ApprovalQueue;
use vault_core::host::PluginHost;
use vault_core::ipc_handlers;

/// Spawn a vault test server on `port` with all 4 plugins registered (BTC,
/// EVM, XMR, LTC) and all vault IPC handlers wired.
///
/// Returns `(auth_token, join_handle, auth_manager)` — the token string, the server
/// task handle, and the AuthManager (for test-controlled lock/unlock).
/// Clean stale accounts/keystore from disk so fresh tests start clean.
fn clean_stale_state() {
    if let Some(home) = std::env::var_os("HOME") {
        let mut path = std::path::PathBuf::from(home);
        path.push(".gullbur");
        let _ = std::fs::remove_file(path.join("accounts.json"));
        let _ = std::fs::remove_file(path.join("keystore"));
    }
}

pub async fn spawn_test_server(
    port: u16,
) -> (String, tokio::task::JoinHandle<()>, Arc<AuthManager>) {
    clean_stale_state();
    let server =
        ipc_core::server::IpcServer::with_encryption(port, false)
            .expect("create IpcServer");
    let token_path = server.auth_token_path().to_path_buf();
    let auth_token =
        std::fs::read_to_string(&token_path).expect("read auth token");

    let auth_manager = Arc::new(AuthManager::new());
    {
        let mut handler = server.handler().await;
        let plugin_host = Arc::new(RwLock::new(PluginHost::new()));
        let seed = Arc::new(RwLock::new(None::<zeroize::Zeroizing<Vec<u8>>>));
        let initialized = Arc::new(AtomicBool::new(false));
        let mn = Arc::new(RwLock::new(None::<String>));
        let approval_queue =
            Arc::new(RwLock::new(ApprovalQueue::new()));

        // Register all 4 plugins
        {
            let mut host = plugin_host.write().await;
            host.register(Arc::new(plugin_btc::BtcPlugin::new(None)));
            host.register(Arc::new(plugin_evm::EvmPlugin::new()));
            host.register(Arc::new(plugin_xmr::XmrPlugin::new()));
            host.register(Arc::new(plugin_ltc::LtcPlugin::new()));
        }

        ipc_handlers::register_vault_handlers(
            &mut handler,
            plugin_host,
            seed,
            mn,
            initialized,
            approval_queue,
            auth_manager.clone(),
        );
    }

    let handle = server.run();
    // Give the server time to bind
    tokio::time::sleep(Duration::from_millis(200)).await;

    (auth_token, handle, auth_manager)
}