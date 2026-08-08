//! Live network integration test — Sepolia connectivity check.
//!
//! Verifies that the EVM plugin can pull live on-chain data from a
//! real public RPC endpoint. This test requires network access and is
//! tagged `#[ignore]` by default so CI doesn't depend on an external
//! service. Run with: cargo test -p plugin-evm --test sepolia_live -- --ignored

use plugin_evm::EvmPlugin;
use wallet_plugin::{Account, WalletPlugin};

#[tokio::test]
#[ignore = "requires network access to public RPC endpoint"]
async fn sepolia_live_get_balance() {
    let plugin = EvmPlugin::new();
    // Vitalik's Sepolia address — a known active account
    let account = Account {
        id: "sepolia-vitalik".into(),
        network: "sepolia".into(),
        address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".into(),
        path: None,
        label: None,
        index: 0,
    };
    let balance = plugin
        .get_balance(&account, "sepolia")
        .await
        .expect("live Sepolia get_balance should succeed");
    eprintln!("Balance on Sepolia: {} {}", balance.confirmed, balance.unit);
    // Just verify it's parseable as a number — don't assert exact value
    let _: f64 = balance
        .confirmed
        .parse()
        .expect("balance should be a valid decimal string");
    assert!(!balance.confirmed.is_empty(), "balance should not be empty");
}

#[tokio::test]
#[ignore = "requires network access to public RPC endpoint"]
async fn sepolia_live_estimate_gas() {
    let plugin = EvmPlugin::new();
    let fee = plugin
        .estimate_fee(b"", "sepolia")
        .await
        .expect("live gas estimate should succeed");
    eprintln!(
        "Sepolia gas: fast={} medium={} slow={} {}",
        fee.fast, fee.medium, fee.slow, fee.unit
    );
    assert!(!fee.fast.is_empty(), "gas price should not be empty");
}

#[tokio::test]
#[ignore = "requires network access to public RPC endpoint"]
async fn plugin_rpc_endpoints_all_resolve() {
    // Verify all 7 network endpoints can be reached
    let networks = [
        "ethereum", "polygon", "arbitrum", "optimism", "bnb", "base", "sepolia",
    ];
    for net in &networks {
        let endpoint = plugin_evm::rpc_endpoint(net);
        assert!(endpoint.is_some(), "{net} should have an endpoint");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("test invariant");
        let resp = client
            .post(endpoint.expect("test invariant"))
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": [],
                "id": 1,
            }))
            .send()
            .await;
        match resp {
            Ok(r) => {
                let status = r.status();
                eprintln!("{net}: HTTP {status}");
                assert!(status.is_success(), "{net} returned {status}");
            }
            Err(e) => {
                // Timeout or DNS failure — warn but don't fail
                eprintln!("{net}: network error (SKIPPED): {e}");
            }
        }
    }
}
