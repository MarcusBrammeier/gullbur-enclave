# Gullbúr Enclave — Security & Architecture Refinements Plan

**Status:** Staged for execution  
**Date:** 2026-08-08  
**Target Workspace:** `/root/fosscryptocore-new`  

---

## Overview

This plan covers 6 prioritized security and architecture refinements discovered during the 2026-08-08 architecture review of Gullbúr Enclave, followed by the transition back to the UI overhaul.

---

## Task Breakdown

### Phase 1: High-Priority Backend Security Hardening

#### Task 1: Real XMR Decoy Selection via `monero-wallet-rpc` / Daemon
- **Goal:** Replace synthetic random curve point generation in `plugin-xmr/src/lib.rs` (`build_decoy_ring()`) with real UTXO decoy selection.
- **Implementation:**
  - Connect to Monero daemon / `wallet-rpc` using existing `reqwest` HTTP client.
  - Implement `get_output_distribution` / `get_outputs` daemon RPC queries to fetch actual on-chain output keys and commitments for ring signatures (ring size 11).
  - Add safety guard: if daemon/wallet-rpc is unreachable, fail transaction building explicitly rather than falling back to unprivate synthetic points.
- **Verification:** Run `cargo test -p plugin-xmr` + verify CLSAG ring construction against mock/live stagenet data.

#### Task 2: Remove Seed from IPC `key_id`
- **Goal:** Prevent raw 64-byte seed hex strings from circulating through IPC and `KeyHandle` structs.
- **Implementation:**
  - Store `seed: Arc<RwLock<Option<Zeroizing<Vec<u8>>>>>` in `PluginHost` / pass `account_index: u32` down the vault signing pipeline.
  - Update `ipc_handlers.rs` `vault.sign_transaction` handler so `key_id` contains an opaque handle/fingerprint or account index instead of raw seed bytes.
  - Keep `WalletPlugin` trait backward-compatible.
- **Verification:** `cargo test -p vault-core --test e2e_websocket` + full-functional-sweep (`scripts/full-functional-sweep.py`).

#### Task 3: Extension Relay Hardening (Rate Limiting & Read Dedup)
- **Goal:** Protect extension IPC channel against spam/DoS from dApps.
- **Implementation:**
  - Add sliding-window rate limiter per origin in `extension-relay/src/native_host.rs` / `main.rs`.
  - Enforce `max_pending_approvals` cap (e.g. max 5 concurrent approval requests per origin).
  - Add request deduplication for idempotent read methods (`eth_getBalance`, `eth_accounts`, `eth_chainId`) with a brief TTL (~2s).
- **Verification:** Run extension relay unit/integration tests in `crates/extension-relay`.

#### Task 4: Gate `IpcServer::with_encryption(false)`
- **Goal:** Ensure unencrypted IPC mode cannot be compiled into production builds.
- **Implementation:**
  - Guard `with_encryption(bind_port, false)` or unencrypted constructor behind `#[cfg(test)]` in `crates/ipc-core/src/server.rs`.
- **Verification:** `cargo check --workspace` + confirm non-test builds only expose encrypted server creation.

---

### Phase 2: System Cleanups & Utility Enhancements

#### Task 5: Encrypted Address Book via Vault Device-Key IPC
- **Goal:** Protect the user's saved recipient contacts in localStorage from plaintext disk leaks.
- **Implementation:**
  - Expose `vault.encrypt_data` and `vault.decrypt_data` IPC endpoints in `vault-core` bound to the device key (`GBAF` / `GBAE` envelope).
  - Update `apps/desktop/src/lib/addressBook.ts` to encrypt address book payloads on write and decrypt on load.
- **Verification:** Svelte address book unit tests (`npm run test` in `apps/desktop`).

#### Task 6: `crypto-wasm` Workspace Cargo.toml Fix & Dead Code Clean
- **Goal:** Normalize workspace layout and remove dead pre-BIP HMAC derivation functions.
- **Implementation:**
  - Remove inner `[workspace]` block from `crates/crypto-wasm/Cargo.toml`.
  - Mark legacy `derive_secp256k1_key` / `derive_k256_key` custom HMAC functions as `#[deprecated]` or prune if unused.
- **Verification:** `cargo check --workspace` + `wasm-pack build crates/crypto-wasm`.

---

## Verification & Suite Run

1. `cargo check --workspace`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test --workspace --lib`
4. `python3 scripts/full-functional-sweep.py`

---

## Next Step After Plan Completion
Proceed directly to the **UI Overhaul** (accent-theme polish, tactical button press, Svelte 5 component layout updates).
