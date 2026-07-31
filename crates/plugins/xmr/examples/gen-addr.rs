//! Generate XMR stagenet address from saved seed.
use plugin_xmr::XmrPlugin;
use wallet_plugin::WalletPlugin;

fn main() {
    let seed_hex = std::env::var("XMR_SEED").unwrap_or_else(|_| "f9c583e84a5d33d27c459590fc4c0967d361980c11bf91ed625c8fd26a268b967dcf90115dadb2e1efb1b1c5104df4d750eaf7b8e152c5bf691041432f832cba".into());
    let seed = hex::decode(&seed_hex).expect("seed must be hex");
    let plugin = XmrPlugin::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let account = rt
        .block_on(plugin.create_account(&seed, 0, "monero-stagenet"))
        .unwrap();
    println!("Address: {}", account.address);
    println!("Seed hex: {}", seed_hex);
}
