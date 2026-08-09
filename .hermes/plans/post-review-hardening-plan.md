# Gullbúr Enclave — Post-Architecture Review Hardening Plan

> **Goal:** Execute the 6 prioritized security, privacy, and architecture hardening tasks before returning to the Svelte 5 UI overhaul.

---

## Task Matrix

| # | Task | Target Crate(s) | Description / Approach | Priority |
|---|------|-----------------|------------------------|----------|
| 1 | **Real XMR Decoy Selection** | `plugin-xmr` | Replace random point decoy generation in `build_decoy_ring()` with real UTXO decoy selection fetched via `monero-wallet-rpc` / daemon RPC (`get_output_distribution` / `get_outputs`). Refuse to sign if daemon RPC is unreachable (no fake fallback). | **P0 (Mainnet Blocker)** |
| 2 | **Decouple Master Seed from `key_id`** | `vault-core`, `ipc-core`, `ipc-handlers` | Remove raw hex seed from `key_id` string on the wire (`"seed_hex@index"` pattern). Update `PluginHost` and IPC handlers to hold the seed in `Arc<RwLock<Option<Zeroizing<Vec<u8>>>>>` and dispatch via `account_index: u32`. | **P1** |
| 3 | **Rate Limiter & Request Dedup in Extension Relay** | `extension-relay` | Implement sliding-window rate limiting (requests/sec per origin) and max pending approval queue cap in `gullbur-relay`. Add 2-second cache for idempotent reads (`eth_getBalance`, `eth_chainId`, `eth_accounts`). | **P1** |
| 4 | **Compile-Gate Plaintext IPC Mode** | `ipc-core` | Restrict `IpcServer::with_encryption(bind_port, false)` behind `#[cfg(test)]` so production builds unconditionally enforce AES-256-GCM. | **P1** |
| 5 | **Encrypt Address Book at Rest via IPC** | `vault-core`, Svelte UI | Add `vault.encrypt_data` / `vault.decrypt_data` IPC handlers backed by the device key (`GBAF` format). Update `addressBook.ts` in Svelte UI to transparently encrypt on save and decrypt on load. | **P2** |
| 6 | **Fix `crypto-wasm` Workspace Declaration** | `crypto-wasm/Cargo.toml`, root `Cargo.toml` | Remove internal `[workspace]` block from `crypto-wasm/Cargo.toml`. Add `crates/crypto-wasm` to root `Cargo.toml` workspace members with proper target conditional checks for `wasm-pack`. | **P2** |
| 7 | **Clean Up Legacy Custom HMAC Code** | `crypto-core` | Deprecate/remove legacy `derive_k256_key` / `derive_secp256k1_key` custom HMAC derivation functions to eliminate confusion with standard BIP-39/44/84 paths. | **P3 (Cleanup)** |

---

## Verification & Gate Criteria

1. `cargo check --workspace` & `cargo clippy --workspace -- -D warnings`
2. `cargo test --workspace --lib` (304+ unit tests passing)
3. `cargo test -p vault-core --test e2e_websocket`
4. `python3 scripts/full-functional-sweep.py` (33/33 checks PASS)
5. `bash scripts/full-test-sweep.sh`
