# Testing & Testnet Coins

This page explains how **Gullbúr Enclave**'s testnet support is validated, what
is proven, what is still pending, and — if you want to help — how to send us
testnet coins to finish the job.

> **Short version:** Signing and broadcast are verified on a **local regtest
> node** and the daemon JSON-RPC path is verified **live**. What is **not yet
> proven** is public **testnet relay** — we have not yet received a payout that
> propagates to the public testnet chain. That is the gap we'd like your help
> closing.

---

## Status by chain

| Chain | What's verified | What's pending |
|-------|-----------------|----------------|
| **Litecoin (LTC)** | BIP-84 derivation, legacy P2PKH signing, and broadcast all verified **end-to-end on a local `litecoind` regtest node** (tx confirmed on-chain). | **Public testnet3/testnet4 relay is NOT yet proven.** We have not yet received testnet LTC that lands on the public chain (t3 address `mkenEChN…` shows 0 tx). |
| **Monero (XMR)** | CLSAG signing (unit-tested), and daemon JSON-RPC (fee estimate, broadcast, decoy fetch) verified **live** against a public node on mainnet/stagenet/testnet. | **Balance/history via `monero-wallet-rpc`** needs a public P2P stagenet daemon, which is not reliably available as of 2026-08. So a full funded-sign-broadcast on stagenet is pending. |
| **Bitcoin (BTC)** | SegWit derivation + PSBT signing. **Live testnet4 broadcast CONFIRMED** — derived address `tb1qxfw0jn…` shows 3 confirmed txs, incl. a 1000-sat-fee spend confirmed in block 148333 (txid `e64e895c…`). | Minor — testnet4 relay is proven; no remaining gap. |
| **Ethereum / EVM** | Signing + broadcast verified on **Sepolia** (live transaction accepted, creds `0x6b9b…` VERIFIED). | EVM testnet funding is proven. Only LTC/XMR testnets remain. |

---

## How to send testnet coins (help us finish validation)

If you'd like to help validate the public testnet path, send **testnet-only**
coins to the address below. These are **valueless testnet tokens** — never send
real funds. When a testnet payment arrives and confirms on the public chain, we
can run a full end-to-end broadcast test against it.

### Litecoin testnet3

```
mkenEChN3CvkNr2hKxUkT72phJ5cZwGgXT
```

This is our derived index-0 address (`m/84'/2'/0'/0/0`), legacy P2PKH on
**testnet3**. Most LTC testnet faucets / testnet wallets pay this format.

### Monero stagenet

```
58ns5D78beWTAAiaMZY9TYDQDfjGD49Xt8L4BSJ8rLuTgf1P7BHrLQS9KHipzVdEjM5UCwxQAAUNrKei5RfzV8Kn6Jk8kdP
```

Stagenet only — valueless.

> We will remove these addresses once public testnet validation is complete.

---

## Known limitation (read before assuming anything)

**The Litecoin testnet broadcast path is verified on a LOCAL regtest node, not
yet on the PUBLIC testnet chain.** This means:

- Our **signing and transaction broadcast logic is confirmed correct** (a real
  Litecoin consensus node accepted and confirmed our signed tx).
- What hasn't been demonstrated is that a **public testnet3/testnet4 node will
  relay and confirm a funded spend** end-to-end.

We're being explicit about this so nobody mistakes "regtest-verified" for
"public-testnet-verified." Once we receive testnet LTC/Monero that lands on the
public chain, we'll run the live test and update this page.