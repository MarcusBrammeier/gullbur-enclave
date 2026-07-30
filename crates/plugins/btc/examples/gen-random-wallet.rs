//! Generate a random testnet Bitcoin wallet.
//! Run: cargo run -p plugin-btc --example gen-random-wallet
use plugin_btc::BtcPlugin;
use wallet_plugin::WalletPlugin;

fn main() {
    let mut rng = rand::thread_rng();
    let mut seed = [0u8; 32];
    use rand::RngCore;
    rng.fill_bytes(&mut seed);
    let plugin = BtcPlugin::new(None);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let account = rt.block_on(plugin.create_account(&seed, 0, "bitcoin-testnet")).unwrap();
    println!("Address: {}", account.address);
    println!("Path:    {}", account.path.unwrap_or_default());
    println!("Seed:    {}", hex::encode(&seed));
}