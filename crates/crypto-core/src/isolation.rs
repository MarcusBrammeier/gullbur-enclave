/// Isolation crypto — re-exported from `crypto-isolation` crate.
///
/// This module exists so that existing code importing
/// `crypto_core::isolation::*` continues to work after the
/// AES-GCM logic was split into its own minimal crate
/// (for WASM compilation compatibility).
pub use crypto_isolation::*;