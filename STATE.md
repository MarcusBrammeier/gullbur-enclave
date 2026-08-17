# Gullbúr Enclave — Project State

> **Version:** 0.1.0-beta.1 (public staging)
> **Last updated:** 2026-08-17
> **HEAD:** `4b5ea60` (2026-08-17)
> **CI:** `cargo check --workspace` ✅ | `cargo test --workspace --lib` ✅ (322 passed, 1 ignored) | `cargo test -p cli-integration` ✅ (6 passed) | account persistence (3) ✅ | **IPC e2e + engine_security (13) + staged-mnemonic e2e (2) ✅** | **WASM crypto round-trip (7) ✅** | **frontend Svelte component tests ✅ (248)** | `cargo fmt --check` ✅ | `cargo clippy -D clippy::unwrap_used` ✅ | `cargo deny check` ✅ | `cargo audit` ✅ | `cargo +nightly fuzz build` ✅ | **full 13-layer sweep ✅** | Linux AppImage/.deb + Android APK/AAB ✅

### Security hardening (2026-08-08)

- **Argon2id KDF** (m=32 MiB, t=3, p=1) replacing fast HKDF-SHA256 for the
  AES-256-GCM envelope — with a self-describing v2 blob (`GBKF` magic + params
  header) and **transparent legacy-HKDF migration** (older saves keep decrypting).
- **`accounts.json` encrypted at rest** (device-key `GBAF` AES-256-GCM blob),
  readable pre-unlock; legacy plaintext still loads.
- **Seed + accounts files mode 0600** on Unix (were 0644).
- **Staged-mnemonic IPC** — a generated seed is held in Rust, returned to the
  UI once for backup, never re-submitted; `clear_staged` discards it on back-out;
  Settings reveal scrubs the seed on Hide/close.

### Features & robustness (2026-08-08, batch 2)

- **Account uniqueness** — `Account.index` now serialized on the wire
  (`#[serde(default)]`, backward-compat) and populated in all 4 plugin
  derivation paths; UI can finally derive truly distinct BIP-44 addresses
  (was: 3rd+ accounts all reused index 1). Regression test asserts 8 distinct
  indices → 8 distinct addresses.
- **Device-key → OS keychain** — `TieredDeviceKeyProvider` (keychain-first,
  file fallback) with a **persistence round-trip probe** before trusting the
  backend (headless-Linux keyring can return `Ok` yet regenerate a different
  key per call, which would make the encrypted seed undecryptable).
- **Address book** — persistent per-network saved recipients in localStorage
  (non-secrets only), with save/update/remove + 6 store tests; wired into Send.
- **QR receive-scan** — `QrScanner.svelte` (getUserMedia + jsQR) fills + auto-
  validates the recipient address; Android CAMERA permission added.
- **E2EE IPC confirmed** — session-key handshake + AES-256-GCM both directions
  (server decrypts requests / encrypts responses; client encrypts every request
  via the WASM crypto blob); verified end-to-end (`e2e_ipc_encrypted`).
- **UI accent-theme system** — 5 accent presets (emerald/violet/amber/cyan/rose)
  drive `--color-accent` tokens + motion tokens; accent picker in OptionsBar.
- **Balance refresh single-flight + tx-history stale-guard** — concurrent
  refreshes coalesce; account-switch history fetches can't overwrite out-of-order.
- *Note:* EVM gas was already REAL on the Rust side (`eth_estimateGas` +
  `eth_call`); the hardcoded `21000` exists only in the `IS_DEMO` browser mock.

### Batch 3 — Security IPC & hardening (2026-08-09)

- **Seed removed from IPC key_id (P1)** — `sign_transaction` now takes
  `(tx, &[u8], u32, network)` instead of `&KeyHandle` with the seed hex-encoded
  into `key_id`. The master seed is read from `Arc<RwLock<seed>>` inside
  `ipc_handlers.rs` and never circulates through the wire protocol. All 4
  plugins updated, all bridges/handlers/commands updated.
- **Extension-relay rate limiter (P1)** — sliding-window 30 req/min per origin,
  max 3 concurrent pending approvals. Applied in `gullbur-relay` main loop
  before origin validation.
- **Unencrypted IPC gated (P1)** — production `LifecycleManager` always calls
  `IpcServer::new(port)` (encrypted). `with_no_encrypt()` is `#[cfg(test)]`
  only. The `with_encryption(port, false)` path is still reachable for external
  test crates but never in production builds.
- **Extension-relay rate limiter (P1)** — `rate_limiter.rs`: sliding-window
  30 req/min per origin, max 3 concurrent approvals, wired into main loop.
- **Removed redundant `[workspace]` from crypto-wasm (P3)** — the standalone
  workspace declaration was a copy-paste artifact that could cause build issues.
- **Deprecated legacy HMAC key derivation (P3)** — `derive_k256_key` and
  `derive_secp256k1_key` marked `#[deprecated]` with migration notes pointing
  to `derive_bip44_eth_key` and plugin-side derivation. No callers deleted
  (backward compat).

### Batch 4 — Real XMR decoy selection (2026-08-09)

- **Real decoy selection wired into CLSAG signing (P0)** — `sign_monero_tx` is
  now `async` and accepts an optional `reqwest` daemon client. When the daemon
  is reachable, `fetch_and_build_ring()` fetches *actual blockchain UTXO output
  keys* via `get_output_distribution` + `get_outs` daemon RPC. Ring signature
  privacy is now real.
- **Graceful fallback** — if the daemon is unreachable or returns an error,
  the signer falls back to synthetic random decoys (safe for testnet/dev; a
  mainnet deployment should always have a daemon connection).
- **Real blockchain offsets** — the returned `offsets` array contains actual
  output indices for `Decoys::new`, replacing the all-zero placeholder that
  would produce invalid transactions on-chain.
- **Removed `#[allow(dead_code)]`** — all decoy_selector functions are now
  exercised through the signing path. The old `build_decoy_ring` is still
  present as a synthetic fallback.

### Batch 5 — UI Polish, Address Book Encryption & Build Pipeline (2026-08-09)

- **Sidebar navigation** — replaced crowded top-header layout with persistent
  sidebar nav (`w-64`, fixed-left) using SVG icons for Dashboard, Portfolio,
  Send, Receive, Settings. Active state tracking via `activeView`.
- **All emoji → SVG icons** — every button and nav item now uses inline
  feather-style SVG icons from `icons.ts`. Zero emoji in UI chrome.
- **Demo/test mode** — `VITE_DEMO=true` / `?demo=true` enables `MockIpcClient`
  with realistic fake balance data. **Amber sticky warning banner** at top:
  "GUI Test Mode — Core engine not connected. Mock data for visual review only."
  Includes an ✕ exit button.
- **Address book encrypted at rest** — `vault.encrypt_data` / `vault.decrypt_data`
  IPC handlers using device-key AES-256-GCM (`encrypt_file_with_key`).
  Frontend `addressBook.ts` wires the IPC client on connect; falls back to
  cleartext localStorage when IPC unavailable (demo mode).
- **Rich mock data** — `MockIpcClient` now returns multi-chain accounts
  (ETH, Arbitrum, Base, Polygon, Sepolia) with realistic addresses, balances,
  transaction history, and fee estimates.
- **Build pipeline automated** — Linux AppImage (177M) + .deb (4.7M) +
  Android APK (23M) + AAB (12M) all built from latest HEAD. Served via
  local webserver at `http://192.168.50.184:8080/`.

---

### Pre-Beta Readiness (2026-08-10 — for v0.1.0-beta.1)

- **EIP-55 checksum hardening** — EVM `validate_address` now performs full
  mixed-case keccak checksum verification (accepts all-lower/all-upper/correct
  mixed-case; rejects invalid mixed-case). Regression tests incl. all-uppercase.
- **XMR disconnected UX** — when `monero-wallet-rpc` is unconfigured, the UI
  shows a yellow "Wallet-RPC disconnected — balance unavailable" badge instead
  of a misleading silent 0 XMR (Dashboard + Portfolio).
- **Live-network nightly CI** — `.github/workflows/live-network.yml`: nightly
  03:17 UTC + manual + tag triggers; runs `#[ignore]`d live tests (EVM RPC,
  Tor real-circuit, funded-wallet broadcasts gated on secrets).
- **UI input fuzzing** — `Send.fuzz.test.ts` (18 cases): zero-width unicode,
  homoglyph addresses, malformed amounts, XSS/path-traversal injection on the
  send wizard. Confirms the `type="number"` amount field sanitizes non-numeric
  input (Continue stays disabled).
- **GUI workflow tests** — `QrScanner.test.ts` (camera error/stream wiring) +
  expanded `addressBook.test.ts` (round-trip, corrupt-data recovery, malformed
  entry filtering).
- **Full 13-layer sweep green** — Layers 8-10 (full functional sweep, 20-account
  E2E stress, daemon crash→restart→reconnect recovery) fixed: headless CLI
  build (un-gated `with_no_encrypt`), numeric JSON-RPC IDs, inode-based PID
  resolution, correct crash-recovery contract. Layers 11-13 added 2026-08-13:
  **WASM crypto round-trip** (real shipped blob), **frontend vitest suite**
  (248 tests), and **static gates** (fmt + clippy + deny + audit) mirroring CI
  so local green == CI green. **ALL 13 LAYERS PASSED.**
- **Standard release gate** — `scripts/full-test-sweep.sh` is now the canonical
  13-layer gate (run before every `build-all.sh` + push): compile → unit →
  integration → persistence → IPC e2e/security → fuzz build → branding audit →
  desktop binary → full functional → E2E stress → disconnect recovery → WASM
  crypto round-trip → frontend vitest → static gates. All Layers gate on **tool
  exit codes** (not `grep -q` on output) to avoid the pipefail/SIGPIPE pitfall,
  so a green run is authoritative.

### Session 2026-08-17 — LTC testnet split + XMR daemon resilience + E2E proof

- **LTC testnet3/testnet4 split** (`26559e6`): `litecoin-testnet` split into
  explicit `litecoin-testnet3` (→ `/testnet/api`, `Network::Testnet`) and
  `litecoin-testnet4` (→ `/testnet4/api`, `Network::Testnet4`), with
  `litecoin-testnet` kept as a backward-compat alias to testnet3. Both derive
  the identical legacy P2PKH `m/n` address (`m/84'/2'/0'/0/i`), so derivation,
  signing and address-validation are shared; only the Esplora base and
  `bitcoin::Network` differ.
- **LTC P2PKH address-helper fix** (`1dde27b`): the address helpers initially
  rejected the new split ids — caught by a focused split-verification test
  before it shipped.
- **LTC regtest E2E test** (`46bfd10`): funds our derived P2PKH address on a
  LOCAL `litecoind` regtest node, signs a spend through `LtcPlugin` legacy
  P2PKH path, broadcasts + confirms on-chain (txid `56992f8c…`, 1 confirm).
  Proves sign→broadcast E2E deterministically, independent of flaky public
  faucets.
- **XMR configurable daemon endpoints + failover** (`ede9d6c`): replaced the
  single hardcoded daemon URL with an ordered per-network fallback list
  (default `node.monerodevs.org`, verified live on mainnet 18089 / stagenet
  38089 / testnet 28089). New `XmrPlugin::with_daemon_urls` builder; `daemon_rpc_with`
  fails over to the next endpoint on failure. `broadcast_transaction` and
  `estimate_fee` route through it; decoys stay best-effort. Live
  `estimate_fee` returns real fees (20000 piconero) on all three networks.
- **LTC live testnet3 broadcast test** (`4b5ea60`): ignored-by-default test
  spending our derived P2PKH UTXO on the public testnet3 chain.

## Project Overview

**Gullbúr Enclave** (formerly FOSS Crypto Core) is a modular, open-source multi-chain cryptocurrency wallet — pure Rust, zero `unsafe`, WASM-isolated crypto.

| Property | Value |
|----------|-------|
| **Language** | Rust 2024 edition, TypeScript (Svelte 5) |
| **Version** | 0.1.0-beta.1 |
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
87e88b3 perf/robustness: single-flight balance refresh + stale-guard on tx history (regression-fixed effect loop)
2aa64ce feat(ui): accent-theme system + motion tokens + premium polish (Phase A)
85d96c0 fix(android): add CAMERA permission; feat(ui): address book + QR camera scan for recipient selection
3b76169 fix: serialize BIP-44 index on Account so the UI derives truly unique addresses
520e136 security: tiered device-key provider (OS keychain → file fallback)
98fd69e docs: refresh STATE.md — security hardening summary, 303 unit + staged-mnemonic e2e
e8ac852 security: keep generated seed in Rust via staged-mnemonic IPC; scrub JS seed state
9fe94fa security: argon2id KDF for encrypted vault + device-key encrypted accounts.json at rest
3f23aa6 docs: refresh STATE.md to v0.0.8 — 296 unit + 83 frontend tests, current HEAD
6e2af69 Revert "fix(ui): modal accessibility — backdrop-close via onclick|self, Escape/tabindex, labeled debug-comments textarea"
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

## Sweep Status (2026-08-08 — Stage 1+2 complete)

| Gate | Status |
|------|--------|
| `cargo fmt --check` | ✅ |
| `cargo check --workspace` | ✅ |
| `cargo clippy --workspace -- -D warnings` | ✅ (zero warnings) |
| `cargo test --workspace --lib` | ✅ (314 passed, 1 ignored) |
| `cargo test -p cli-integration` | ✅ (6 passed, 41 total) |
| `cargo test -p vault-core --test e2e_websocket` | ✅ (13 methods) |
| `cargo test -p vault-core --test account_persistence` | ✅ (3 passed) |
| **WASM crypto round-trip** (`crypto_wasm.test.ts`) | ✅ (7 tests, real shipped blob) |
| **IPC e2e handshake: e2e_ipc_flow / e2e_disconnect_reconnect / e2e_full_lifecycle** | ✅ (3 tests) |
| `cargo deny check` | ✅ |
| `bash scripts/audit.sh` | ✅ (advisories suppressed for 17 known Tauri transitive) |
| `cargo +nightly fuzz build` | ✅ |
| `cargo +nightly fuzz run` (5 targets, 1k ea) | ✅ (zero crashes) |
| `cargo tauri android build --target aarch64` | ✅ (APK 12M universal, **AAB 7.1M**) |
| `scripts/cli-binary-sweep.sh` | ✅ (12 checks incl. **real WS handshake probe**) |
| `scripts/android-sweep.sh` | ✅ (added **on-device adb-forward WS handshake**) |
| **scripts/full-functional-sweep.py** | ✅ **33/33 checks** |
| **— Stage 1: Testing Upgrades (2026-08-08) —** | |
| **Svelte component tests** | ✅ **248 tests** (per Layer 12 vitest sweep, 2026-08-13) |
| **scripts/e2e-full-stack-sweep.py** | ✅ **20-account concurrent balance + tx-history stress** |
| **scripts/disconnect-recovery-test.py** | ✅ **Daemon crash → restart → reconnect lifecycle** |
| **Edge-case input fuzzing** | ✅ **8 fuzz tests** (zero-width chars, homoglyphs, XSS, path traversal, amount fuzzing) |
| **— Stage 2: Theme Engine (2026-08-08) —** | |
| **themeEngine.svelte.ts** | ✅ Zod-validated, reactive Svelte 5 $state, immutable builtins |
| **Built-in themes** | ✅ `legacy-emerald` (fallback), `dark-slate`, `light-slate` |
| **Security boundary** | ✅ Rejects url(), javascript:, HTML injection, eval, path traversal |
| **Theme import/export** | ✅ JSON export/import with Zod validation |
| **— Stage 3: Tactical UI Polish (2026-08-08) —** | |
| **Tactical button press** | ✅ 1px compression on active, split CSS transitions |
| **Focus rings** | ✅ `--focus-ring` token on all interactive elements |
| **Motion speed presets** | ✅ instant / normal / expressive via data-motion attribute |
| **Accent selector** | ✅ 5 presets via themeEngine in OptionsBar |
| **Card hover effect** | ✅ Restored (.card:hover) after CSS refactor |

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
- **314+ unit tests** + **6 CLI integration** + **13 IPC e2e/security** + **2 staged-mnemonic e2e** + **248 frontend tests** (incl. 7 WASM crypto round-trip) — all passing
- **Address book store** (`apps/desktop/src/lib/addressBook.ts`) — persistent per-network recipients + 6 tests
- **QR receive-scan** (`QrScanner.svelte`) — getUserMedia + jsQR recipient scanning
- **Accent-theme system** — 5 presets (emerald/violet/amber/cyan/rose) + motion tokens
- **ws-handshake-probe.py** — live WS hello→session_key→RPC probe used by CLI & Android sweeps
- **full-functional-sweep.py** — **33/33 checks**, every IPC method exercised via real binary WS protocol