# Gullbúr Enclave — Post-Architecture Security & Robustness Plan

**Status:** Staged for execution  
**Target Repo:** `/root/fosscryptocore-new`  
**Goal:** Address 6 prioritized security, privacy, and architectural hardening items discovered during the architecture review before proceeding to the UI overhaul.

---

## Task List & Execution Details

### Task 1: Implement Real Monero (XMR) Decoy Selection (P0 — Mainnet Privacy Blocker)
- **Problem:** `plugin-xmr/src/lib.rs` (`build_decoy_ring()`) generates random curve points instead of real blockchain UTXOs, yielding zero ring-signature privacy.
- **Solution:** 
  1. Add a daemon/wallet RPC helper in `XmrPlugin` using `reqwest` to query `get_outputs` / `get_output_distribution` from `monero-wallet-rpc` / daemon.
  2. Populate the decoy ring with real public outputs from the Monero blockchain instead of random points.
  3. If RPC is unavailable or returns an error, fail closed — refuse to sign rather than producing a linkable transaction.

### Task 2: Remove Raw Seed Exposure from `key_id` IPC Flow (P1)
- **Problem:** `ipc_handlers.rs` encodes raw seed hex into `KeyHandle.key_id` (`{seed_hex}@{account_index}`), circulating the master secret through the IPC protocol.
- **Solution:**
  1. Have `PluginHost` hold an `Arc<RwLock<Option<Zeroizing<Vec<u8>>>>>` reference to the active seed.
  2. Change `PluginHost::sign_transaction` to take `account_index: u32` rather than parsing seed hex out of `KeyHandle.key_id`.
  3. In `ipc_handlers.rs`, pass only the `account_index` (and non-secret key identifier) across the IPC boundary.

### Task 3: Extension-Relay Rate Limiter & Read Deduplication (P1)
- **Problem:** `extension-relay` has no rate limiting or request throttling, allowing dApps to spam signing requests.
- **Solution:**
  1. Implement a sliding-window rate limiter in `crates/extension-relay/src/` (e.g., max 30 requests/minute per origin).
  2. Add a `tokio::sync::Semaphore` to enforce a max pending approval limit (e.g., max 3 concurrent approval prompts).
  3. Implement request deduplication for idempotent read methods (`eth_getBalance`, `eth_chainId`, `eth_accounts`) with a short TTL (1-2s).

### Task 4: Restrict `IpcServer::with_encryption(..., false)` to Test Builds (P1)
- **Problem:** `IpcServer::with_encryption(bind_port, false)` allows unencrypted IPC in production code.
- **Solution:**
  1. Gate `with_encryption(..., false)` behind `#[cfg(test)]`.
  2. In release/production builds, enforce that `IpcServer::new()` ALWAYS enables AES-256-GCM encryption.

### Task 5: Encrypt Address Book via Device Key IPC (P2)
- **Problem:** `addressBook.ts` saves recipient addresses as cleartext in browser `localStorage`.
- **Solution:**
  1. Add two new IPC methods in `vault-core`: `vault.encrypt_data` and `vault.decrypt_data` using the existing device key (`GBAF` / `keystore-core`).
  2. Update `apps/desktop/src/lib/addressBook.ts` to pass the address book payload through `vault.encrypt_data` before writing to `localStorage`, and decrypt on load.

### Task 6: Clean Up `crypto-wasm` Workspace Declaration & Remove Dead HMAC Code (P2 / P3)
- **Problem:** `crypto-wasm/Cargo.toml` has its own redundant `[workspace]` tag, and legacy custom HMAC key derivation functions (`derive_k256_key` / `derive_secp256k1_key`) remain as dead code.
- **Solution:**
  1. Remove the standalone `[workspace]` block from `crates/crypto-wasm/Cargo.toml`.
  2. Mark dead HMAC key derivation functions in `crypto-core/src/keys.rs` with `#[deprecated]` or prune them safely.
  3. Run full verification sweep (`cargo check --workspace`, `cargo test --workspace`, full functional sweep script).

---

## Verification Plan

After all tasks are complete, run:
```bash
# 1. Full compilation and lint checks
cargo check --workspace
cargo clippy --workspace -- -D warnings

# 2. Rust unit & integration test suite
cargo test --workspace --lib
cargo test -p cli-integration

# 3. Full functional sweep via WebSocket
python3 scripts/full-functional-sweep.py

# 4. Svelte frontend tests
npm --prefix apps/desktop run test
```
