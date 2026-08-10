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

## Phase 3 — Public Staging & Beta Release (v0.1 Beta)

1. **Documentation Refresh**
   - Update `STATE.md` and `FOSS_BOUNDARY.md` to reflect current HEAD, test coverage, and pre-release status.

2. **Clean FOSS Tree Staging**
   - Prepare clean public staging repository per `FOSS_BOUNDARY.md` rules (exclude `apps/cli`, internal plans, debug logs, private scripts).

3. **Release Artifact Build & Deploy**
   - Build and verify release binaries:
     - Linux AppImage & `.deb` package
     - Android arm64 `.apk` & `.aab`
   - Draft GitHub Release `v0.1.0-beta.1` with release notes, checksums, and artifact downloads.

---

## Phase 4 — Post-Beta Roadmap (1.0 Path)

1. Physical Android device testing & biometric authentication verification.
2. Private key import/export UX (WASM encrypted key derivation).
3. Audit readiness preparation & FOSS license transition plan (BSL-1.1 -> Apache-2.0 at 1.0).
