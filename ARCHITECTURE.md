# Gullbúr Enclave — Architecture

> Self-custodial, desktop-first cryptocurrency wallet for Bitcoin, Ethereum, Monero, and Litecoin.
> Rust vault engine + Svelte 5 Tauri shell + browser extension relay.

## Multi-Crate Workspace (18 members)

```
gullbur/
├── crates/
│   ├── crypto-core/         ← Key derivation (BIP-39/BIP-32/BIP-44), signing (ECDSA/Schnorr/ed25519), hashing
│   ├── crypto-isolation/    ← AES-256-GCM for IPC encryption (WASM-safe, no native deps)
│   ├── keystore-core/       ← OS keychain vault + password-based envelope encryption
│   ├── ipc-protocol/        ← JSON-RPC 2.0 types, error codes, EIP-6963 schemas
│   ├── wallet-plugin/       ← WalletPlugin trait + shared types (Account, Balance, FeeEstimate)
│   ├── plugin-manifest/     ← FPI plugin.toml loading and validation
│   ├── ipc-core/            ← WebSocket server on 127.0.0.1 + one-time file token auth
│   ├── tor-daemon/          ← Out-of-process arti child-process manager
│   ├── auth-core/           ← Biometric engine, FIDO2, ERC-7579 session keys, auto-lock timer
│   ├── vault-core/          ← Orchestrator: plugin host, lifecycle, IPC handler registration
│   ├── extension-relay/     ← Native messaging host for browser extension (EIP-6963 bridge)
│   └── plugins/
│       ├── btc/             ← Bitcoin: BIP-84 P2WPKH, PSBT signing, Esplora RPC
│       ├── evm/             ← Ethereum/EVM: EIP-1559, 6 chains via RPC switch
│       ├── xmr/             ← Monero: CLSAG ring sigs, daemon RPC, wallet-rpc balance
│       └── ltc/             ← Litecoin: FPI reference implementation (Scrypt-based, BIP-84 pattern)
├── apps/
│   ├── desktop/             ← Tauri v2 + Svelte 5 + Tailwind CSS (Linux/macOS/Windows)
│   ├── cli/                 ← Internal testing CLI (13 subcommands, WebSocket IPC)
│   └── browser-extension/   ← Chrome/Firefox MV3 EIP-6963 relay extension
├── fuzz/                    ← cargo-fuzz targets (nightly only)
└── scripts/                 ← test-*.sh, CI helpers
```

## Security Boundaries

| Boundary | Rule | Enforcement |
|----------|------|-------------|
| UI ↔ Keys | **Never touch raw key material** | All signing in Rust; encrypted IPC via AES-256-GCM |
| WASM isolation | Iframe holds the encryption key | `crypto_wasm.js` blob in sandboxed iframe; main window never has key |
| Storage | OS-level AES-256-GCM | `keystore-core` + `zeroize` on drop; no localStorage/IndexedDB for secrets |
| Network | Clear-net default, Tor opt-in | `tor-daemon` child process; toggle per session |
| Auth | Biometric + optional FIDO2 | `auth-core` state machine with auto-lock timer |
| Extension | Passive relay only | Extension opens IPC channel to desktop vault; no wallet logic in JS |

## Key Dependencies

| Crate | Purpose | Status |
|-------|---------|--------|
| `aes-gcm` | IPC & storage encryption | Pure Rust, mature |
| `k256` | EVM ECDSA signatures | Pure Rust, replaces secp256k1-sys |
| `bitcoin` v0.32 | BTC data structures, PSBT, BIP-84 | Mature |
| `monero-serai-mirror` | XMR CLSAG ring sigs | Actively maintained |
| `curve25519-dalek` | Monero curve ops | Audited |
| `revm` v41 | Offline EVM simulation | Pure Rust EVM |
| `tauri` v2 | Desktop shell | Stable |

**Removed:** `alloy` (serde incompatibility), `secp256k1-sys` (C FFI), `ring`/`aws-lc-rs` (pure Rust only).

## IPC Protocol

- **Transport:** WebSocket bound to `127.0.0.1:19876`
- **Format:** JSON-RPC 2.0
- **Auth (desktop):** Loopback trust — client sends `{"type":"hello"}` on connect, server skips token check for localhost peers
- **Auth (extension):** One-time random token file stored at `$XDG_RUNTIME_DIR/gullbur-token`
- **After auth:** Encrypted session key exchange via AES-256-GCM, all subsequent messages encrypted
- **Dual channel:** Legacy `eth_*` API (MetaMask-compatible) + Next-gen `vault_*` API (ERC-4337 / ERC-7579 / simulate)

## Crate Dependency Graph (No Cycles)

```
crypto-core ← keystore-core, crypto-isolation, ipc-core, auth-core, wallet-plugin, plugin-*
ipc-protocol ← ipc-core
wallet-plugin ← plugin-btc, plugin-evm, plugin-xmr, plugin-ltc
plugin-manifest ← (FPI manifest validation, no deps on other workspace crates)
tor-daemon ← vault-core
auth-core ← vault-core
extension-relay ← ipc-core
vault-core ← (orchestrator, ties everything together)
fuzz ← ipc-protocol, wallet-plugin, plugin-xmr
```

## Architecture Decisions

| # | Date | Decision | Status |
|---|------|----------|--------|
| 1 | 2026-07-15 | Multi-crate workspace in single repo | ✅ Current |
| 2 | 2026-07-15 | Compile-time plugin discovery via features | ✅ Current |
| 3 | 2026-07-15 | **Alloy → removed** — raw reqwest + k256 for EVM | 🗑️ Replaced |
| 4 | 2026-07-15 | **aws-lc-rs → replaced** — pure Rust only | 🗑️ Replaced |
| 5 | 2026-07-15 | arti as out-of-process child daemon | ✅ Current |
| 6 | 2026-07-15 | WebSocket + one-time token for extension IPC | ✅ Current |
| 7 | 2026-07-15 | monero-serai for XMR | ✅ Current |
| 8 | 2026-07-15 | Split vault into separate crates | ✅ Current |
| 9 | 2026-07-15 | WalletPlugin trait in own crate | ✅ Current |
| 10 | 2026-07-18 | BIP-39 + BIP-32 HD derivation (replace HMAC) | ✅ Current |
| 11 | 2026-07-19 | secp256k1-sys → k256 (pure Rust BTC signing) | ✅ Current |
| 12 | 2026-07-19 | Testnet3 (not testnet4) for BTC faucet compatibility | ✅ Current |
| 13 | 2026-07-23 | FPI plugin.toml manifest validation (plugin-manifest crate) | ✅ Current |
| 14 | 2026-07-23 | fuzz/ for cargo-fuzz targets (nightly) | ✅ Current |
| 15 | 2026-07-23 | LTC plugin as FPI reference implementation | ✅ Current |

## Key Patterns

- **Security is Deterministic** — revm for offline EVM simulation. No LLM reads raw contract bytecode.
- **Zero-trust IPC** — Frontend never has access to encryption keys. Sandboxed WASM iframe on desktop.
- **Dual-channel API** — Legacy `eth_*` for MetaMask compatibility + `vault_*` for next-gen features.
- **All 3 chains broadcast-proven** — BTC (testnet3), ETH (Sepolia), XMR (stagenet).