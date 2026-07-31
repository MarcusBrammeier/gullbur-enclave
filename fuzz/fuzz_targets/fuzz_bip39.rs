#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz BIP-39: try to parse fuzzer data as a mnemonic phrase.
    // Verify no panic — only graceful errors for invalid input.
    let s = String::from_utf8_lossy(data);

    // Try parsing as a mnemonic
    if let Ok(mnemonic) = bip39::Mnemonic::parse_normalized(&s) {
        let _seed = mnemonic.to_seed("");
        let _words: Vec<&str> = mnemonic.words().collect();
    }
});
