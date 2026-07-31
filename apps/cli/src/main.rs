use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[cfg(feature = "headless")]
mod headless;
#[cfg(feature = "headless")]
mod vault;

#[derive(Parser)]
#[command(
    name = "gullbur-cli",
    version = "0.0.1",
    about = "Gullbúr Enclave Internal CLI"
)]
struct Cli {
    /// IPC server port (default: 19876)
    #[arg(short = 'p', long = "port", default_value = "19876")]
    port: u16,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the vault IPC server
    Launch {
        #[arg(short = 't', long = "tor-port")]
        tor_port: Option<u16>,
        /// Optional HTTP JSON-RPC bridge port (e.g. 8080)
        #[arg(short = 'H', long = "http-port")]
        http_port: Option<u16>,
    },
    /// Initialize a wallet (generate or restore)
    Init {
        seed_phrase: Option<String>,
        #[arg(short = 'P', long = "passphrase")]
        passphrase: Option<String>,
    },
    /// List all networks
    ListNetworks,
    /// Create a new account on a network
    CreateAccount { network: String, index: Option<u32> },
    /// List all accounts
    ListAccounts,
    /// Get balance for an address
    GetBalance { network: String, address: String },
    /// Generate a new mnemonic
    GenerateMnemonic,
    /// Validate an address
    ValidateAddress { network: String, address: String },
    /// Estimate fee for a transaction
    EstimateFee {
        network: String,
        to: String,
        amount: String,
    },
    /// Sign a transaction (placeholder — takes raw params)
    SignTransaction {
        network: String,
        from: String,
        to: String,
        amount: String,
        #[arg(long = "fee-level", default_value = "medium")]
        fee_level: String,
    },
    /// Broadcast a signed transaction
    BroadcastTransaction { signed_tx: String },
    /// Get transaction history
    TransactionHistory {
        network: String,
        address: String,
        limit: Option<u32>,
    },
    /// Lock the vault
    Lock,
    /// Get vault status
    Status,
}

async fn call(method: &str, params: Value, port: u16) -> Result<Value, String> {
    let url = format!("ws://127.0.0.1:{port}");
    let (ws_stream, _) = connect_async(&url)
        .await
        .map_err(|e| format!("WebSocket connection failed: {e}"))?;

    let (mut write, mut read) = ws_stream.split();

    // Send hello (trusted loopback)
    write
        .send(Message::Text(r#"{"type":"hello"}"#.into()))
        .await
        .map_err(|e| format!("send hello failed: {e}"))?;

    // Wait for session key
    let timeout = tokio::time::timeout(Duration::from_secs(5), read.next());
    match timeout.await {
        Ok(Some(Ok(msg))) => {
            let text = msg.to_text().unwrap_or("");
            let parsed: Value =
                serde_json::from_str(text).map_err(|e| format!("JSON parse: {e}"))?;
            if parsed.get("type").and_then(|v| v.as_str()) != Some("session_key") {
                return Err(format!("Expected session_key, got: {text}"));
            }
        }
        Ok(Some(Err(e))) => return Err(format!("WS error during auth: {e}")),
        Ok(None) => return Err("Connection closed during auth".into()),
        Err(_) => return Err("Auth timeout (5s)".into()),
    }

    // Send the actual RPC call
    let id: u64 = rand::random();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id,
    });
    write
        .send(Message::Text(request.to_string()))
        .await
        .map_err(|e| format!("send rpc failed: {e}"))?;

    // Read response
    let timeout = tokio::time::timeout(Duration::from_secs(30), read.next());
    match timeout.await {
        Ok(Some(Ok(msg))) => {
            let text = msg.to_text().unwrap_or("");
            let parsed: Value =
                serde_json::from_str(text).map_err(|e| format!("JSON parse: {e}"))?;
            if let Some(err) = parsed.get("error") {
                Err(err["message"].as_str().unwrap_or("rpc error").to_string())
            } else {
                Ok(parsed.get("result").cloned().unwrap_or(parsed))
            }
        }
        Ok(Some(Err(e))) => Err(format!("WS error: {e}")),
        Ok(None) => Err("Connection closed".into()),
        Err(_) => Err("Response timeout (30s)".into()),
    }
}

fn print_result(result: Result<Value, String>, json: bool) {
    match result {
        Ok(v) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&v)
                    .expect("JSON serialization of Value should never fail")
            );
        }
        Err(e) => {
            if json {
                println!(r#"{{"error":"{}"}}"#, e);
            } else {
                eprintln!("Error: {e}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let port = cli.port;
    let json = cli.json;

    match &cli.command {
        #[allow(unused_variables)]
        Commands::Launch {
            tor_port,
            http_port,
        } => {
            #[cfg(feature = "headless")]
            {
                if let Err(e) = headless::run_headless_vault(port, *tor_port, *http_port).await {
                    eprintln!("Vault error: {e}");
                    std::process::exit(1);
                }
            }
            #[cfg(not(feature = "headless"))]
            {
                println!(
                    "Launch IPC server via CLI not yet supported — start the desktop app first or use:\\\\n  cargo run -p gullbur-desktop -- --ipc-port {port}"
                );
                println!("Or recompile with: cargo build -p gullbur-cli --features headless");
            }
        }
        Commands::Init {
            seed_phrase,
            passphrase,
        } => {
            let p = passphrase.as_deref().unwrap_or("");
            let r = call(
                "vault.initialize",
                serde_json::json!({"seed_phrase": seed_phrase.as_deref().unwrap_or(""), "passphrase": p}),
                port,
            )
            .await;
            print_result(r, json);
        }
        Commands::ListNetworks => {
            print_result(
                call("vault.list_networks", serde_json::json!({}), port).await,
                json,
            );
        }
        Commands::CreateAccount { network, index } => {
            let idx = index.unwrap_or(0);
            print_result(
                call(
                    "vault.create_account",
                    serde_json::json!({"network": network, "index": idx}),
                    port,
                )
                .await,
                json,
            );
        }
        Commands::ListAccounts => {
            print_result(
                call("vault.list_accounts", serde_json::json!({}), port).await,
                json,
            );
        }
        Commands::GetBalance { network, address } => {
            print_result(
                call(
                    "vault.get_balance",
                    serde_json::json!({"network": network, "address": address}),
                    port,
                )
                .await,
                json,
            );
        }
        Commands::GenerateMnemonic => {
            print_result(
                call("vault.generate_mnemonic", serde_json::json!({}), port).await,
                json,
            );
        }
        Commands::ValidateAddress { network, address } => {
            print_result(
                call(
                    "vault.validate_address",
                    serde_json::json!({"network": network, "address": address}),
                    port,
                )
                .await,
                json,
            );
        }
        Commands::EstimateFee {
            network,
            to,
            amount,
        } => {
            print_result(
                call(
                    "vault.estimate_fee",
                    serde_json::json!({"network": network, "recipient": to, "amount": amount}),
                    port,
                )
                .await,
                json,
            );
        }
        Commands::SignTransaction {
            network,
            from,
            to,
            amount,
            fee_level,
        } => {
            print_result(
                call(
                    "vault.sign_transaction",
                    serde_json::json!({"network": network, "from": from, "to": to, "amount": amount, "feeLevel": fee_level}),
                    port,
                )
                .await,
                json,
            );
        }
        Commands::BroadcastTransaction { signed_tx } => {
            print_result(
                call(
                    "vault.broadcast_transaction",
                    serde_json::json!({"signed_tx": signed_tx}),
                    port,
                )
                .await,
                json,
            );
        }
        Commands::TransactionHistory {
            network,
            address,
            limit,
        } => {
            let lim = limit.unwrap_or(10);
            print_result(
                call(
                    "vault.get_transaction_history",
                    serde_json::json!({"network": network, "address": address, "limit": lim}),
                    port,
                )
                .await,
                json,
            );
        }
        Commands::Lock => {
            print_result(call("vault.lock", serde_json::json!({}), port).await, json);
        }
        Commands::Status => {
            print_result(
                call("vault.status", serde_json::json!({}), port).await,
                json,
            );
        }
    }
}
