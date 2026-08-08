# Gullbúr Enclave — Project State

> **Version:** 0.0.8 (pre-beta, internal)
> **Last updated:** 2026-08-08  
> **HEAD:** `f7d2164` (2026-08-08)  
> **CI:** `cargo check --workspace` ✅ | `cargo test --workspace --lib` ✅ (296 passed, 1 ignored) | `cargo test -p cli-integration` ✅ (6 passed) | account persistence (3) ✅ | **IPC e2e handshake + engine_security (13) ✅** | **frontend Svelte component tests ✅ (83)** | `cargo clippy -D warnings` ✅ | `cargo deny check` ✅ | `cargo audit` ✅ | `cargo +nightly fuzz build` ✅ | AAB 7.1MB ✅ | APK 12M ✅

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

```text
f7d2164 fix(ui): modal accessibility — backdrop-close via onclick|self, Escape/tabindex, labeled debug-comments textarea
9de7e98 test: add 50 component tests for Send/VaultInit/Portfolio/OptionsBar
dd84a7f test: fix update-checker live test for private repo (404 skip, not fail)
94f9d1e test: Phase 2.5 — headless Tauri IPC isolation routing e2e (delegated)
635e47d feat: Svelte component testing + 3s cycling error-toast system
928ed06 test: LTC multi-input PSBT signing regression + invalid-PSBT guard
a455a84 fix: each_key_duplicate when creating multiple accounts
9457ffc fix: balance visibility + UI polish (Thread B)
0308445 fix: IPC launch hardening + WASM crypto rebuild (Thread A)
f46f193 fix: fmt — move inline comment below fn call for cargo fmt compliance
170f070 fix: remove invalid compressNativeLibs manifest attribute (blocks APK build)
f16e8dd sweep: fix clippy, unbundle center(), replace GitHub placeholders with MarcusBrammeier
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

## Sweep Status (2026-08-03)

| Gate | Status |
|------|--------|
| `cargo fmt --check` | ✅ |
| `cargo check --workspace` | ✅ |
| `cargo clippy --workspace -- -D warnings` | ✅ (zero warnings) |
| `cargo test --workspace --lib` | ✅ (296 passed, 1 ignored) |
| `cargo test -p cli-integration` | ✅ (6 passed, 41 total) |
| `cargo test -p vault-core --test e2e_websocket` | ✅ (13 methods) |
| `cargo test -p vault-core --test account_persistence` | ✅ (3 passed) |
| **IPC e2e handshake: e2e_ipc_flow / e2e_disconnect_reconnect / e2e_full_lifecycle** | ✅ (3 tests) — wired into sweep as Layer 4b |
| `cargo deny check` | ✅ |
| `bash scripts/audit.sh` | ✅ (advisories suppressed for 17 known Tauri transitive) |
| `cargo +nightly fuzz build` | ✅ |
| `cargo +nightly fuzz run` (5 targets, 1k ea) | ✅ (zero crashes) |
| `cargo tauri android build --target aarch64` | ✅ (APK 12M universal, **AAB 7.1M**) |
| `scripts/cli-binary-sweep.sh` | ✅ (12 checks incl. **real WS handshake probe**) |
| `scripts/android-sweep.sh` | ✅ (added **on-device adb-forward WS handshake**) |
| **scripts/full-functional-sweep.py** | ✅ **33/33 checks** — every vault IPC method exercised against real binary |

### Fixes Applied This Batch (beta.5 + beta.6)

1. **Closed the IPC handshake verification hole** — CLI/Android sweep scripts previously only checked the port was listening (`/proc/net/tcp`). Added `scripts/ws-handshake-probe.py` doing a **real WebSocket handshake** (hello → session_key → JSON-RPC) against the live binary; wired into both `cli-binary-sweep.sh` (step 1b) and `android-sweep.sh` (adb-forward on-device).
2. **Fixed brittle hello-auth in `ipc-core`** — the loopback hello check was an exact *string* match (`== "{\"type\":\"hello\"}"`), rejecting valid whitespace-form JSON (e.g. Python's `json.dumps`). Now parsed **structurally**; locked in with `test_hello_is_structural_json`.
3. **Multi-input PSBT regression test** — `test_sign_transaction_multi_input_all_inputs_signed` proves sign_transaction signs EVERY input (not just `inputs[0]`) with the same key. Closes Phase 2.1.
4. **APK/AAB shrink** — release profile: `strip=true` (was `strip="symbols"`, keeps debuginfo) → **AAB 12M→7.1M** (under 8M target). Kept `lto="thin"` to preserve build/fuzz speed (fat LTO barely moved the `.so`). Universal APK stays 12M (bundles uncompressed arm64 `.so` for direct-mmap side-load).
5. **full-test-sweep Layer 3** — grep `-v "0 passed; 0 failed"` filter (doc-header + trailing zero) + Layer 6 branding exclusion for android-sweep.sh (pending from prior session, now committed).
6. **Phone home: repo deployed to GitHub** — `github.com/MarcusBrammeier/gullbur-enclave` (private). Github Actions CI green (apt deps, frontend build, 32 crypto-core tests, clippy, deny, cli-integration). Dependabot active.
7. **Phase 2 integration tests** — Extension-relay E2E (3 real-binary pipe tests), Tor real-circuit (3 tests with live HTTP-200 through SOCKS), XMR wallet-rpc live (real v0.18.5.1 binary, stagenet wallet). All #[ignore]d by default for CI.
8. **Fingerprint/FIDO2 seams** — `BiometricPolicy` lockout extracted (5 tests), wired into `confirm_hardware`. `VaultState::with_biometric_engine()` + `with_fido2_authenticator()` injectors. FIDO2 flow test (4 tests through object-safe dyn trait). Android adapters token-gated (need arm64 device).
9. **Full functional sweep** — `scripts/full-functional-sweep.py` exercises ALL 16 vault IPC methods through the real binary WebSocket protocol. **31/31 checks PASS** on every launch→generate→init→create (4)→status→list_networks→validate_address→list_accounts→balance→fee→history→sign→broadcast→lock→blocked→status. New Layer 8 in full-test-sweep.sh.

---

## Known Issues

| Issue | Impact | Status |
|-------|--------|--------|
| BTC/LTC PSBT signing only signs `inputs[0]` | Was only signing first input — multi-input PSBTs partially signed | ✅ Fixed this session — loop over all inputs |
| BTC/LTC hardcoded account index 0 | Actually already fixed (key_id@index format parsed) | ✅ Fixed in prior session |
| EVM BIP-44 derivation was ignored | Was signing all accounts with same key | ✅ Fixed this session |
| `cargo deny` suppresses 17 Tauri transitive `unmaintained` advisories | Low risk (transitive, non-core deps) | Accepted |
| GitHub URLs are `MarcusBrammeier` placeholders | User must set actual org/repo before publishing | Placeholder (set via grep) |

---

## KeyHandle Trait Status

The STATE.md mentioned a "KeyHandle trait overhaul" but BTC/LTC PSBT already parse `key_id@index` format and EVM now uses BIP-44 derivation. The `KeyHandle` struct on the trait side still only has `key_id: String` — the index is encoded in the string format rather than a dedicated field. This works correctly but could be cleaner with `derivation_path: Option<String>`.

---

## Deliverables

- **APK** (12M universal side-load): `apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`
- **AAB** (7.1M — Play/distribution, target hit): `apps/desktop/src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab`
- **5 fuzz targets**: aes_gcm, bip39, json_rpc, psbt, validate_address
- **296+ unit tests** + **6 CLI integration** + **13 IPC e2e/security** + **83 frontend Svelte component tests** — all passing
- **ws-handshake-probe.py** — live WS hello→session_key→RPC probe used by CLI & Android sweeps
- **full-functional-sweep.py** — **33/33 checks**, every IPC method exercised via real binary WS protocol