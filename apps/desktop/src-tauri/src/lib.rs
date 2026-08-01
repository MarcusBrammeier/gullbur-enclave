/// Gullbúr Enclave — Tauri v2 desktop shell library.
///
/// This module serves as the bridge between the Svelte 5 frontend and
/// the Rust vault-core engine. All cryptographic operations happen
/// exclusively in the Rust memory heap behind Tauri command handlers.
///
/// SECURITY: The isolation pattern creates a sandboxed iframe that
/// encrypts ALL IPC payloads with SubtleCrypto AES-GCM before they
/// reach the Rust backend. The main window's Svelte code never has
/// access to the encryption key.
///
/// SAFETY: The crash reporter writes panic info to disk. It captures
/// ONLY version string, panic message, and file:line backtrace.
/// NO seed data, keys, transactions, or addresses are ever included.
mod commands;
mod isolation;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

/// Install a privacy-safe panic hook that writes crash reports to disk.
/// Captures only version, panic message, and file:line backtrace.
/// Never includes seed data, keys, transactions, or addresses.
fn install_crash_reporter() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let version = env!("CARGO_PKG_VERSION");
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let panic_msg = panic_info
            .to_string()
            .lines()
            .next()
            .unwrap_or("unknown panic")
            .to_string();
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".into());

        // Capture a backtrace string (non-verbose to avoid PII leaks in log)
        let backtrace = std::backtrace::Backtrace::capture();
        let backtrace_str = format!("{backtrace}");

        let payload = serde_json::json!({
            "version": version,
            "timestamp": timestamp,
            "panic": panic_msg,
            "location": location,
            "backtrace": backtrace_str,
        });

        let crash_dir = dirs_next::home_dir()
            .unwrap_or_default()
            .join(".gullbur")
            .join("crashes");
        let _ = std::fs::create_dir_all(&crash_dir);
        let path = crash_dir.join(format!("crash-{timestamp}.json"));
        let _ = std::fs::write(
            &path,
            serde_json::to_string_pretty(&payload).unwrap_or_default(),
        );
        // Still call the default hook so the user sees the panic too
        default_hook(panic_info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_crash_reporter();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init());

    let builder = if cfg!(debug_assertions) {
        builder.plugin(tauri_mcp::init())
    } else {
        builder
    };

    builder
        .setup(|app| {
            let vault_state = Arc::new(RwLock::new(commands::VaultState::default()));
            let isolation_state = Arc::new(isolation::IsolationState::new());

            // Mark key as not yet sent — first IPC from iframe will request it
            tracing::info!(
                "[isolation] Session key generated: {}...",
                &isolation_state.key_hex[..16]
            );

            // Set TMPDIR to app cache dir so IPC token file works on Android.
            {
                let cache = app.path().app_cache_dir().ok();
                if let Some(dir) = cache {
                    // SAFETY: set_var is marked unsafe in edition 2024; we
                    // control the env lifecycle in the Tauri setup context.
                    unsafe {
                        std::env::set_var("TMPDIR", &dir);
                    }
                    tracing::info!("IPC token dir: {}", dir.display());
                }
            }

            // Auto-launch IPC server on startup. The launch_ipc_server command
            // handler also checks ipc_handle before re-launching, so a second
            // call from the frontend is safe (no-op if already running).
            let vs = vault_state.clone();
            tauri::async_runtime::spawn(async move {
                let state = vs.read().await;
                let mut vault_guard = state.vault.write().await;
                if let Some(ref mut vault) = *vault_guard {
                    // Try to restore a previously persisted keystore.
                    // If the keystore exists and decrypts successfully,
                    // the vault starts already initialized — the user
                    // can use it without re-entering their mnemonic.
                    match vault.try_restore().await {
                        Ok(()) => {
                            if vault.initialized.load(std::sync::atomic::Ordering::SeqCst) {
                                tracing::info!("Vault state restored from keystore on disk");
                            }
                        }
                        Err(e) => {
                            tracing::info!("No persisted keystore to restore ({e}) — fresh start");
                        }
                    }

                    match vault.launch(state.ipc_port, None).await {
                        Ok(()) => {
                            let mut h = state.ipc_handle.write().await;
                            *h = vault.take_ipc_handle();
                            tracing::info!(
                                "vault-core IPC server launched on 127.0.0.1:{}",
                                state.ipc_port
                            );
                        }
                        Err(e) => {
                            tracing::error!("Failed to launch vault IPC server: {e}");
                        }
                    }
                }
            });

            // Force window to configured size on first paint.
            // Tauri v2 on X11 starts webviews at 10×10 on headless Xvfb;
            // setup() runs too early so we defer via async_runtime::spawn.
            if let Some(window) = app.get_webview_window("main") {
                let win = window.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    let _ = win
                        .set_size(tauri::LogicalSize::new(1200.0, 800.0))
                        .inspect_err(|e| tracing::warn!("set_size: {e}"));
                });
            }

            app.manage(vault_state);
            app.manage(isolation_state);

            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
            tracing::info!("Gullbúr Enclave vault initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_vault_status,
            commands::initialize_vault,
            commands::create_account,
            commands::get_balance,
            commands::sign_transaction,
            commands::broadcast_transaction,
            commands::get_transaction_history,
            commands::list_accounts,
            // Phase 2
            commands::execute_batch,
            commands::request_session_key,
            commands::simulate_and_send,
            commands::simulate_transfer,
            commands::launch_ipc_server,
            // Isolation Pattern
            commands::__get_isolation_key,
            commands::isolation_invoke,
            // Auth
            commands::lock_vault,
            commands::confirm_hardware,
            commands::get_security_stats,
            // Tor
            commands::toggle_tor,
            // Account management
            commands::rename_account,
            // Seed re-export
            commands::get_seed_phrase,
            // Vault file management
            commands::open_vault_from_path,
            commands::open_vault_from_bytes,
            commands::export_current_keystore,
            // Biometric unlock
            commands::biometric_unlock_vault,
            // Bug Reporter
            commands::report_bug,
            // Update Checker
            commands::check_for_updates,
            // Debug Report
            commands::generate_debug_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
