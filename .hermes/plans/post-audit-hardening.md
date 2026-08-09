# Gullbúr Enclave — Post-Audit Hardening Plan

> **Goal:** Address all security & architectural action items identified during the architecture review before starting the UI overhaul.
> **Scope:** Security hardening, key handling cleanup, XMR decoy selection, relay rate limiting, IPC encryption safety, address book encryption, WASM crate configuration.

---

## Task List & Execution Order

### Phase 1: High Priority / Security Hardening

- [ ] **1. Real XMR Decoy Selection**
  - **Goal:** Replace synthetic random curve points in `plugin-xmr/src/plugin.rs` (`build_decoy_ring()`) with real UTXOs fetched via Monero daemon / wallet RPC (`get_output_distribution` / `get_outputs`).
  - **Safety:** If daemon RPC is unreachable, refuse to sign rather than producing zero-privacy synthetic signatures.
  - **Files:** `crates/plugins/xmr/src/plugin.rs`, `crates/plugins/xmr/src/lib.rs`

- [ ] **2. Seed Out of IPC `key_id`**
  - **Goal:** Stop passing hex-encoded raw seed in `KeyHandle.key_id` across IPC.
  - **Approach:**
    - Have `PluginHost` hold the seed (`Arc<RwLock<Option<Zeroizing<Vec<u8>>>>`) directly.
    - Change IPC `vault.sign_transaction` to pass `account_index` (u32) instead of embedding seed bytes in `key_id`.
    - Keep `WalletPlugin` trait signature backward-compatible: `PluginHost` constructs the `KeyHandle` internally from its held seed + `account_index` before calling the plugin.
  - **Files:** `crates/vault-core/src/host.rs`, `crates/vault-core/src/ipc_handlers.rs`, `crates/plugins/btc/src/plugin.rs`, `crates/plugins/ltc/src/plugin.rs`

- [ ] **3. Extension Relay Rate Limiting & Request Deduplication**
  - **Goal:** Prevent dApp spam attacks and reduce IPC traffic.
  - **Features:**
    - Sliding-window rate limiter per origin (e.g., max 30 requests/minute).
    - Max pending approvals limit (semaphore, e.g., max 3 pending).
    - Request deduplication for idempotent reads (`eth_getBalance`, `eth_accounts`, `eth_chainId`) with 2s TTL.
  - **Files:** `crates/extension-relay/src/native_host.rs`, `crates/extension-relay/src/router.rs`, `crates/extension-relay/src/main.rs`

- [ ] **4. Lock Down `IpcServer::with_encryption(false)`**
  - **Goal:** Ensure unencrypted IPC mode cannot be compiled or invoked in release/production builds.
  - **Files:** `crates/ipc-core/src/server.rs` (`#[cfg(test)]` attribute on unencrypted path)

---

### Phase 2: Medium Priority / Infrastructure & Encryption

- [ ] **5. Encrypted Address Book via Vault IPC**
  - **Goal:** Prevent cleartext recipient address book from sitting unencrypted in `localStorage`.
  - **Approach:**
    - Add generic device-key vault IPC helper: `vault.encrypt_data` / `vault.decrypt_data` (AES-256-GCM via device key in Rust).
    - Update `apps/desktop/src/lib/addressBook.ts` to encrypt/decrypt entries transparently through this IPC endpoint.
  - **Files:** `crates/vault-core/src/ipc_handlers.rs`, `apps/desktop/src/lib/addressBook.ts`

- [ ] **6. `crypto-wasm` Crate Configuration Fix**
  - **Goal:** Remove redundant inner `[workspace]` block in `crates/crypto-wasm/Cargo.toml` so workspace tools process it cleanly.
  - **Files:** `crates/crypto-wasm/Cargo.toml`, root `Cargo.toml`

- [ ] **7. Remove Stale Legacy HMAC Functions**
  - **Goal:** Deprecate or remove unused pre-BIP custom HMAC functions (`derive_secp256k1_key`, `derive_k256_key`) to prevent accidental future usage.
  - **Files:** `crates/crypto-core/src/keys.rs`

---

## Verification Strategy

1. `cargo check --workspace`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test --workspace --lib`
4. `cargo test -p cli-integration`
5. `python3 scripts/full-functional-sweep.py` (verify all 33/33 IPC checks pass with new key handling)
6. `bash scripts/full-test-sweep.sh`
7. `cargo tauri android build --target aarch64` (verify APK/AAB build unaffected)
