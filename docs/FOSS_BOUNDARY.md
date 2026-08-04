# Gullbúr Enclave — FOSS / Premium Boundary

> **Status:** Pre-Beta v0.0.7 (internal, private repo)
> **Purpose:** Defines what ships in the public **FOSS** release (v0.1 Beta → 1.0)
> vs. what stays proprietary as **Pro/Enterprise** features.
> This document is INTERNAL. Do not commit to a public repo.

---

## Guiding Principle

The public FOSS project contains **only** code directly related to the core
features released at 1.0. Nothing internal — test tooling, plans, roadmap,
internal documents, or polished-but-unreleased premium work — appears in the
public tree. The public repo is scrubbed and staged independently at v0.1 Beta.

## Feature Matrix

| Area | FOSS (public @ 1.0) | Pro / Enterprise (private) |
|------|---------------------|----------------------------|
| **Core wallet engine** | ✅ Vault: BTC/ETH/XMR/LTC | — |
| **Desktop GUI (Tauri + Svelte)** | ✅ Full app | — |
| **Seed management** | ✅ BIP-39 generate/restore, KeyStore seam | Hardware KeyStore backend *(perms-gated)* |
| **Auth** | ✅ Lock/unlock, auto-lock | Biometric (fingerprint) + FIDO2 backend |
| **CLI (`gullbur-cli`)** | ❌ NOT public | ✅ **Pro/Enterprise** — internal test/power tool |
| **Extension relay (EIP-6963)** | ✅ dApp bridge | Batch / session-key / simulate-and-send (advanced) |
| **Diagnostics & error reporting** | ✅ Status bar, copy terminal, review-then-send report | Raw log streaming, deep engine introspection |
| **Update checker** | ✅ Version check + banner | — |

## How to keep the boundary clean

1. **Feature-flag the premium surface.** Premium capabilities are compiled in
   behind `#[cfg(feature = "pro")]` / a `tauri`-level flag. The public build
   omits that feature; the premium build enables it.
2. **CLI excluded from public.** `apps/cli` is **not** published in the FOSS
   repo (controller: remove or vendor privately). It ships as a Pro/Enterprise
   binary at 1.0.
3. **Separate public staging repo.** At v0.1 Beta, a clean public repo is staged
   from the private dev repo, committing only the FOSS tree — no git history of
   internal docs/plans/tooling.
4. **Scrub anything internal** from what enters a public commit: `STATE.md`,
   internal scripts, roadmap/plan docs, `.env`, signing keystore, test wallets.

## What the public Diagnostics panel includes (review-first)

- Status bar + engine-process view (state, version, plugins, accounts)
- Copy terminal button
- **Send Error Report**: auto-captures logs, redacts PII (addresses, keys, IPs,
  paths), and the user **reviews before sending** — to copy, or open a GitHub
  issue with a sanitized body.

## Pro/Enterprise features planned (announced only at 1.0)

- GUI-complete CLI / headless CLI automation
- Advanced extension batching, session keys, simulate-and-send
- Hardware KeyStore (TEE) + biometric/FIDO2 guarantees
- Extended multi-signature & policy tooling

---

*Last updated: 2026-08-03 (v0.0.7 pre-beta).*
