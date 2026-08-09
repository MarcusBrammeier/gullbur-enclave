# Plan: Security & Architecture Refinements (Pre-UI Overhaul)

**Target Repository:** `/root/fosscryptocore-new`  
**Date:** 2026-08-08  
**Goal:** Implement 7 key security & architectural hardening tasks prior to initiating the full UI overhaul.

---

## Task Breakdown & Priority Matrix

### 1. [P0] Real Monero Decoy Selection (Mainnet Blocker)
- **Problem:** `build_decoy_ring()` in `crates/plugins/xmr/src/plugin.rs` uses synthetic random curve points for CLSAG rings, providing zero privacy on mainnet.
- **Solution:** 
  - Add daemon / `monero-wallet-rpc` calls (`get_output_distribution` / `get_outputs`) via `XmrPlugin::build_client()` to select real UTXO public keys from the blockchain.
  - Require daemon reachability for signing; fail closed if real decoys cannot be fetched.

### 2. [P1] Remove Raw Seed from IPC `key_id` Protocol
- **Problem:** `ipc_handlers.rs` formats `key_id` as `"{seed_hex}@{account_index}"`, passing master seed material through the IPC message layer.
- **Solution:**
  - Update `PluginHost` to hold a zeroized reference/Arc to the vault seed.
  - Update `sign_transaction` on `PluginHost` to accept `account_index: u32` rather than parsing seed hex out of `KeyHandle.key_id`.
  - Update IPC handlers to pass only the account index across the wire.

### 3. [P1] Extension Relay Rate Limiting & Read Request Deduplication
- **Problem:** `crates/extension-relay` forwards stdio messages to WebSocket with no rate limiting or request throttling.
- **Solution:**
  - Implement a sliding-window rate limiter (e.g. max 30 requests / 10 sec per origin).
  - Add a per-origin maximum pending approval limit.
  - Add short-lived (2s TTL) request deduplication for idempotent read calls (`eth_getBalance`, `eth_accounts`, `eth_chainId`).

### 4. [P1] Restrict `IpcServer::with_encryption(..., false)` to Test Builds
- **Problem:** `with_encryption` allows unencrypted IPC via a boolean parameter in production code paths.
- **Solution:**
  - Wrap `with_encryption(..., false)` / unencrypted constructor options behind `#[cfg(test)]`.
  - Ensure production builds enforce AES-256-GCM encryption at compile time.

### 5. [P2] Encrypt Address Book via Device Key IPC
- **Problem:** Address book in `apps/desktop/src/lib/addressBook.ts` is stored in unencrypted `localStorage`.
- **Solution:**
  - Add `vault.encrypt_data` and `vault.decrypt_data` JSON-RPC IPC endpoints backed by `keystore-core`'s device key (`TieredDeviceKeyProvider`).
  - Update `addressBook.ts` to encrypt entries before storing to `localStorage` and decrypt on load.

### 6. [P2] Fix `crypto-wasm` Cargo Workspace Block
- **Problem:** `crates/crypto-wasm/Cargo.toml` contains an isolated `[workspace]` block that prevents `cargo check --workspace` from recognizing it cleanly.
- **Solution:**
  - Remove the standalone `[workspace]` block from `crypto-wasm/Cargo.toml`.
  - Maintain the `wasm-pack build` build script target so it builds cleanly for `wasm32-unknown-unknown`.

### 7. [P3] Clean Up Legacy Custom HMAC Key Derivation
- **Problem:** `derive_secp256k1_key` and `derive_k256_key` in `crypto-core` were early HMAC-based derivation prototypes and are now dead code.
- **Solution:**
  - Deprecate or remove unused HMAC derivation functions in `crates/crypto-core/src/keys.rs`.
  - Verify all key derivation routes strictly use standard BIP-39/BIP-44/BIP-84 paths.

---

## Execution Command

To execute this plan in a future session, speak the following trigger phrase:

```text
Execute the pre-UI security hardening plan at /root/fosscryptocore-new/PLANS/2026-08-08-security-architecture-refinements.md
```
