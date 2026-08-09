# Gullbúr Enclave — Architecture Hardening Plan

**Date:** 2026-08-08  
**Status:** Approved by Marcus — Pending Execution  
**Target Repository:** `/root/fosscryptocore-new`  

---

## Plan Overview

This plan addresses the priority security, architectural, and operational findings identified during the architecture review. Once completed, the codebase will be fully prepped for the UI overhaul.

---

## Action Items

### Task 1: Real XMR Decoy Selection via Daemon/Wallet RPC
- **Goal:** Replace `build_decoy_ring()` random curve points with real UTXOs from the Monero blockchain to ensure real ring-signature privacy.
- **Scope:** `crates/plugins/xmr/src/lib.rs`
- **Details:**
  - Call `get_output_distribution` / `get_outputs` via `monero-wallet-rpc` / daemon RPC.
  - Fail closed (refuse to sign) if daemon/RPC is unreachable rather than falling back to random points.

### Task 2: Remove Seed Bytes from `key_id` in IPC
- **Goal:** Stop passing hex-encoded seed bytes inside `KeyHandle.key_id` across IPC.
- **Scope:** `crates/vault-core/src/ipc_handlers.rs`, `crates/vault-core/src/host.rs`
- **Details:**
  - Hold seed in `PluginHost` / `Vault` state.
  - Pass `account_index: u32` across IPC instead of embedding seed bytes in `key_id`.
  - Plugins derive keys internally using `seed + account_index`.

### Task 3: Extension Relay Rate Limiting & Read Deduplication
- **Goal:** Protect `extension-relay` against spam / dApp request floods.
- **Scope:** `crates/extension-relay/src/`
- **Details:**
  - Implement a sliding-window rate limiter per origin.
  - Set a max-pending-approvals limit using a semaphore.
  - Add simple 2-second TTL request deduplication for idempotent read methods (`eth_getBalance`, `eth_accounts`, `eth_chainId`).

### Task 4: Restrict `IpcServer::with_encryption(false)`
- **Goal:** Eliminate unencrypted IPC mode in production builds.
- **Scope:** `crates/ipc-core/src/server.rs`
- **Details:**
  - Gate `with_encryption(false)` behind `#[cfg(test)]`.
  - Enforce encryption in all non-test builds.

### Task 5: Encrypted Address Book via `vault.encrypt_data` IPC
- **Goal:** Encrypt persistent address book entries at rest using the device key.
- **Scope:** `crates/vault-core/src/ipc_handlers.rs`, `apps/desktop/src/lib/addressBook.ts`
- **Details:**
  - Add `vault.encrypt_data` and `vault.decrypt_data` IPC endpoints backed by device key AES-256-GCM.
  - Update `addressBook.ts` to encrypt before saving to `localStorage` and decrypt upon loading.

### Task 6: Clean Up `crypto-wasm` Workspace Manifest
- **Goal:** Resolve workspace nesting ambiguity with `crypto-wasm`.
- **Scope:** `crates/crypto-wasm/Cargo.toml`
- **Details:**
  - Remove standalone `[workspace]` block from `crypto-wasm/Cargo.toml`.
  - Maintain standalone `wasm-pack build` target script without workspace conflicts.

### Task 7: Clean Up Legacy HMAC Functions
- **Goal:** Mark unused HMAC key derivation functions as deprecated or remove dead code.
- **Scope:** `crates/crypto-core/src/keys.rs`
- **Details:**
  - Add `#[deprecated]` to legacy `derive_k256_key` / `derive_secp256k1_key` HMAC routines to prevent accidental future usage over standard BIP paths.

---

## Execution Command

To execute this plan in a future session, run or tell Hermes:
```text
Execute the Architecture Hardening Plan from PLANS/2026-08-08-architecture-hardening.md
```
