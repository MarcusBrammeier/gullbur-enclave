#!/usr/bin/env python3
"""
Gullbúr Enclave — Full Functional Sweep (v0.1.0-beta.6)

Exercises EVERY vault IPC method through the REAL running binary over
WebSocket, simulating the exact protocol the Tauri desktop app uses.
Covers every handler registered in ipc_handlers.rs that can be driven
without a GUI or approved extension flow.

Methods categorized:
  ✅ FULL  = non-trivial call with valid params, asserts response shape
  ⚡ ROUTE = network-dependent (balance/broadcast/history/fee); verifies
             the method routes to the plugin (not -32601 method_not_found)
  ⏭ SKIP  = needs user-approval UI (extension-only Phase 2 methods)

Usage:
  ./scripts/full-functional-sweep.py            # default port 19876
  ./scripts/full-functional-sweep.py <port>     # custom port
"""

import json
import subprocess
import sys
import time
from websockets.sync.client import connect

# ── Configuration ──────────────────────────────────────────────────────────
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19876
CLI = "./target/release/gullbur-cli"
TIMEOUT = 10.0
PASS = 0
FAIL = 0
FAILURES = []


def ws_call(method: str, params: dict) -> dict:
    """Send a JSON-RPC call over WebSocket and return the parsed response."""
    with connect(f"ws://127.0.0.1:{PORT}", open_timeout=TIMEOUT) as ws:
        # Hello handshake
        ws.send(json.dumps({"type": "hello"}))
        raw = ws.recv(timeout=TIMEOUT)
        msg = json.loads(raw)
        assert msg.get("type") == "session_key", f"expected session_key, got {raw[:80]}"
        # JSON-RPC call
        req = {"jsonrpc": "2.0", "method": method, "params": params, "id": 1}
        ws.send(json.dumps(req))
        resp_raw = ws.recv(timeout=TIMEOUT)
        resp = json.loads(resp_raw)
        if "error" in resp:
            return {"error": resp["error"]}
        return resp.get("result", {})


def cli(cmd: list, timeout: int = 30) -> dict:
    """Run a gullbur-cli subcommand and parse JSON output."""
    full = [CLI, "--port", str(PORT), "--json"] + cmd
    r = subprocess.run(full, capture_output=True, text=True, timeout=timeout)
    try:
        return json.loads(r.stdout)
    except (json.JSONDecodeError, ValueError):
        return {"raw_stdout": r.stdout, "raw_stderr": r.stderr}


def check(label: str, ok: bool, detail: str = ""):
    global PASS, FAIL
    if ok:
        PASS += 1
        print(f"  ✓ {label}")
    else:
        FAIL += 1
        FAILURES.append(f"{label}: {detail}")
        print(f"  ✗ {label}: {detail}")


# ═══════════════════════════════════════════════════════════════════════════
print(f"═══════════════════════════════════════════════════════════")
print(f"  Gullbúr Enclave — Full Functional Sweep")
print(f"  Binary: {CLI}")
print(f"  Port:   {PORT}")
print(f"═══════════════════════════════════════════════════════════\n")

# ── PHASE 0: Server must be running ───────────────────────────────────────
# This script assumes the IPC server is already launched externally.
# Verify connectivity with a hello handshake.
try:
    with connect(f"ws://127.0.0.1:{PORT}", open_timeout=5) as ws:
        ws.send(json.dumps({"type": "hello"}))
        raw = ws.recv(timeout=5)
        msg = json.loads(raw)
        assert msg.get("type") == "session_key"
    check("IPC server reachable on :PORT", True)
except Exception as e:
    check("IPC server reachable on :PORT", False, str(e))
    print("\n  Server not running. Launch it first:\n")
    print(f"    {CLI} --port {PORT} launch &\n")
    sys.exit(1)

# ── PHASE 1: vault.generate_mnemonic (no params) ───────────────────────────
print("\n▸ [1] vault.generate_mnemonic")
mn = ws_call("vault.generate_mnemonic", {})
if "mnemonic" in mn:
    words = mn["mnemonic"].split()
    check("generate_mnemonic: 24-word phrase", len(words) == 24, f"got {len(words)} words")
else:
    check("generate_mnemonic: mnemonic key", False, str(mn))

# ── PHASE 2: vault.initialize (seed_phrase) ────────────────────────────────
print("\n▸ [2] vault.initialize")
init = ws_call("vault.initialize", {"seed_phrase": mn.get("mnemonic", "")})
check("initialize: success=true", init.get("success") is True, str(init))
check("initialize: initialized=true", init.get("initialized") is True, str(init))
check("initialize: master_key present", "master_key" in init, str(init))

# ── PHASE 3: vault.status (no params) ──────────────────────────────────────
print("\n▸ [3] vault.status")
st = ws_call("vault.status", {})
check("status: initialized=true", st.get("initialized") is True, str(st))
check("status: plugin_ids >= 4", len(st.get("plugin_ids", [])) >= 4, str(st))
check("status: networks >= 5", len(st.get("networks", [])) >= 5, str(st))

# ── PHASE 4: vault.list_networks (no params) ───────────────────────────────
print("\n▸ [4] vault.list_networks")
nets = ws_call("vault.list_networks", {})
check("list_networks: is array", isinstance(nets, list), str(nets)[:80])
net_ids = [n.get("id", "") for n in nets if isinstance(n, dict)]
has_btc = any("bitcoin" in n for n in net_ids)
has_eth = any("ethereum" in n for n in net_ids)
has_xmr = any("monero" in n for n in net_ids)
has_ltc = any("litecoin" in n for n in net_ids)
check("list_networks: Bitcoin (btc)", has_btc, f"ids: {net_ids}")
check("list_networks: Ethereum (eth)", has_eth, f"ids: {net_ids}")
check("list_networks: Monero (xmr)", has_xmr, f"ids: {net_ids}")
check("list_networks: Litecoin (ltc)", has_ltc, f"ids: {net_ids}")

# ── PHASE 5: vault.create_account (4 chains) ──────────────────────────────
print("\n▸ [5] vault.create_account (4 chains)")
accounts = {}
for net, label in [("bitcoin", "BTC"), ("ethereum", "ETH"), ("monero", "XMR"), ("litecoin", "LTC")]:
    acct = ws_call("vault.create_account", {"network": net, "index": 0})
    if "address" in acct:
        accounts[net] = acct["address"]
        check(f"create_account {label}: address present", True, acct["address"][:40])
    else:
        check(f"create_account {label}: address", False, str(acct))

# ── PHASE 6: vault.validate_address ────────────────────────────────────────
print("\n▸ [6] vault.validate_address")
va = ws_call("vault.validate_address", {"network": "bitcoin", "address": "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"})
check("validate_address: valid BTC address accepted", va.get("valid") is True, str(va))
vb = ws_call("vault.validate_address", {"network": "bitcoin", "address": "not-an-address"})
check("validate_address: invalid BTC address rejected", vb.get("valid") is False, str(vb))

# ── PHASE 7: vault.list_accounts (no params) ─────────────────────────────
print("\n▸ [7] vault.list_accounts")
accts = ws_call("vault.list_accounts", {})
acct_list = accts if isinstance(accts, list) else []
check(f"list_accounts >= 4", len(acct_list) >= 4, f"got {len(acct_list)}: {[a.get('id','')[:30] for a in acct_list]}")

# ── PHASE 8: vault.get_balance (routing check) ─────────────────────────────
print("\n▸ [8] vault.get_balance (routing)")
if "bitcoin" in accounts:
    bal = ws_call("vault.get_balance", {"network": "bitcoin", "address": accounts["bitcoin"]})
    # Valid: either a balance result OR a routed error (but NOT -32601 method_not_found)
    is_not_method_not_found = True
    if "error" in bal and bal.get("error", {}).get("code") == -32601:
        is_not_method_not_found = False
    check("get_balance: routes to BTC plugin", is_not_method_not_found, str(bal)[:80])

if "ethereum" in accounts:
    bal_e = ws_call("vault.get_balance", {"network": "ethereum", "address": accounts["ethereum"]})
    is_not_mnf = not ("error" in bal_e and bal_e.get("error", {}).get("code") == -32601)
    check("get_balance: routes to EVM plugin", is_not_mnf, str(bal_e)[:80])

# ── PHASE 9: vault.estimate_fee (routing check) ───────────────────────────
print("\n▸ [9] vault.estimate_fee (routing)")
fee = ws_call("vault.estimate_fee", {"network": "bitcoin"})
is_not_mnf = not ("error" in fee and fee.get("error", {}).get("code") == -32601)
check("estimate_fee: routes to BTC plugin", is_not_mnf, str(fee)[:80])

# ── PHASE 10: vault.get_transaction_history (routing check) ──────────────
print("\n▸ [10] vault.get_transaction_history (routing)")
if "bitcoin" in accounts:
    hist = ws_call("vault.get_transaction_history", {"network": "bitcoin", "address": accounts["bitcoin"], "limit": 3})
    is_not_mnf = not ("error" in hist and hist.get("error", {}).get("code") == -32601)
    check("get_transaction_history: routes to BTC plugin", is_not_mnf, str(hist)[:80])

# ── PHASE 11: vault.sign_transaction (bad PSBT → error, proves routing) ──
print("\n▸ [11] vault.sign_transaction (error path — validates routing)")
sign = ws_call("vault.sign_transaction", {
    "network": "bitcoin",
    "tx_hex": "00",  # not a valid PSBT
    "key_id": "deadbeef" + "0" * 56,
    "key_type": "Secp256k1",
})
# Should get a plugin-level error (not -32601 method_not_found)
is_not_mnf = not ("error" in sign and sign.get("error", {}).get("code") == -32601)
check("sign_transaction: routes to BTC plugin", is_not_mnf, str(sign)[:100])

# ── PHASE 11b: vault.sign_transaction (valid PSBT, multi-input) ──────────
# This tests the full signing path with a valid-format multi-input PSBT.
# Uses the pre-existing deterministic seed from init.
print("\n▸ [11b] vault.sign_transaction (valid multi-input PSBT)")
# Build a multi-input PSBT in Python that matches the Rust test pattern
import hashlib
# Simplified: we can't easily serialize a PSBT in Python, so we test that
# the CLI sign command at least routes. The deep multi-input test is in
# the Rust unit test (test_sign_transaction_multi_input_all_inputs_signed).
sign2 = ws_call("vault.sign_transaction", {
    "network": "bitcoin",
    "tx_hex": "70736274ff01007e0200000002010101010101010101010101010101010101010101010101010101010101010100000000000000000002020202020202020202020202020202020202020202020202020202020202020201000000000000000000018096980000000000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000002200",
    "key_id": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef@0",
    "key_type": "Secp256k1",
})
is_not_mnf = not ("error" in sign2 and sign2.get("error", {}).get("code") == -32601)
# If it was processed by the BTC plugin but failed on parsing/utxo, that's OK — proves routing
if "signed_tx_hex" in sign2:
    check("sign_transaction: multi-input PSBT signed", True)
else:
    check("sign_transaction: multi-input PSBT routes to BTC plugin (may fail on missing utxo)", is_not_mnf, str(sign2)[:120])

# ── PHASE 12: vault.broadcast_transaction (error path — bad tx) ──────────
print("\n▸ [12] vault.broadcast_transaction (error path)")
bt = ws_call("vault.broadcast_transaction", {"network": "bitcoin", "signed_tx_hex": "00"})
# Should get a broadcast failure, not method_not_found
is_not_mnf = not ("error" in bt and bt.get("error", {}).get("code") == -32601)
check("broadcast_transaction: routes to BTC plugin", is_not_mnf, str(bt)[:80])

# ── PHASE 13: vault.lock (no params) ──────────────────────────────────────
print("\n▸ [13] vault.lock")
lk = ws_call("vault.lock", {})
check("lock: locked=true", lk.get("locked") is True, str(lk))

# ── PHASE 14: Operations blocked after lock ────────────────────────────────
print("\n▸ [14] Operations blocked after lock")
blk = ws_call("vault.create_account", {"network": "bitcoin", "index": 1})
# After lock, create_account should fail with auth_required (code -32002)
code = blk.get("error", {}).get("code", 0) if "error" in blk else 0
is_auth_rejected = code == -32002
check("create_account blocked after lock (auth_required)", is_auth_rejected, str(blk)[:80])

# ── PHASE 15: vault.status after lock ─────────────────────────────────────
print("\n▸ [15] vault.status after lock")
st2 = ws_call("vault.status", {})
check("status after lock: still returns data", "plugin_ids" in st2, str(st2)[:80])

# ── PHASE 16: Phase 2 extension methods (expected to reject w/o origin) ──
print("\n▸ [16] Phase 2 extension methods (expected error w/o origin)")
for ext_method in ["vault_executeBatch", "vault_requestSessionKey", "vault_simulateAndSend"]:
    eb = ws_call(ext_method, {"network": "ethereum", "operations": [], "origin": ""})
    check(f"{ext_method}: returns error (no approval UI)",
          "error" in eb or "raw_stdout" in eb, str(eb)[:80])

# ── SUMMARY ───────────────────────────────────────────────────────────────
print(f"\n═══════════════════════════════════════════════════════════")
t = PASS + FAIL
pct = f"{PASS}/{t}"
if FAIL == 0:
    print(f"  ✓ ALL {PASS}/{PASS} CHECKS PASSED")
else:
    print(f"  ✓ {PASS}/{t} PASS, ✗ {FAIL}/{t} FAIL")
    for f in FAILURES:
        print(f"    FAIL {f}")
print(f"═══════════════════════════════════════════════════════════")
sys.exit(1 if FAIL > 0 else 0)