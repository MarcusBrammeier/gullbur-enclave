# Gullbúr Enclave — Threat Model

> Version: 1.0 (2026-08-04)
> Scope: Gullbúr Enclave Core v0.1.0 (pre-beta). This is a living document for the
> third-party audit and should be updated whenever the architecture or threat surface changes.

---

## 1. Assets

| # | Asset | Sensitivity | Location |
|---|-------|-------------|----------|
| A1 | **Seed phrase / BIP-39 mnemonic** | Highest — full wallet control | Rust heap only; never on disk unencrypted |
| A2 | **Private keys (secp256k1, ed25519, k256)** | Highest | Derived on demand in Rust heap; zeroized on drop |
| A3 | **Encrypted seed at rest** | High | `~/.gullbur/keystore.key` (0600) + encrypted vault blob |
| A4 | **Device key (HKDF-derived)** | High — decrypts A3 | `~/.gullbur/keystore.key`; Android seam → KeyStore TEE |
| A5 | **Wallet addresses** | Low (public) | DB / IPC responses |
| A6 | **Transaction history & balances** | Low–Med (privacy) | DB / Esplora / wallet-rpc |
| A7 | **Auth session keys** | Med | In-memory session state |
| A8 | **Debug/crash reports** | Med (may contain addresses) | Local + optional GitHub issue |

---

## 2. Trust Boundaries

```
┌─────────────────────────────── Trusted (Rust) ───────────────────────────────┐
│  vault-core engine: keystore, key derivation, signing, auth state machine    │
│  crypto-core: primitives (zero-unsafe)                                       │
│  plugins (btc/evm/xmr/ltc): network + address/signing logic                  │
└──────────────▲───────────────────────────────────────────────▲───────────────┘
               │ JSON-RPC over encrypted WS                     │ native messaging
               │ (Tauri Isolation AES-256-GCM / loopback)       │ (extension relay)
┌──────────────┴─────��────────┐                  ┌──────────────┴──────────────┐
│  GUI WebView (Svelte)       │                  │  Browser extension          │
│  — untrusted-ish (XSS risk) │                  │  — untrusted (approval-gated)│
└─────────────────────────────┘                  └─────────────────────────────┘
```

- **Boundary 1 — WebView ↔ engine:** encrypted IPC. WebView never holds the encryption key or raw key material.
- **Boundary 2 — extension ↔ engine:** native-messaging host, every privileged call goes through an approval queue.
- **Boundary 3 — engine ↔ network:** Esplora / monero-wallet-rpc / Tor SOCKS. Addresses and tx hex only.

---

## 3. Threat Actors

| Actor | Capability | Motivation |
|-------|-----------|------------|
| **Remote attacker** | Network access to public endpoints | Steal funds, phishing, DoS |
| **Malicious dApp / extension** | Runs in browser context | Trick approval, exfiltrate addresses, phish |
| **Local malware / co-tenant process** | Same OS user, can read files | Steal seed/keys from disk or memory |
| **WebView XSS** | Code execution in renderer | RPC to engine, exfiltrate public data |
| **Physical attacker** | Access to unlocked device | Steal seed, sign malicious tx |
| **Supply-chain attacker** | Compromise a dependency | Backdoor crypto or key handling |

---

## 4. Attack Surfaces & Mitigations

### 4.1 Seed / key extraction
- **Surface:** disk, memory, swap, crash dumps, backups.
- **Mitigations:**
  - Seed encrypted at rest with AES-256-GCM; key in separate 0600 file.
  - `zeroize` on all key material; `Zeroizing` wrappers for seeds.
  - Keys derived on demand, never persisted.
  - Zero production `unsafe`; `unsafe_code = "warn"` workspace lint.
  - Debug report explicitly excludes seed/keys/balances (privacy-safe by design).

### 4.2 IPC / RPC abuse
- **Surface:** WebSocket loopback, Tauri IPC, HTTP bridge, extension relay.
- **Mitigations:**
  - AES-256-GCM encrypted IPC (Tauri Isolation) — WebView can't read key.
  - Three-tier auth state machine; signing/broadcast blocked unless unlocked.
  - Extension calls gated by approval queue (`eth_sendTransaction`, `personal_sign`, `eth_requestAccounts`).
  - Structural hello-auth (not brittle string match) — locked by test.

### 4.3 Malicious / malformed input
- **Surface:** address strings, PSBTs, tx hex, RPC params.
- **Mitigations:**
  - **Address validation is checksum-aware** (XMR keccak base58, LTC base58check + bech32, BTC via `bitcoin` crate) — corrupted addresses rejected.
  - PSBT parsing via `bitcoin` crate (validates structure); multi-input signing.
  - Fuzz targets: `aes_gcm`, `bip39`, `json_rpc`, `psbt`, `validate_address` (10k runs, 0 crashes).
  - Clippy `-D warnings` + `-D clippy::unwrap_used` gates.

### 4.4 Network privacy / surveillance
- **Surface:** Esplora API, wallet-rpc, DNS.
- **Mitigations:**
  - Optional Tor SOCKS5 routing (`with_tor`) for all plugins.
  - `tor-daemon` crate manages arti; real-circuit test (HTTP 200 through SOCKS).
  - Monero stagenet/testnet support for safe testing.

### 4.5 Supply chain
- **Surface:** dependencies.
- **Mitigations:**
  - `cargo deny` (bans, licenses, sources) on every CI run.
  - `cargo audit` weekly — 668 crates, 0 vulnerabilities.
  - 17 suppressed `unmaintained` advisories = Tauri transitive only (non-core).
  - All crypto deps are well-known, actively maintained (k256, ed25519-dalek, curve25519-dalek, bitcoin, aes-gcm, sha2).

### 4.6 Physical / device theft
- **Surface:** unlocked device, biometric bypass.
- **Mitigations:**
  - Auto-lock timer after inactivity.
  - Biometric failure lockout after 5 consecutive denials (`BiometricPolicy`).
  - Android seams for KeyStore TEE + FIDO2 (deferred — needs device).
  - Mainnet gated behind explicit testnet-warning acknowledgment.

---

## 5. Residual Risks (accepted / deferred)

| Risk | Status | Notes |
|------|--------|-------|
| Android KeyStore TEE adapter | **Deferred** | Seam done; needs arm64 device to build+verify. Until then seed key is file-based on Android. |
| Android BiometricPrompt / FIDO2 adapter | **Deferred** | Seam + flow tests done; device needed. |
| 17 `unmaintained` Tauri transitive advisories | **Accepted** | Non-core, transitive only. Review periodically. |
| WebView XSS → public-data exfiltration | **Partially mitigated** | Isolation + no key in renderer; public data only. |
| No formal crypto audit yet | **Open** | This doc is the prep; audit is the gate to public v0.1. |
| `--all-targets` clippy lints (examples/tests) | **Accepted as polish** | Not in CI gate; cleaned opportunistically. |

---

## 6. Audit Checklist (mapped to this model)

- [x] Zero production `unsafe` — verified (1 non-test block, documented SAFETY).
- [x] Checksum-aware address validation (XMR/LTC/BTC).
- [x] Supply chain: deny + audit clean.
- [x] Doc coverage report generated.
- [ ] **Threat model reviewed by third party** ← this doc.
- [ ] Android device adapters (KeyStore/biometric/FIDO2) built + verified.
- [ ] Longer fuzz runs (nightly 600s) for audit.
- [ ] `#![warn(missing_docs)]` dry-run to enumerate remaining undocumented pub items.

---

*This document is internal and must be scrubbed from the public repo before the v0.1 Beta public release (see FOSS_BOUNDARY.md).*
