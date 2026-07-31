//! crypto-core — shared cryptographic primitives for Gullbúr Enclave.
//! Crypto backend: RustCrypto (sha2, sha3, ripemd, hmac, hkdf, aes-gcm, k256,
//! secp256k1 via bitcoin crate re-export, ed25519-dalek).
//! No direct C FFI dependencies — all pure Rust.
//! See STATE.md D4 for backend migration notes.

pub mod error;
pub mod hash;
pub mod isolation;
pub mod keys;
pub mod signer;
pub mod types;

pub use error::CryptoError;
pub use hash::{hash160, keccak256, ripemd160, sha256};
pub use keys::{
    derive_bip44_eth_key, derive_bip44_xmr_entropy, generate_mnemonic, generate_seed,
    mnemonic_from_string, mnemonic_to_seed, mnemonic_to_string,
};
pub use signer::{sign_ecdsa_k256, sign_ecdsa_secp256k1, sign_schnorr, verify_ecdsa_secp256k1};
pub use types::*;
