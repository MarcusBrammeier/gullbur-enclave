//! Generate a testnet Bitcoin wallet with a fresh seed.
//! Run: cargo run -p plugin-btc --example gen-testnet-wallet
use plugin_btc::BtcPlugin;
use wallet_plugin::WalletPlugin;

fn main() {
    // Fresh seed for a unique address (hex: "gullbur-btc-test-jul20-wallet-2")
    let seed = b"gullbur-btc-test-jul20-wallet-2";
    let plugin = BtcPlugin::new(None);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let account = rt.block_on(plugin.create_account(seed, 0, "bitcoin-testnet")).unwrap();
    println!("Address: {}", account.address);
    println!("Path:    {}", account.path.unwrap_or_default());
    println!("Seed hex: {}", hex::encode(seed));
}