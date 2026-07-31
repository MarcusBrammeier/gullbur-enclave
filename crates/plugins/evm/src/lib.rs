// ── plugin-evm: Ethereum & EVM-compatible chains wallet plugin ──
//
// Implements the `WalletPlugin` trait for Ethereum and popular L2s.
// BIP44 derivation, EIP-1559 transaction parsing via alloy,
// EIP-55 address validation, and RPC endpoint switching.

pub mod plugin;

pub use plugin::EvmPlugin;
pub use plugin::rpc_endpoint;
