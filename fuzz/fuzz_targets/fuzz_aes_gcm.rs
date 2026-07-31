#![no_main]
#![allow(deprecated)]
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let key_bytes = [0xabu8; 32];
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&[0x42u8; 12]);

    let plaintext = if data.len() > 4096 {
        &data[..4096]
    } else {
        data
    };
    if let Ok(ciphertext) = cipher.encrypt(nonce, plaintext) {
        let _ = cipher.decrypt(nonce, ciphertext.as_ref());

        let mut tampered = ciphertext.clone();
        if let Some(last) = tampered.last_mut() {
            *last ^= 0xff;
        }
        let _ = cipher.decrypt(nonce, tampered.as_ref());
    }
});
