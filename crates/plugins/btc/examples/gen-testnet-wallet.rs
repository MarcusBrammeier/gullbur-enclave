//! Generate a testnet Bitcoin wallet and print the address.
//! Run: cargo run -p plugin-btc --example gen-testnet-wallet
use plugin_btc::BtcPlugin;
use wallet_plugin::WalletPlugin;

fn main() {
    let seed = b"gullbur-testnet-seed-2026-07-18";
    let plugin = BtcPlugin::new(None);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let account = rt
        .block_on(plugin.create_account(seed, 0, "bitcoin-testnet"))
        .unwrap();
    println!("Address: {}", account.address);
    println!("Path:    {}", account.path.unwrap_or_default());
    println!("Seed:    {}", hex::encode(seed));
}
