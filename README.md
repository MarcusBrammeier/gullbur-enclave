# Gullbúr Enclave

A **modular, open-source** multi-chain cryptocurrency wallet — built in Rust with zero unsafe code.

**Self-custody. Private. Multi-Chain.** Bitcoin, Ethereum, Monero, and Litecoin — one vault, your keys, your control.

[Download the latest release](https://github.com/gullbur/gullbur/releases) · [Report a bug](https://github.com/gullbur/gullbur/issues/new)

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

```bash
# Build the headless CLI
cargo build --release -p gullbur-cli --features headless

# Launch the vault server
./target/release/gullbur-cli launch

# In another terminal:
./target/release/gullbur-cli init "your seed phrase"
./target/release/gullbur-cli create-account bitcoin-testnet 0
./target/release/gullbur-cli get-balance bitcoin-testnet tb1...
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

## License

MIT OR Apache-2.0

## Donate

Support the development of Gullbúr Enclave:

- **GitHub Sponsors**: [github.com/sponsors/YOUR_USERNAME](https://github.com/sponsors/YOUR_USERNAME)
- **Bitcoin**: `bc1q...`
- **Ethereum**: `0x...`
- **Monero**: `4...`

All donations go toward security audits, developer hardware, and open-source sustainability.