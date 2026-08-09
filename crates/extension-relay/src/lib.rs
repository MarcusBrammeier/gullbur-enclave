//! EIP-6963 browser extension relay.
//!
//! Bridges browser wallet extensions to vault-core via native messaging:
//! - NativeMessagingHost: stdio JSON message loop with origin validation
//! - Eip6963Provider: announces Gullbúr Enclave as an injected provider
//! - MethodRouter: translates eth_* JSON-RPC to vault.* IPC methods
//! - PermissionManager: origin-based account access gating

pub mod native_host;
pub mod permissions;
pub mod provider;
pub mod rate_limiter;
pub mod router;
