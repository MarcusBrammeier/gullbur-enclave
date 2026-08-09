# Action Plan: Security & Architecture Hardening Phase

**Target:** `gullbur-enclave` (formerly `fosscryptocore-new`)
**Goal:** Complete the 6 prioritized architectural & security hardening tasks before proceeding to the UI overhaul.

---

## Tasks Overview

### Task 1: Real XMR Decoy Selection via `monero-wallet-rpc` (P0)
- **Problem:** `build_decoy_ring()` in `crates/plugins/xmr/src/plugin.rs` currently uses random curve points for CLSAG decoys, providing zero ring-signature privacy on mainnet.
- **Solution:** 
  - Add a daemon RPC call (`get_output_distribution` / `get_outputs`) via the existing `reqwest` client in `XmrPlugin`.
  - Fetch real output public keys / UTXOs from the Monero blockchain / daemon.
  - Fail closed (refuse to sign) if daemon/wallet-rpc is unreachable rather than substituting synthetic points.

### Task 2: Decouple Seed from `key_id` in IPC Layer (P1)
- **Problem:** `ipc_handlers.rs` currently hex-encodes the raw 64-byte BIP-39 seed and passes it inside `KeyHandle.key_id` (e.g. `<seed_hex>@<index>`).
- **Solution:**
  - Store the `seed` in `PluginHost` / `Vault` memory.
  - Accept `account_index: u32` in `sign_transaction` IPC calls instead of embedding raw seed bytes in the `key_id` payload.
  - `PluginHost` constructs the `KeyHandle` internally from the held seed and account index before handing off to the target plugin.

### Task 3: Extension-Relay Rate Limiting & Request Deduplication (P1)
- **Problem:** `crates/extension-relay` has no rate limiter or request queue controls, making it vulnerable to request spam from dApps.
- **Solution:**
  - Implement a sliding-window rate limiter per origin (`std::time::Instant` + request count).
  - Add a cap on concurrent pending approval requests.
  - Add 2-second request deduplication for idempotent read methods (`eth_getBalance`, `eth_accounts`, `eth_chainId`).

### Task 4: Restrict Plaintext IPC to `#[cfg(test)]` (P1)
- **Problem:** `IpcServer::with_encryption(port, false)` allows unencrypted IPC in production builds if accidentally invoked.
- **Solution:**
  - Enforce `encrypt = true` in production `IpcServer::new()`.
  - Move the unencrypted constructor behind `#[cfg(test)]`.

### Task 5: Encrypted Address Book via `vault.encrypt_data` IPC (P2)
- **Problem:** Address book (`addressBook.ts`) is currently stored in cleartext `localStorage`.
- **Solution:**
  - Add `vault.encrypt_data` and `vault.decrypt_data` IPC handlers in `vault-core` using `keystore-core` device-key encryption.
  - Update `addressBook.ts` to encrypt entries before storing to `localStorage` and decrypt on load.

### Task 6: `crypto-wasm` Workspace Cleanup (P2)
- **Problem:** `crates/crypto-wasm/Cargo.toml` contains an isolated `[workspace]` block, disconnecting it from root workspace tools.
- **Solution:**
  - Remove the standalone `[workspace]` table from `crates/crypto-wasm/Cargo.toml`.
  - Ensure CI / build scripts retain `wasm-pack build crates/crypto-wasm --target web`.

### Task 7 (Cleanup): Deprecate Legacy Custom HMAC Key Derivation (P3)
- **Problem:** Legacy `derive_secp256k1_key` / `derive_k256_key` custom HMAC routines are obsolete (standard BIP-39/44/84 paths are used for all user wallets).
- **Solution:**
  - Annotate unused HMAC derivation routines in `crypto-core` with `#[deprecated]`.

---

## Verification & Gates
1. Run full test suite: `cargo test --workspace`
2. Run functional IPC sweep: `python3 scripts/full-functional-sweep.py`
3. Run clippy: `cargo clippy --workspace -- -D warnings`
4. Confirm APK/AAB build: `cargo tauri android build --target aarch64`
