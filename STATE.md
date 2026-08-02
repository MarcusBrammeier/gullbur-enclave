# Gullbúr Enclave — Project State

> **Version:** 0.1.0  
> **Last updated:** 2026-08-01  
> **HEAD:** `f46f193` (2026-08-01 22:41 UTC)  
> **CI:** `cargo check --workspace` ✅ | `cargo test --lib` ✅ (296+ passed, 1 ignored) | `cargo clippy -D warnings` ✅ | `cargo deny check` ✅ | `cargo audit` ✅ | `cargo +nightly fuzz build` ✅ | 12MB APK ✅

---

## Project Overview

**Gullbúr Enclave** (formerly FOSS Crypto Core) is a modular, open-source multi-chain cryptocurrency wallet — pure Rust, zero `unsafe`, WASM-isolated crypto.

| Property | Value |
|----------|-------|
| **Language** | Rust 2024 edition, TypeScript (Svelte 5) |
| **Version** | 0.1.0 |
| **Workspace members** | 20 crates/apps |
| **Chains** | Bitcoin (BIP-84), Ethereum/EVM (EIP-1559), Monero (CLSAG), Litecoin (BIP-84) |
| **Desktop shell** | Tauri v2 + Svelte 5 + Tailwind CSS |
| **IPC** | WebSocket `127.0.0.1:19876` — JSON-RPC 2.0, AES-256-GCM encrypted |
| **Auth** | Biometric (TouchID/Windows Hello/Android fingerprint), FIDO2 YubiKey |
| **License** | MIT OR Apache-2.0 |

### Workspace Layout (20 members)

```
crates/
├── core:          crypto-core (BIP-39/32/44, ECDSA/Schnorr/ed25519), crypto-isolation (AES-256-GCM, WASM-safe)
├── storage:       keystore-core (OS keychain + envelope encryption)
├── protocol:      ipc-protocol (JSON-RPC 2.0 types), ipc-core (WebSocket server, token auth)
├── plugins:       wallet-plugin (trait + shared types), plugin-manifest (FPI manifest validation)
│   ├── btc/       — Bitcoin: BIP-84 P2WPKH, PSBT signing, Esplora RPC
│   ├── evm/       — Ethereum/EVM: EIP-1559, 6 chains via RPC switch
│   ├── xmr/       — Monero: CLSAG ring sigs, daemon RPC, wallet-rpc balance
│   └── ltc/       — Litecoin: FPI reference implementation (Scrypt-based)
├── infra:         tor-daemon (arti child-process manager), auth-core (biometric/FIDO2/auto-lock)
├── server:        vault-core (orchestrator, IPC handler registration)
├── relay:         extension-relay (browser extension native messaging host)
├── update:        update-checker (GitHub release version check)
apps/
├── desktop/       — Tauri v2 + Svelte 5 desktop GUI
├── cli/           — Internal testing CLI (13 subcommands, WebSocket IPC)
tests/
├── cli-integration/ — Integration tests (27 tests)
fuzz/              — cargo-fuzz targets (nightly only, 5 targets)
```

---

## Recent Commits (all 20+)

```
f46f193 fix: fmt — move inline comment below fn call for cargo fmt compliance
170f070 fix: remove invalid compressNativeLibs manifest attribute (blocks APK build)
f16e8dd sweep: fix clippy, unbundle center(), replace GitHub placeholders with YOUR_GITHUB_ORG
8347b5b config: cargo registry and build flags
d92c5cd config: cargo config, deny policy, workspace lockfile, STATE.md
172d797 crates: IPC core, wallet-plugin, tauri-mcp, vault-core, LTC plugin
820e984 desktop: Svelte UI, Tauri backend, Android config
119aab5 fix: suppress all Tauri transitive unmaintained advisories in audit script
719131e chore: APK shrink — compress native libs, trim META-INF and kotlin metadata
2a8881d fix: e2e signing flow — pass hex-encoded seed as key_id instead of account name
4a9e4f9 fix: test race condition via serial_test, default features for vault-core plugins
ebcde4a chore: pre-beta sweep — clippy fixes, handle cleanup, new files
d90030b feat: crash reporter + DONATIONS.md prepared for GitHub info
751b048 fix: rebrand demo.html title to Gullbúr Enclave
9529653 fix: rebrand remaining fosscrypto refs to Gullbúr Enclave
9c729f7 fix: clippy lint fixes and license audit allowlist
7bb3317 ci: harden gates, add fuzz schedule + Android job
c36a865 chore: pre-shrink sweep fixes
0050f68 Initial commit — Gullbúr Enclave
```

---

## Sweep Status (2026-08-01)

| Gate | Status |
|------|--------|
| `cargo fmt --check` | ✅ |
| `cargo check --workspace` | ✅ |
| `cargo clippy --workspace -- -D warnings` | ✅ (zero warnings) |
| `cargo test --workspace --lib` | ✅ (296 passed, 1 ignored) |
| `cargo test -p cli-integration` | ✅ (27 passed) |
| `cargo deny check` | ✅ |
| `bash scripts/audit.sh` | ✅ (advisories suppressed for 17 known Tauri transitive) |
| `cargo +nightly fuzz build` | ✅ |
| `cargo +nightly fuzz run` (5 targets, 1k ea) | ✅ (zero crashes) |
| `cargo tauri android build --target aarch64` | ✅ (12MB APK, 12MB AAB) |

### Fixes Applied This Session

1. **IPC token fallback path** — `ok_or_else` → `ok_or` (clippy fix in ipc-core/server.rs)
2. **`win.center()` removed** — API doesn't exist in Tauri 2.x lib.rs
3. **`android:compressNativeLibs="true"` removed** — not a valid manifest attribute, blocks AAPT resource linking
4. **GitHub placeholders** — all 10 `gullbur/gullbur` / `gullbur/gullburcore` → `YOUR_GITHUB_ORG/YOUR_GITHUB_REPO` with `// REPLACE_ME` markers
5. **EVM BIP-44 derivation** — `derive_key_from_keyid()` now uses `derive_bip44_eth_key()` with correct index from `key_id@index` format. Includes cross-index differentiation test.
6. **tauri-mcp unused import** — removed unused `use serde_json::json;`
7. **AppImage libgcrypt** — confirmed already configured in `tauri.conf.json` with symlinks in `local-libs/`

---

## Known Issues

| Issue | Impact | Status |
|-------|--------|--------|
| BTC/LTC PSBT signing only signs `inputs[0]` | Was only signing first input — multi-input PSBTs partially signed | ✅ Fixed this session — loop over all inputs |
| BTC/LTC hardcoded account index 0 | Actually already fixed (key_id@index format parsed) | ✅ Fixed in prior session |
| EVM BIP-44 derivation was ignored | Was signing all accounts with same key | ✅ Fixed this session |
| `cargo deny` suppresses 17 Tauri transitive `unmaintained` advisories | Low risk (transitive, non-core deps) | Accepted |
| GitHub URLs are `YOUR_GITHUB_ORG` placeholders | User must set actual org/repo before publishing | Placeholder (set via grep) |

---

## KeyHandle Trait Status

The STATE.md mentioned a "KeyHandle trait overhaul" but BTC/LTC PSBT already parse `key_id@index` format and EVM now uses BIP-44 derivation. The `KeyHandle` struct on the trait side still only has `key_id: String` — the index is encoded in the string format rather than a dedicated field. This works correctly but could be cleaner with `derivation_path: Option<String>`.

---

## Deliverables

- **APK** (12MB): `apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`
- **AAB** (12MB): `apps/desktop/src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab`
- **5 fuzz targets**: aes_gcm, bip39, json_rpc, psbt, validate_address
- **296+ unit tests** + **27 integration tests** — all passing