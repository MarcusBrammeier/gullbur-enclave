// ── plugin-btc: Bitcoin wallet plugin ──────────────────────────────
//
// Implements the `WalletPlugin` trait for Bitcoin.
// BIP84 (Native SegWit / P2WPKH) derivation, PSBT parsing,
// and address validation. Network-dependent stubs for RPC ops.

pub mod plugin;

pub use plugin::BtcPlugin;
