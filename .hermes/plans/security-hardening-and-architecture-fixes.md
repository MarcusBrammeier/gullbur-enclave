# Security Hardening & Architecture Fixes Plan

> **Goal:** Execute the 6 prioritized architecture and security improvements for Gullbúr Enclave before returning to the UI overhaul.

---

## Task Breakdown

### Phase 1: High Priority / Security Critical

#### 1. Real Monero (XMR) Decoy Selection via Daemon / wallet-rpc
- **File:** `crates/plugins/xmr/src/plugin.rs` (or `lib.rs`)
- **Action:** Replace synthetic random curve point generation in `build_decoy_ring()` with real UTXOs fetched via daemon RPC (`get_output_distribution` / `get_outputs`) or `monero-wallet-rpc`.
- **Safety:** Refuse to sign if real decoy selection is unavailable (do NOT default to synthetic decoys on mainnet).

#### 2. Remove Raw Seed from `key_id` in IPC
- **Files:** `crates/vault-core/src/host.rs`, `crates/vault-core/src/ipc_handlers.rs`, `crates/plugins/btc/src/plugin.rs`, `crates/plugins/ltc/src/plugin.rs`
- **Action:** Update `PluginHost` to hold the `seed` reference directly. Route transaction signing by `account_index` (u32) or internal account lookup rather than serializing raw seed hex bytes into `key_id` across the IPC protocol.
- **Safety:** Ensure `WalletPlugin` trait stays clean and secret handling is confined to Rust-side host logic.

#### 3. Sliding-Window Rate Limiter & Request Dedup in Extension Relay
- **Files:** `crates/extension-relay/src/main.rs`, `crates/extension-relay/src/router.rs` (or dedicated module)
- **Action:**
  - Add rate limiting (sliding window per origin / global token bucket) for native messaging incoming calls.
  - Implement a max-pending-approvals limit (e.g., max 5 concurrent approval requests) to prevent dApp spam.
  - Add simple deduplication for idempotent read methods (`eth_getBalance`, `eth_accounts`, `eth_chainId`) with a 2-second TTL.

#### 4. Gate Unencrypted IPC Server Behind `#[cfg(test)]`
- **File:** `crates/ipc-core/src/server.rs`
- **Action:** Enforce that production builds (`#[cfg(not(test))]`) always run with encryption enabled (`with_encryption(port, true)`). Restrict unencrypted instantiation to test contexts only.

---

## Phase 2: Enhancements & Cleanup

#### 5. Encrypted Address Book via Vault IPC
- **Files:** `crates/vault-core/src/ipc_handlers.rs`, `apps/desktop/src/lib/addressBook.ts`
- **Action:**
  - Add lightweight IPC endpoints `vault.encrypt_data` and `vault.decrypt_data` in Rust backed by the device key (AES-256-GCM).
  - Update `addressBook.ts` to pass address book JSON through Rust encryption before saving to `localStorage`, keeping device secrets in Rust.

#### 6. Clean Up `crypto-wasm` Workspace & Dead Code
- **Files:** `crates/crypto-wasm/Cargo.toml`, `Cargo.toml`, `crates/crypto-core/src/keys.rs`
- **Action:**
  - Clean up `crypto-wasm/Cargo.toml` workspace block declaration so `cargo check --workspace` operates smoothly.
  - Mark obsolete custom HMAC derivation functions (`derive_secp256k1_key`, `derive_k256_key`) as `#[deprecated]` or remove if completely unreferenced.

---

## Verification & Validation
- Run full test suite: `cargo test --workspace --lib`
- Run CLI integration tests: `cargo test -p cli-integration`
- Run functional sweep: `python3 scripts/full-functional-sweep.py`
- Run clippy & deny check: `cargo clippy --workspace -- -D warnings` and `cargo deny check`
