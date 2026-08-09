# Gullbúr Enclave — Security Hardening & Architecture Fixes Plan

**Date:** 2026-08-08  
**Status:** Staged for execution  
**Target Repo:** `/root/fosscryptocore-new`  

---

## Overview

A 7-item architecture & security hardening plan to be executed before resuming the frontend UI overhaul. Addresses Monero decoy privacy, seed exposure over IPC, extension relay rate-limiting, IPC encryption safety, address book at-rest security, WASM workspace configuration, and dead code cleanup.

---

## Tasks

### 1. Real XMR Decoy Selection via Monero Daemon / Wallet RPC (P0 — Mainnet Blocker)
- **Goal:** Replace synthetic random curve point generation in `plugin-xmr/src/plugin.rs` (`build_decoy_ring()`) with real UTXO decoy selection.
- **Implementation:**
  - Wire `get_output_distribution` or `get_outputs` JSON-RPC call through the HTTP client in `XmrPlugin`.
  - Fetch real output keys from the Monero network (via connected daemon / wallet-rpc).
  - Add fail-closed behavior: if RPC is unreachable, fail transaction creation instead of falling back to random points.
- **Verification:** Unit & integration tests for XMR signing with live or mocked daemon responses; verify non-synthetic ring points.

### 2. Remove Seed Exposure in `KeyHandle` / IPC (P1)
- **Goal:** Prevent raw seed bytes from circulating in string form inside `key_id` across IPC.
- **Implementation:**
  - Store `seed: Arc<RwLock<Option<Zeroizing<Vec<u8>>>>>` in `PluginHost`.
  - Update `PluginHost::sign_transaction()` to accept `account_index: u32` rather than expecting `seed@index` formatted inside `key_id`.
  - Update `ipc_handlers.rs` `vault.sign_transaction` to pass `account_index` and look up the seed in Rust memory without encoding it into `KeyHandle.key_id`.
- **Verification:** `cargo test --workspace` and `full-functional-sweep.py` pass.

### 3. Extension Relay Rate Limiting & Read Deduplication (P1)
- **Goal:** Protect `extension-relay` from rapid request spam and duplicate read requests.
- **Implementation:**
  - Add sliding-window rate limiter per origin (`Instant` timestamp tracking + window counter).
  - Add max concurrent pending approval limit using a `tokio::sync::Semaphore`.
  - Add 2-second in-memory response cache for idempotent reads (`eth_getBalance`, `eth_accounts`, `eth_chainId`).
- **Verification:** Extension relay unit tests & concurrency stress test.

### 4. Gate `IpcServer::with_encryption(..., false)` Behind `#[cfg(test)]` (P1)
- **Goal:** Guarantee unencrypted IPC cannot be enabled in non-test builds.
- **Implementation:**
  - In `crates/ipc-core/src/server.rs`, wrap `with_encryption(bind_port, false)` or the `encrypt: false` branch in `#[cfg(test)]`.
  - Ensure production `IpcServer::new()` unconditionally enforces `encrypt_responses = true`.
- **Verification:** `cargo check --workspace` compiles cleanly.

### 5. Encrypted Address Book via `vault.encrypt_data` IPC (P2)
- **Goal:** Encrypt user recipient addresses in `localStorage` without exposing device key to JavaScript.
- **Implementation:**
  - Expose `vault.encrypt_data` and `vault.decrypt_data` JSON-RPC endpoints in `vault-core` using AES-256-GCM + device key.
  - Update `apps/desktop/src/lib/addressBook.ts` to pass JSON payloads through `vault.encrypt_data` before writing to `localStorage` and `vault.decrypt_data` on load.
  - Maintain backward compatibility for reading legacy unencrypted entries.
- **Verification:** Svelte address book component tests & round-trip persistence tests.

### 6. Fix `crypto-wasm` Workspace Configuration (P2)
- **Goal:** Clean up standalone `[workspace]` block in `crates/crypto-wasm/Cargo.toml`.
- **Implementation:**
  - Remove standalone `[workspace]` declaration from `crates/crypto-wasm/Cargo.toml`.
  - Add `"crates/crypto-wasm"` to top-level `Cargo.toml` workspace members or handle explicitly in root workspace.
  - Ensure `wasm-pack build` and `cargo check --workspace` work harmoniously in CI.
- **Verification:** Both `cargo check --workspace` and `wasm-pack build` succeed cleanly.

### 7. Clean Up Dead Custom HMAC Derivation Code (P3 — Cleanup)
- **Goal:** Mark or remove pre-BIP custom HMAC key derivation functions (`derive_secp256k1_key`, `derive_k256_key`) in `crypto-core`.
- **Implementation:**
  - Deprecate or remove unused functions in `crates/crypto-core/src/keys.rs`.
  - Confirm all active key derivations flow through BIP-39/BIP-32/BIP-44/BIP-84 standard paths.
- **Verification:** `cargo check --workspace` and unit tests pass with zero warnings.

---

## Recommended Execution Order

1. **Item 4:** Gate unencrypted IPC (5 mins)
2. **Item 6:** `crypto-wasm` workspace fix (5 mins)
3. **Item 7:** HMAC dead code cleanup (10 mins)
4. **Item 2:** Seed out of `key_id` in IPC (2-3 hrs)
5. **Item 3:** Relay rate limiter & dedup (1 hr)
6. **Item 5:** Encrypted address book IPC (1 hr)
7. **Item 1:** Real XMR decoy selection (3-4 hrs)
