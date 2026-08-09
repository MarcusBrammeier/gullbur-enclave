# Implementation Plan: Pre-UI Hardening & Monero Decoys

**Target repo:** `/root/fosscryptocore-new`  
**Status:** Staged for execution  

---

## Task Summary

Address security & architecture action items before beginning the UI overhaul:
1. **P0 (Mainnet Blocker):** Real Monero decoy selection via `monero-wallet-rpc` / daemon RPC (replace random curve points).
2. **P1:** Remove seed exposure from IPC `key_id` — have `PluginHost` hold the seed in memory and accept an account index.
3. **P1:** Add sliding-window rate limiter, per-origin request cap, and idempotent read deduplication in `extension-relay`.
4. **P1:** Gate `IpcServer::with_encryption(false)` behind `#[cfg(test)]`.
5. **P2:** Address book encryption at rest via new `vault.encrypt_data` / `vault.decrypt_data` IPC methods.
6. **P2:** Fix `crypto-wasm` workspace isolation / build setup.
7. **P3:** Clean up / deprecate dead legacy HMAC key derivation functions (`derive_k256_key`, `derive_secp256k1_key`).

---

## Detailed Step Breakdown

### Step 1: Real Monero Decoy Selection (`crates/plugins/xmr`)
- Add a daemon / `monero-wallet-rpc` call for output distribution / real decoy UTXOs (`get_output_distribution` / `get_outputs`).
- Replace `build_decoy_ring()` in `plugin-xmr/src/plugin.rs` (random curve points) with actual UTXOs fetched from the Monero network.
- Fail closed: if decoys cannot be fetched, return an error rather than falling back to fake points.

### Step 2: Remove Seed from IPC `key_id` (`crates/vault-core` & `crates/ipc-core`)
- Modify `PluginHost` and IPC handlers (`ipc_handlers.rs`) so `sign_transaction` routes via `account_index: u32` and reads the seed internally from `Vault` state.
- Keep the `WalletPlugin` trait clean without exposing raw hex seeds across IPC messages.

### Step 3: Relay Rate Limiting & Dedup (`crates/extension-relay`)
- Add sliding-window rate limiting per origin in `gullbur-relay` native host.
- Add maximum pending approval limit to prevent prompt spamming.
- Implement 2-second TTL cache for idempotent read requests (`eth_getBalance`, `eth_accounts`, `eth_chainId`).

### Step 4: Restrict Unencrypted IPC Mode (`crates/ipc-core`)
- Restrict `IpcServer::with_encryption(bind_port, false)` to `#[cfg(test)]` so production builds always enforce AES-256-GCM encryption.

### Step 5: Encrypted Address Book IPC (`apps/desktop` & `crates/vault-core`)
- Add `vault.encrypt_data` and `vault.decrypt_data` IPC handlers in `vault-core` using the existing device key (`GBAF` AES-256-GCM).
- Update `apps/desktop/src/lib/addressBook.ts` to store encrypted payloads in `localStorage`.

### Step 6: Clean Up `crypto-wasm` Workspace Config (`crates/crypto-wasm`)
- Remove duplicate `[workspace]` block in `crates/crypto-wasm/Cargo.toml`.
- Configure proper target handling so `cargo check --workspace` and `wasm-pack build` work seamlessly.

### Step 7: Clean Up Legacy HMAC Functions (`crates/crypto-core`)
- Mark unused legacy `derive_k256_key` / `derive_secp256k1_key` as `#[deprecated]` or remove dead code.

---

## Verification
- Run `cargo check --workspace`
- Run `cargo clippy --workspace -- -D warnings`
- Run `cargo test --workspace`
- Execute `scripts/full-functional-sweep.py`
