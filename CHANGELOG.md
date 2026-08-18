# Changelog

All notable changes to **Gullbúr Enclave** (desktop app version `0.0.2` internal).

Format based on [Keep a Changelog](https://keepachangelog.com/).
Versioning: **0.0.xxx** = internal pre-beta snapshots (all prior 0.1.x releases are
reclassified internal). The public release, once network paths are proven, will be
**0.1.0-beta** → 1.0.

---

## [0.0.2] — 2026-08-17 (internal)

> Internal pre-beta snapshot. Not published. Reclassifies all earlier dev
> releases as internal; public cadence reserved for 0.1.0-beta onward.

### Added
- **Litecoin testnet3/testnet4 split** — `litecoin-testnet` now exposes explicit
  `litecoin-testnet3` and `litecoin-testnet4` network specs (plus a backward-
  compatible alias). Testnet3 → `/testnet/api` (Esplora), testnet4 →
  `/testnet4/api`. Both derive the same legacy P2PKH `m/n` address.
- **LTC regtest E2E test** — proves sign → broadcast → confirm on a local
  `litecoind` regtest node (real consensus validation, txid `56992f8c…`).
- **LTC live-testnet3 broadcast test** — ignored-by-default test for the public
  testnet chain.
- **XMR configurable daemon endpoints + failover** — per-network ordered list of
  daemon JSON-RPC URLs (defaults to verified live `node.monerodevs.org`), with
  automatic failover on the next endpoint when one is unreachable. Applies to
  broadcast, fee estimation, and decoy selection.

### Fixed
- LTC address helpers now accept the split testnet3/testnet4 network ids (they
  previously only matched the old `litecoin-testnet` alias).

### Changed
- `cargo fmt` applied across the workspace; `STATE.md` refreshed to current HEAD.
- Documented that `monero-wallet-rpc` needs a **P2P** daemon (not JSON-RPC), and
  that public stagenet/testnet P2P nodes are not reliably available (2026-08);
  mainnet P2P verified live. Override via `with_daemon` or a local
  `monerod --stagenet`.

### Known limitations (honest status)
- **LTC public testnet relay is NOT yet proven.** Signing + broadcast are
  verified on a **local regtest** node; we have not yet received a testnet LTC
  payout that lands on the public testnet3/4 chain. Community testnet sends to
  the address in `TESTING.md` will help close this.
- **XMR stagenet balance/history** requires a P2P stagenet daemon; verification
  via a local pruned `monerod --stagenet` is in progress.
- CypherFaucet tLTC does **not** propagate to the public testnet chain (its
  explorer is a `.onion`); avoid it for public testing.

### Security / licensing
- Source-available under BSL-1.1 during beta; converts to Apache-2.0 at the
  Change Date (target v1.0). See `LICENSE`.

---

## [0.1.0-beta.1] — 2026-08-13 (public staging)

Initial staging build. Core crypto across Bitcoin, Ethereum, Monero, Litecoin.
E2EE IPC, WASM key isolation, Argon2id encrypted seed/accounts, biometric +
FIDO2 seams, Tor SOCKS5, accent-theme UI. 314+ workspace unit tests, 248
frontend Svelte tests, full 13-layer sweep, Linux AppImage/.deb + Android
APK/AAB.

See `STATE.md` for the detailed feature/batch history.

---

<!-- Release compare template (fill in at first public release):
[0.1.0-beta.2]: https://github.com/MarcusBrammeier/gullbur-enclave/compare/v0.1.0-beta.1...v0.1.0-beta.2
[0.1.0-beta.1]: https://github.com/MarcusBrammeier/gullbur-enclave/releases/tag/v0.1.0-beta.1
-->