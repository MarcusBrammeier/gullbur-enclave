#![no_main]
use libfuzzer_sys::fuzz_target;
use wallet_plugin::traits::WalletPlugin;

fuzz_target!(|data: &[u8]| {
    // Fuzz XMR address validation (parses base58, checksum, prefix)
    let plugin = plugin_xmr::XmrPlugin::new();
    let addr = String::from_utf8_lossy(data);
    let _ = futures::executor::block_on(plugin.validate_address(&addr, "monero"));
});
