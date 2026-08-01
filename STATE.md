# Gullbúr Enclave — Project State

> **Version:** 0.1.0  
> **Last updated:** 2026-07-31  
> **HEAD:** `119aab5` (2026-07-31 22:59 UTC)  
> **CI:** `cargo check --workspace` ✅ | `cargo test --lib` ✅ (290+ passed, 1 ignored)

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
├── cli-integration/ — Integration tests
fuzz/              — cargo-fuzz targets (nightly only)
```

### Architecture Decisions

| # | Date | Decision | Status |
|---|------|----------|--------|
| 1–15 | 2026-07 | Multi-crate workspace, FPI plugin discovery, pure Rust only, arti daemon, monero-serai, dual-channel API | ✅ Settled |
| 2026-08-01 | Batch alpha-2 completed — LTC legacy addresses, IPC fallback path, OptionsBar UI, GitHub URLs, address audit, AppImage | ✅ |

---

## Recent Commits (all 14)

```
119aab5  fix: suppress all Tauri transitive unmaintained advisories in audit script
719131e  chore: APK shrink — compress native libs, trim META-INF and kotlin metadata
2a8881d  fix: e2e signing flow — pass hex-encoded seed as key_id instead of account name
4a9e4f9  fix: test race condition via serial_test, default features for vault-core plugins
ebcde4a  chore: pre-beta sweep — clippy fixes, handle cleanup, new files
d90030b  feat: crash reporter + DONATIONS.md prepared for GitHub info
751b048  fix: rebrand demo.html title to Gullbúr Enclave
9529653  fix: rebrand remaining fosscrypto refs to Gullbúr Enclave
9c729f7  fix: clippy lint fixes and license audit allowlist
7bb3317  ci: harden gates, add fuzz schedule + Android job
c36a865  chore: pre-shrink sweep fixes
0050f68  Initial commit — Gullbúr Enclave
```

> **Note:** Commits `c672872` and `6b1260a` are duplicate authored versions of `2a8881d` and `4a9e4f9` respectively (same tree, different author).

---

## Recent Batch Work

### 1. LTC Legacy P2PKH Address (`ltc_p2pkh_address`)
Testnet faucets reject Bech32 (`tltc1...`) — they only accept legacy base58-check P2PKH. Added `ltc_p2pkh_address()` encoding a `CompressedPublicKey` into P2PKH format with correct version bytes (`0x30` mainnet / `0x6f` testnet). `create_account()` now selects P2PKH for testnet, Bech32 for mainnet.

### 2. IPC Token Path Fallback (`~/.gullbur/ipc-tokens`)
On installed `.deb` packages, `XDG_RUNTIME_DIR` and `TMPDIR` may be unset/unwritable, causing permanent "Disconnected" in the Svelte UI. Added `~/.gullbur/ipc-tokens/` fallback via `dirs_next::home_dir()` + `create_dir_all()`, resolving priority: `XDG_RUNTIME_DIR` → `TMPDIR` → `~/.gullbur/ipc-tokens` → `.`.

### 3. OptionsBar.svelte (Testnet Toggle + Theme Picker)
New Svelte 5 component with:
- Testnet-only mode toggle (default ON for beta)
- Beta-warning dialog on mainnet opt-in (`confirmMainnet()` / `cancelMainnet()`)
- Theme picker (light/dark)

### 4. Update Checker Repo → `gullbur/gullbur`
The `update-checker` crate queries `GET /repos/gullbur/gullbur/releases/latest` to notify users of new releases. `check_for_updates()` is wired as a Tauri command and rendered as a non-blocking `UpdateBanner.svelte` component.

### 5. Bug Reporter Repo → `gullbur/gullbur`
The `report_bug()` Tauri command opens `https://github.com/gullbur/gullbur/issues/new` for crash reports and manual bug reports. Fallback URL in Settings.svelte error handler also set to `gullbur/gullbur`.

---

## Test Status (`cargo test --lib`)

All **290+ tests pass**, 1 ignored (version-dependent test in `update-checker`).

| Crate | Tests | Status |
|-------|-------|--------|
| `auth-core` | 37 | ✅ |
| `crypto-core` | 32 | ✅ |
| `crypto-isolation` | 6 | ✅ |
| `ipc-protocol` | 22 | ✅ |
| `wallet-plugin` | 3 | ✅ |
| `plugin-manifest` | 6 | ✅ |
| `ipc-core` | 6 | ✅ |
| `extension-relay` | 22 | ✅ |
| `plugin-btc` | 10 | ✅ |
| `plugin-evm` | 19 | ✅ |
| `plugin-xmr` | 23 | ✅ |
| `plugin-ltc` | 23 | ✅ |
| `tor-daemon` | 7 | ✅ |
| `vault-core` | 25 | ✅ |
| `update-checker` | 52 + 1 ignored | ✅ |
| `cli` | 9 | ✅ |
| `cli-integration` | 11 | ✅ |

### CI Gates
- `cargo check --workspace` — ✅
- `cargo clippy --workspace -- -D warnings` — ✅ (minor warnings only, see known issues)
- `cargo fmt --check` — ✅
- `cargo deny check` — ✅ (with audit suppression for 3 known Tauri transitive unmaintained)
- `cargo audit` — ✅ via `scripts/audit.sh`
- Fuzz schedule — wired in CI
- Android APK build — wired in CI

---

## Known Issues

| Issue | Impact | Status |
|-------|--------|--------|
| LTC `create_account()` unused `secp` vars (3 locations) | Warnings only, no runtime impact | Minor |
| Vault-core unused `Path` import | Warning only | Minor |
| BTC/LTC PSBT signing hardcodes account index 0 | Signing uses wrong key for accounts > index 0 | Unfixed (requires `KeyHandle` trait change) |
| EVM `sign_transaction()` doesn't use BIP-44 derivation index | Bridge fix applied (sha256→hex-seed) but trait-level fix pending | Partial fix |
| AppImage `libgcrypt` missing on some distros | Desktop AppImage may fail to launch | **Needs fix** |
| GitHub URLs are `gullbur/gullbur` placeholders | Not production-forged; a pre-release sweep must finalize these | Placeholder |
| `cargo deny` suppresses 3 Tauri transitive `unmaintained` advisories | Low risk (transitive, non-core deps) | Accepted |

---

## What's Next

### High Priority
- **AppImage libgcrypt fix** — Some distros lack `libgcrypt.so.20`, causing the `.AppImage` to fail at launch. Needs bundling or a startup check with an informative error message.
- **Address cutover for beta** — Replace remaining `gullbur/gullbur` placeholders with the actual GitHub org/repo once determined. Finalize `DONATIONS.md` donation addresses.

### Medium Priority
- **KeyHandle trait overhaul** — Add `derivation_path: Option<String>` to `KeyHandle` so BTC/LTC PSBT signing and EVM signing use the correct derivation index per account.
- **APK size optimization** — Native libs compressed, META-INF trimmed; monitor APK stays ≤30 MB.
- **Mainnet opt-in flow** — `OptionsBar.svelte` beta warning dialog wired; needs E2E test coverage.

### Low Priority / Nice-to-Have
- Browser extension MV3 listing (Chrome Web Store, Firefox Add-ons)
- macOS `.dmg` signing for CI release pipeline
- Fuzz target expansion beyond `fuzz_json_rpc`

---

## References

- `ARCHITECTURE.md` — Full crate dependency graph, security boundaries, IPC protocol
- `README.md` — User-facing intro, quick start, supported chains
- Sprint archive: `// Hermes skill references/sprint-archive.md` — All completed sprints P1–P17 + Sprint A/B/D
- Development skill: `fosscrypto-core-dev` — Full development workflow, pitfalls, patterns
