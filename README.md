# Gullbúr Enclave

A **source-available** multi-chain cryptocurrency wallet — built in Rust with zero unsafe code. **Becomes fully open-source (FOSS) at v1.0.**

**Self-custody. Private. Multi-Chain.** Bitcoin, Ethereum, Monero, and Litecoin — one vault, your keys, your control.

[Download the latest release](https://github.com/MarcusBrammeier/gullbur-enclave/releases) · [Report a bug](https://github.com/MarcusBrammeier/gullbur-enclave/issues/new)

---

## ⚠️ Licensing: Source-Available during Beta, FOSS at 1.0

While this project is in beta and under testing, it is released under the
**Business Source License 1.1 (BSL-1.1)** — *source-available, not open-source*.

**Why:** The engines and designs are still being tested and validated. To
prevent premature forking or porting before the project is confirmed to work,
the source is fully visible for evaluation, review, testing, and bug reporting —
but you may not fork, port, embed, or commercialize it until the Change Date.

**Commitment to Open Source:** The Licensor intends for the project to be a fully
open-source (FOSS) project as soon as it is ready — see the [LICENSE](LICENSE)
which converts to **Apache License 2.0** at the Change Date (target: the v1.0
release). Read the license for the exact terms.

---

## Downloads

| Platform | Format | Size |
|----------|--------|------|
| **Android** arm64 | `.apk` | 30 MB |
| **Linux** x86_64 | `.AppImage` | 100 MB |
| **Linux** x86_64 | `.deb` | *(CI build)* |
| **macOS** arm64 | `.dmg` | *(CI build)* |

Android APK installs as a standalone app — no desktop companion needed.

---

## What is this?

Gullbúr Enclave is a multi-chain cryptocurrency wallet that runs multiple blockchains through a single plugin system. Every chain — Bitcoin, Ethereum, Monero, Litecoin — is a plugin implementing the same trait. The Rust engine handles keys, signing, and IPC; the plugin handles chain-specific logic.

**Key properties:**

- **Zero `unsafe`** — full Rust safety guarantees across all crates
- **Plugin architecture** — add any blockchain by implementing a trait
- **BIP-39/BIP-44/84** — standard key derivation, recoverable in other wallets
- **WASM isolation** — cryptographic keys never touch the UI process (AES-256-GCM encrypted IPC)
- **Biometric unlock** — TouchID / Windows Hello / Android fingerprint
- **FIDO2 YubiKey** — optional hardware gating for high-value operations
- **Tor support** — optional SOCKS5 routing for all RPC traffic
- **CLI + desktop + mobile** — headless binary for automation, Tauri desktop shell, Android APK

## Architecture

```
┌──────────────────────────────────────────────┐
│              Desktop / CLI / Mobile           │
├──────────────────────────────────────────────┤
│             IPC WebSocket Layer               │
├──────────┬──────────┬───────────────────────┤
│  BTC     │  EVM     │  Monero               │
│  Plugin  │  Plugin  │  Plugin               │
├──────────┴──────────┴───────────────────────┤
│            Crypto Isolation                  │
│        (AES-256-GCM · WASM sandbox)          │
├──────────────────────────────────────────────┤
│           Key Store · Auth · BIP-39/44       │
└──────────────────────────────────────────────┘
```

## Supported Chains

| Chain | Networks | Key Derivation | Signing | Balance |
|-------|----------|---------------|---------|---------|
| Bitcoin | mainnet, testnet, signet | BIP-84 (SegWit) | PSBT + ECDSA | ✅ Esplora |
| Ethereum + L2s | ETH, ARB, OP, Base, POL, BNB, Sepolia | BIP-44 | EIP-1559 | ✅ JSON-RPC |
| Monero | mainnet, stagenet, testnet | BIP-44 + CLSAG | CLSAG rings | ⏳ Requires wallet-rpc |
| Litecoin | mainnet, testnet | BIP-84 | PSBT | ✅ Esplora |

## Quick Start

> The headless CLI (`gullbur-cli`) is a **Pro/Enterprise** power-user feature and
> is intentionally **not** part of the public FOSS build. Public users use the
> desktop GUI (Linux / macOS) or the Android app.

```bash
# Build + run the app
cargo build --release -p gullbur-desktop
```

## Development

```bash
cargo check --workspace
cargo test --lib
cargo +nightly fuzz build --fuzz-dir fuzz
cargo +nightly fuzz run --fuzz-dir fuzz fuzz_json_rpc -- -max_total_time=30
```

Built with Rust 2024 edition. Requires nightly for fuzzing.

## Status

**Beta.** Core crypto works across all 4 chains. All 18 beta gates pass. The Android APK is testable — sideload the `.apk` from the releases page. Testnet-only mode is on by default; mainnet access requires an explicit opt-in warning.

> **Testnet validation status:** signing/broadcast are verified on a local regtest
> node and the daemon JSON-RPC path is verified live; **public testnet relay (LTC
> testnet3/4, XMR stagenet) is still being validated.** See
> [TESTING.md](TESTING.md) for details and how to send us testnet coins.

## License

**Business Source License 1.1 (BSL-1.1)** — source-available during beta/testing.
Converts to **Apache License 2.0** at the Change Date (target: v1.0 release).
See [LICENSE](LICENSE) for full terms.

## Donate

Support the development of Gullbúr Enclave:

- **GitHub Sponsors**: [github.com/sponsors/MarcusBrammeier](https://github.com/sponsors/MarcusBrammeier)
- **Bitcoin**: *(address set at v0.1 public release)*
- **Ethereum**: *(address set at v0.1 public release)*
- **Monero**: *(address set at v0.1 public release)*

All donations go toward security audits, developer hardware, and open-source sustainability.