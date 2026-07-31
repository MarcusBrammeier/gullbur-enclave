/// Headless vault launcher — starts the vault IPC server from the CLI.
/// Optionally starts an HTTP JSON-RPC bridge.
/// Only compiled when the `headless` feature is enabled.
#[cfg(feature = "headless")]
use std::sync::Arc;
#[cfg(feature = "headless")]
use std::sync::atomic::AtomicBool;
#[cfg(feature = "headless")]
use tokio::sync::RwLock;

/// Start the vault IPC server in-process and block until Ctrl+C.
/// Optionally starts an HTTP JSON-RPC bridge on `http_port`.
#[cfg(feature = "headless")]
pub async fn run_headless_vault(
    ipc_port: u16,
    tor_socks_port: Option<u16>,
    http_port: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // Create the plugin host with default plugins
    let host = crate::vault::create_plugin_host().await?;
    let plugin_host = Arc::new(RwLock::new(host));

    // Seed storage, initialized flag, approval queue, auth
    let seed: Arc<RwLock<Option<zeroize::Zeroizing<Vec<u8>>>>> = Arc::new(RwLock::new(None));
    let initialized = Arc::new(AtomicBool::new(false));
    let approval_queue = Arc::new(RwLock::new(vault_core::approval::ApprovalQueue::new()));

    let auth_manager = Arc::new(auth_core::AuthManager::new());
    let mnemonic: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

    // Configure and start the lifecycle manager (no encryption for CLI)
    let mut lifecycle = vault_core::lifecycle::LifecycleManager::new().with_no_encrypt();
    lifecycle = lifecycle.with_ipc_port(ipc_port);
    if let Some(tor) = tor_socks_port {
        lifecycle = lifecycle.with_tor_port(tor);
    }

    let _ipc_handle = lifecycle
        .start(
            plugin_host.clone(),
            seed.clone(),
            mnemonic.clone(),
            initialized.clone(),
            approval_queue.clone(),
            auth_manager.clone(),
        )
        .await?;

    println!("Vault IPC server running on 127.0.0.1:{ipc_port}");

    // Start HTTP JSON-RPC bridge if requested
    if let Some(http) = http_port {
        let bridge = Arc::new(vault_core::vault_bridge::VaultBridgeImpl::new(
            plugin_host.clone(),
            initialized.clone(),
            seed.clone(),
        ));
        let bridge_handle = tokio::spawn(async move {
            if let Err(e) = ipc_core::http_bridge::run(bridge, http).await {
                eprintln!("HTTP bridge error: {e}");
            }
        });
        println!("HTTP JSON-RPC bridge on http://127.0.0.1:{http}/json-rpc");
        // Keep bridge handle alive until Ctrl+C
        tokio::select! {
            _ = bridge_handle => {},
            _ = tokio::signal::ctrl_c() => {},
        }
    } else {
        println!("Press Ctrl+C to stop");
        tokio::signal::ctrl_c().await?;
    }

    println!("\nShutting down...");

    // Graceful shutdown
    lifecycle.shutdown().await?;
    Ok(())
}
