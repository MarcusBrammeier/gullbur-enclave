# Gullbúr Enclave — Audit Readiness Report
> Generated: 2026-08-03
> Branch: main, HEAD: d30af65

## Unsafe Blocks
- Non-test blocks: 1 (workspace lint: `unsafe_code = "warn"`)
  - `apps/desktop/src-tauri/src/lib.rs:101:                    unsafe {`
- Test-only blocks: 2
  - `crates/vault-core/tests/account_persistence.rs`
  - `crates/vault-core/tests/account_persistence.rs`

## Cryptographic Dependencies
| Dependency | Version | Purpose |
|------------|---------|---------|
| `k256` | 0.13.4 | secp256k1 ECDSA + Schnorr, BIP-340/341 |
| `ed25519-dalek` | 2.2.0 | Ed25519 signatures (Monero compatibility) |
| `curve25519-dalek` | 4.1.3 | Curve25519 for Monero CLSAG ring signatures |
| `bip32` | 0.5.3 | BIP-32 hierarchical deterministic key derivation |
| `bitcoin` | 0.32.102 | Bitcoin PSBT parsing, sighash computation, address validation |
| `aes-gcm` | 0.11.0 | AES-256-GCM for seed encryption and IPC isolation |
| `sha2` | 0.10.9 | SHA-256 hashing (BIP-39, HKDF) |
| `hkdf` | 0.13.0 | HKDF key derivation for seed encryption |
| `hmac` | 0.12.1 | HMAC-SHA512 (BIP-39 PBKDF2) |
| `zeroize` | 1.9.0 | Zeroize sensitive material on drop |

## Supply Chain
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok
- `cargo audit`: 668 crate dependencies scanned, 0 vulnerabilities
- 17 known `unmaintained` advisories SUPPRESSED (Tauri transitive deps only — non-core)
- All crypto deps use well-known, actively maintained crates

## Threat Model (Key Points)

1. **Seed never touches disk unencrypted.** Encrypted via AES-256-GCM with a per-device HKDF-derived key stored at `~/.gullbur/keystore.key` (0600 perms). On Android, the key seam (`DeviceKeyProvider`) exists to route through Android KeyStore TEE.

2. **Key material never leaves Rust heap.** All signing, key derivation, and cryptographic operations happen exclusively in the Rust memory heap. The WebView/JS side only receives public addresses and signed transaction hex.

3. **IPC encrypted on desktop.** The Tauri Isolation Pattern encrypts all IPC payloads with AES-256-GCM via SubtleCrypto in a sandboxed iframe. The main window never has access to the encryption key. (Brownfield on Android — no WASM isolation iframe, but the Rust process already sandboxes the WebView.)

4. **Auth state machine.** Three-tier auth (Unauthenticated → BiometricUnlocked → HardwareRequired). Auto-lock timer expires after inactivity. Biometric failure lockout after 5 consecutive denials (tested policy).

5. **Extension relay security.** All extension-originated dApp calls are gated through an approval queue. `eth_sendTransaction`, `personal_sign`, `eth_requestAccounts` require explicit user consent before reaching the vault.

6. **Zero non-test unsafe blocks.** The workspace lint `unsafe_code = "warn"` enforces this. All `unsafe` is in test-only code (filesystem setup for account persistence tests).

7. **Supply chain.** All 668 cached crate dependencies are scanned weekly via `cargo audit`. `cargo deny` checks bans, licenses, and sources on every CI run. Known suppressed advisories are Tauri transitive deps only.
