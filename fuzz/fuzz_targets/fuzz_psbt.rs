#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz PSBT: try to deserialize fuzzer data as a PSBT.
    // Verify no panic — only graceful errors for invalid data.
    let _ = bitcoin::Psbt::deserialize(data);
});