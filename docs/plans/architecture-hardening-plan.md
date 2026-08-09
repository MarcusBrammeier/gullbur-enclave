# Gullbúr Enclave — Architecture Hardening & Security Plan

> **Created:** 2026-08-08
> **Status:** Pending execution
> **Context:** Pre-UI overhaul security & architecture hardening phase

---

## Plan Overview

This plan covers 7 prioritized action items identified during the codebase architecture and security review. Executing these items completes the backend security hardening before moving into the UI overhaul.

---

## Action Items

### 1. Real XMR Decoy Selection via Daemon/Wallet-RPC (P0 — Mainnet Blocker)
- **Problem:** `plugin-xmr/src/plugin.rs` currently generates random curve points for CLSAG decoys, providing zero ring-signature privacy.
- **Fix:** 
  - Connect decoy selection to real Monero blockchain outputs via `monero-wallet-rpc` / daemon RPC (`get_output_distribution` / `get_outputs`).
  - Refuse to sign if daemon/wallet-rpc is unreachable rather than falling back to synthetic points.
- **Files:** `crates/plugins/xmr/src/plugin.rs`, `crates/vault-core/src/xmr_wallet_rpc.rs`

### 2. Remove Raw Seed from `key_id` in IPC Layer (P1)
- **Problem:** `ipc_handlers.rs` currently formats `key_id` as `hex(seed)@index`, exposing raw seed bytes in the IPC message layer.
- **Fix:**
  - Store `seed: Arc<RwLock<Option<Zeroizing<Vec<u8>>>>>` in `PluginHost` / pass `account_index: u32` through IPC instead of raw seed in `key_id`.
  - Plugins receive `account_index` and derive keys internally without the IPC payload carrying raw seed hex.
- **Files:** `crates/vault-core/src/ipc_handlers.rs`, `crates/vault-core/src/host.rs`

### 3. Extension Relay Rate Limiting & Request Deduplication (P1)
- **Problem:** `gullbur-relay` forwards stdio messages to WebSocket with zero rate limiting or request deduplication.
- **Fix:**
  - Add sliding-window rate limiter per origin in `crates/extension-relay/src/native_host.rs` / `main.rs`.
  - Add per-origin maximum pending approvals limit.
  - Implement 2-second TTL request deduplication for idempotent read methods (`eth_getBalance`, `eth_accounts`, `eth_chainId`).
- **Files:** `crates/extension-relay/src/native_host.rs`, `crates/extension-relay/src/main.rs`, `crates/extension-relay/src/router.rs`

### 4. Gate IPC Unencrypted Mode Behind `#[cfg(test)]` (P1)
- **Problem:** `IpcServer::with_encryption(bind_port, false)` is callable in release builds.
- **Fix:**
  - Restrict `with_encryption(..., false)` / unencrypted constructor mode behind `#[cfg(test)]` attribute so production builds always enforce AES-256-GCM encryption.
- **Files:** `crates/ipc-core/src/server.rs`

### 5. Encrypted Address Book via Device Key IPC (P2)
- **Problem:** Address book in `apps/desktop/src/lib/addressBook.ts` stores plain recipient addresses in frontend `localStorage`.
- **Fix:**
  - Expose `vault.encrypt_data` and `vault.decrypt_data` IPC methods backed by `keystore-core` device key.
  - Update `addressBook.ts` to store an AES-256-GCM encrypted blob in `localStorage` decrypted via vault IPC on load.
- **Files:** `crates/vault-core/src/ipc_handlers.rs`, `apps/desktop/src/lib/addressBook.ts`

### 6. Fix `crypto-wasm` Workspace Declaration (P2)
- **Problem:** `crates/crypto-wasm/Cargo.toml` declares its own `[workspace]` block, disconnecting it from root `cargo check --workspace`.
- **Fix:**
  - Remove `[workspace]` from `crates/crypto-wasm/Cargo.toml`.
  - Add `"crates/crypto-wasm"` to root `Cargo.toml` workspace members list.
  - Verify standalone `wasm-pack build` and root `cargo check --workspace` both pass.
- **Files:** `crates/crypto-wasm/Cargo.toml`, `/root/fosscryptocore-new/Cargo.toml`

### 7. Clean Up Unused Legacy HMAC Derivation Code (P3)
- **Problem:** `derive_secp256k1_key` and `derive_k256_key` in `crypto-core` use custom HMAC labels rather than standard BIP-44/84 paths.
- **Fix:**
  - Mark functions `#[deprecated]` or remove dead custom HMAC derivation routines from `crypto-core`.
  - Keep `derive_ed25519_key` only where needed for ephemeral session keys.
- **Files:** `crates/crypto-core/src/keys.rs`

---

## Verification Commands

After completing the items, run:
```bash
cargo check --workspace
cargo test --workspace --lib
cargo clippy --workspace -- -D warnings
cargo deny check
scripts/full-functional-sweep.py
```
