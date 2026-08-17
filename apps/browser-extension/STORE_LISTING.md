# Gullbúr Enclave — Extension Store Assets

## Chrome Web Store Listing

### Name
Gullbúr Enclave — Non-Custodial Multi-Chain Vault

### Short Description (132 chars)
Non-custodial, E2EE multi-chain wallet. Bitcoin, Ethereum, Monero. BIP-39/44, WASM isolated crypto, native messaging relay.

### Full Description

**Gullbúr Enclave** is a non-custodial, end-to-end encrypted cryptocurrency wallet that runs as a browser extension with a native desktop companion. Your keys never leave your machine.

**Supported Chains**
- Bitcoin (BIP-84 SegWit, PSBT signing, Esplora-backed)
- Ethereum & EVM chains (EIP-1559, `eth_call` simulation, Sepolia testnet)
- Monero (CLSAG ring signatures, wallet-rpc balance & history)
- Litecoin (Scrypt, BIP-84, Esplora-backed — FPI reference implementation)

**Security Architecture**
- Zero unsafe Rust across the entire engine
- WASM isolation — cryptographic keys never touch the browser (AES-256-GCM encrypted IPC)
- BIP-39 mnemonic with BIP-32 HD derivation — recoverable in any standard wallet
- Optional Tor SOCKS5 routing for all RPC traffic
- Biometric unlock (TouchID / Windows Hello) with FIDO2 YubiKey gating for high-value operations

**How it works**
1. Install the native companion app (open-source, builds available on GitHub)
2. Create or restore a wallet from your BIP-39 seed phrase
3. The extension relays dApp requests to the native vault via encrypted native messaging
4. Sign and broadcast transactions from any dApp — your keys stay in the native vault

**What's included**
- EIP-6963 provider injection for seamless dApp compatibility
- Encrypted WebSocket IPC between extension and native vault
- Zero-knowledge design: the extension never sees raw keys
- All blockchain logic runs in the native Rust engine

**Privacy**
- No telemetry, no analytics, no third-party RPC hijacking
- Optional Tor integration for network-level privacy
- Open-source under MIT/Apache-2.0 — fully auditable

### Category
🏦 Finance / Crypto & Web3

### Screenshots
1. `screenshots/wallet-dashboard.png` — Vault dashboard with multi-chain portfolio view (glass-morphism dark theme)
2. `screenshots/send-flow.png` — Send transaction flow with address validation, fee selection, and ETH simulation preview
3. `screenshots/wallet-init.png` — Wallet initialization screen (BIP-39 mnemonic generation with backup confirmation)
4. `screenshots/settings.png` — Settings modal with testnet toggle, Tor toggle, auto-lock slider

---

## Firefox Add-ons Listing

### Name
Gullbúr Enclave

### Summary
Non-custodial, E2EE multi-chain crypto wallet. Bitcoin, Ethereum, Monero (BIP-39, WASM crypto, Tor).

### Description
Same as Chrome listing above.

### Tags
cryptocurrency, wallet, bitcoin, ethereum, monero, bip39, bip44, security, privacy, tor, self-custody

### Homepage URL
https://github.com/MarcusBrammeier/gullbur-enclave

### Support URL
https://github.com/MarcusBrammeier/gullbur-enclave/issues

### Extension ID (configured in manifest.json)
wallet@gullbur.io

---

## Promotional Assets Needed

| Asset | Size | Description |
|-------|------|-------------|
| Store icon | 128x128 | PNG (generated: `icons/icon-128.png`) |
| Small promo tile | 440x280 | PNG — "Secure Multi-Chain Vault" |
| Large promo tile | 920x680 | PNG — Full marketing graphic |
| Marquee promo tile | 1400x560 | PNG — Wide format (YouTube) |
| Screenshot 1 | 1280x800 | Dashboard with portfolio |
| Screenshot 2 | 1280x800 | Send flow |
| Screenshot 3 | 1280x800 | Wallet init |
| Screenshot 4 | 1280x800 | Settings |

Screenshots should be captured from the running Tauri desktop app and cropped to 1280x800.