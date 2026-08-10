# Gullbúr Enclave — Beta Release Gameplan

> **Created:** 2026-08-10  
> **Status:** Approved / Pending Execution  
> **Target:** v0.1 Beta Release & Hardening  

---

## Overview

This gameplan defines the remaining phases to transition Gullbúr Enclave from pre-beta internal state to a fully hardened, verified v0.1 Beta public staging release.

---

## Phase 1 — Technical Loose Ends & Quick Wins ✅

1. **EVM `validate_address` Checksum Hardening (P2)**
   - **File:** `crates/plugins/evm/src/plugin.rs`
   - **Task:** Upgrade format-only validation (0x + 42 hex chars) to include full EIP-55 mixed-case keccak checksum verification. Ensure it accepts all-lowercase, all-uppercase, and correct mixed-case while rejecting invalid mixed-case strings.

2. **XMR Zero-Balance UX Clarity (P2)**
   - **File:** `apps/desktop/src/lib/components/Dashboard.svelte` / `Portfolio.svelte`
   - **Task:** When Monero `wallet-rpc` is not configured, explicitly surface a "Daemon/Wallet-RPC disconnected — balance unavailable" status badge instead of silently displaying `0 XMR`.

3. **Clean Up Placeholder Test Module (P3)** — *Already resolved:* `crates/vault-core/src/commands.rs` no longer exists (pruned in `a093f80`); no `vault_file_tests` placeholder remains. `docs/CODE_SWEEP.md` updated to drop the stale finding.

4. **Live-Network Nightly Integration Job (P2)**
   - **File:** `.github/workflows/live-network.yml`
   - **Task:** Add a scheduled/manual GitHub Actions workflow that executes `#[ignore]`d live-network tests (Tor real-circuit, XMR wallet-rpc live, live broadcast probes).

---

## Phase 2 — Beta Hardening & Stress Testing Pass ✅

1. **Full GUI Workflow Test Pass in Demo/Live Mode**
   - Verify multi-chain account switching, transaction history pagination/filtering, address book persistence, and QR scanner on desktop and Android formats.

2. **UI Edge-Case & Input Fuzzing**
   - Fuzz Send modal amount parsing, custom gas limits, recipient inputs, and seed backup confirmation screens.

3. **Full System Verification Sweep**
   - Run complete test matrix:
     ```bash
     cargo check --workspace
     cargo test --workspace --lib
     cargo clippy --workspace -- -D warnings
     cargo deny check
     scripts/full-test-sweep.sh
     python3 scripts/full-functional-sweep.py
     ```

---

## Phase 3 — Public Staging & Beta Release (v0.1.0-beta.1)

1. **Documentation Refresh** ✅ — `STATE.md` + (internal) `FOSS_BOUNDARY.md` bumped to
   0.1.0-beta.1, current HEAD, 307 unit + 192 component tests, 10-layer sweep green.
2. **Clean FOSS Tree Staging** ✅ — `scripts/stage-foss-tree.sh` clones the private repo,
   drops dev tags, excludes internal paths (apps/cli, .hermes, PLANS, docs/plans,
   STATE.md, FOSS_BOUNDARY, scripts), commits + tags a clean public tree (verified: 308
   files, no internal paths, only v0.1.0-beta.1 tag).
3. **Release Artifact Build & Deploy** — 🔄 in progress:
   - Linux AppImage + `.deb` built (version bumped 0.0.8 → 0.1.0).
   - Android APK/AAB built by CI on tag push (`release.yml` android job).
   - `release.yml` publish job added → drafts GitHub Release on tag push.
   - `build-all.sh` added as the canonical all-variants build gate.
   - Remaining: final `v0.1.0-beta.1` tag push to trigger CI + release draft.

---

## Phase 4 — Post-Beta Roadmap (1.0 Path)

1. Physical Android device testing & biometric authentication verification.
2. Private key import/export UX (WASM encrypted key derivation).
3. Audit readiness preparation & FOSS license transition plan (BSL-1.1 -> Apache-2.0 at 1.0).
