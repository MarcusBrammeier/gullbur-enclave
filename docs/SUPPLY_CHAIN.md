# Gullbúr Enclave — Supply Chain Report

> Version: 1.0 (2026-08-04)
> Scope: third-party dependencies, dual-version analysis, and CVE posture.

## Summary
- **0 known vulnerabilities** in the entire dependency graph.
- **0 unsound** advisories.
- **16 `unmaintained` warnings** — ALL Tauri/desktop GUI transitive deps (gtk, gdk, atk,
  gtk3-macros, proc-macro-error, unic-*). **ZERO crypto-adjacent.**
- Duplicate majors (k256 0.13+0.14, sha2 0.10+0.11, rand 0.8+0.9, secp256k1 0.27+0.29)
  are normal Rust semver resolution, NOT a security risk.

## Crypto Dependencies — Advisory Check
| Crate | Resolved | In-tree dup | Advisory? |
|-------|----------|-------------|-----------|
| k256 | 0.14.0 | 0.13.4 (via bip32) | none |
| secp256k1 | 0.29.1 | 0.27.0 | none |
| sha2 | 0.10.9 (pin) | 0.11.0 (transitive) | none |
| ed25519-dalek | 2.2.0 | — | none |
| curve25519-dalek | 4.1.3 | — | none |
| bitcoin | 0.32.102 | — | none |
| bitcoin_hashes | 0.14.101 | — | none |
| aes-gcm | 0.11.0 | — | none |
| hkdf | 0.13.0 | — | none |
| hmac | 0.12.1 | 0.13.0 | none |
| sha3 | 0.10.9 | 0.12.0 | none |
| zeroize | 1.9.0 | — | none |
| bip39 | 2.2.2 | — | none |
| bip32 | 0.5.3 | — | none |

**Result: 0/14 crypto crates have any advisory.**

## Dual-Version Analysis (why the "older versions"?)
Rust allows multiple **major** versions of a crate in one tree. The older ones here are
*transitive-only pins* — not what our code directly uses:
- **k256 0.13** ← pulled by `bip32 0.5.3` (crypto-core's BIP-32). Our workspace uses k256 **0.14**.
- **rand 0.8** ← transitive via `secp256k1 0.29 → bitcoin 0.32`, and the CLI. Workspace uses rand **0.9**.
- **sha2 0.11** ← transitive. We deliberately pin **0.10** (HKDF coupling — see note).
- **secp256k1 0.27** ← transitive behind the 0.29 the `bitcoin` crate uses.

**Deliberate pin worth flagging:** `sha2` is pinned to **0.10** because `keystore-core`'s HKDF
derivation is coupled to sha2 0.10 (and `hkdf 0.13`). A blanket `cargo update` that drifts
keystore-core to sha2 0.11 breaks HKDF. This is a *reasoned* pin, not an accident — and it
should NOT be force-upgraded without decoupling keystore-core's HKDF first.

## The 16 `unmaintained` Warnings — Are They a Risk?
| Crate | Path | Risk |
|-------|------|------|
| gtk / gdk / atk / gtk3-macros (0.18.x) | desktop GUI → Tauri | Non-core UI. Deprecated in favor of gtk-rs bindings, but functional. |
| proc-macro-error 1.0.4 | proc-macro transitive (anyhow/tauri build) | Compile-time only, not in runtime path. |
| unic-char-* (0.9.0) | unicode text processing (transitive) | Parser/formatting, not crypto. |

None are in the key-handling or networking hot path. **Accepted**, revisit before public release.

## Threat-adjacent deps with real care taken
- **monero-serai-mirror 0.1.5-alpha** — a *mirror* (fork) of Serai's Monero stack. Early-ish
  version. It is only used for CLSAG/ring-signature math and address encoding; the space is
  inherently young. Flag for the auditor's explicit review.
- **monero-clsag-mirror / monero-bulletproofs-mirror** — same mirror family.

## Recommendations
1. Keep audit/deny running in CI (already done).
2. Before public v0.1, have the auditor review `monero-*-mirror` crates explicitly.
3. Revisit the 16 `unmaintained` GUI-warnings before a public desktop release; consider
   pinning to gtk-rs 0.20+ bindings if feasible.
4. Do NOT `cargo update` blindly — pin sha2 0.10 / hkdf coupling is intentional.
5. Add a nightly **600s** fuzz schedule for deeper coverage (audit prep).

