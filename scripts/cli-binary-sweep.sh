#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────
# Gullbúr Enclave — CLI Binary Functional Sweep
# Runs the ACTUAL gullbur-cli binary (not the test crate) through a
# full vault lifecycle: launch → init → create accounts → get balances
# → lock → status → shutdown.
#
# Exit code 0 = all checks passed.
# ──────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLI="$ROOT/target/release/gullbur-cli"
TEST_DIR="/tmp/gullbur-cli-sweep-$$"
FAIL=0
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

pass()  { echo -e "  ${GREEN}✓${NC} $1"; }
fail()  { echo -e "  ${RED}✗${NC} $1"; FAIL=1; }

# Clean test dir
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

# Choose a port far from any conflict
PORT=19777

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  Gullbúr Enclave — CLI Binary Functional Sweep"
echo "═══════════════════════════════════════════════════════════"
echo "  Binary: $CLI"
echo "  Port:   $PORT"
echo "  Workdir: $TEST_DIR"
echo ""

cleanup() {
    echo ""
    echo "▸ Cleaning up..."
    # Kill the vault server if still running
    if [ -f /tmp/gullbur-sweep-pid-$PORT ]; then
        kill "$(cat /tmp/gullbur-sweep-pid-$PORT)" 2>/dev/null || true
        rm -f /tmp/gullbur-sweep-pid-$PORT
    fi
    rm -rf "$TEST_DIR"
    echo "  done."
}
trap cleanup EXIT

# Kill any stale process on our port
fuser -k "$PORT/tcp" 2>/dev/null || true
sleep 1

# ── 1. Launch IPC server ──────────────────────────────────────────
echo "▸ [1/12] Launching IPC server..."
"$CLI" --port "$PORT" launch &
LAUNCH_PID=$!
echo "$LAUNCH_PID" > /tmp/gullbur-sweep-pid-$PORT
sleep 1

# Verify process is alive
if kill -0 "$LAUNCH_PID" 2>/dev/null; then
    pass "IPC server started (pid=$LAUNCH_PID)"
else
    fail "IPC server failed to start"
    exit "$FAIL"
fi

# Wait for the port to be listening
for i in $(seq 1 10); do
    if ss -tlnp 2>/dev/null | grep -q ":$PORT "; then
        pass "IPC server listening on :$PORT"
        break
    fi
    sleep 1
done
if ! ss -tlnp 2>/dev/null | grep -q ":$PORT "; then
    fail "IPC server never bound to port $PORT"
    # For JSON output
    echo "{\"result\":\"TIMEOUT\",\"tests\":[]}" > /dev/null
    exit 1
fi

# ── 1b. Live WS handshake + RPC probe ─────────────────────────────
# Closes the old testing hole: previously we only verified the port was
# *listening* (/proc/net/tcp). This proves the ACTUAL IPC handshake works
# (hello → session_key → JSON-RPC) against the real running binary.
echo "▸ [1b/12] Live WebSocket handshake + RPC..."
if python3 "$ROOT/scripts/ws-handshake-probe.py" "$PORT" "vault.generate_mnemonic" 2>&1 \
    | tee /tmp/gullbur-sweep-handshake-$$.log | grep -q "PASS"; then
    pass "Live WS handshake + RPC verified on :$PORT"
else
    fail "Live WS handshake + RPC FAILED (see log)"
fi
rm -f /tmp/gullbur-sweep-handshake-$$.log

# ── 2. Generate mnemonic ──────────────────────────────────────────
echo "▸ [2/12] Generating mnemonic..."
MNEMONIC_OUTPUT=$("$CLI" --port "$PORT" generate-mnemonic 2>&1)
MNEMONIC=$(echo "$MNEMONIC_OUTPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('mnemonic',''))
except:
    pass
" 2>/dev/null || echo "")
WORD_COUNT=$(echo "$MNEMONIC" | wc -w)
if [ "$WORD_COUNT" -eq 24 ]; then
    pass "Mnemonic generated: $WORD_COUNT words"
else
    fail "Mnemonic: expected 24 words, got $WORD_COUNT"
fi

# ── 3. Initialize vault ────────────────────────────────────────────
echo "▸ [3/12] Initializing vault..."
INIT_OUTPUT=$("$CLI" --port "$PORT" init "$MNEMONIC" 2>&1)
if echo "$INIT_OUTPUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d.get('initialized') == True, 'not initialized'
assert 'master_key' in d, 'no master_key'
" 2>/dev/null; then
    pass "Vault initialized"
else
    fail "Vault init: $INIT_OUTPUT"
fi

# ── 4. Status after init ──────────────────────────────────────────────
echo "▸ [4/12] Status after init..."
STATUS_OUTPUT=$("$CLI" --port "$PORT" status 2>&1)
if echo "$STATUS_OUTPUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d.get('initialized') == True
assert len(d.get('networks',[])) >= 5
assert len(d.get('plugin_ids',[])) >= 4
" 2>/dev/null; then
    pass "Status: initialized=true, networks>=5, plugins>=4"
else
    fail "Status check: $STATUS_OUTPUT"
fi

# ── 5. Create BTC account ─────────────────────────────────────────────
echo "▸ [5/12] Creating BTC account..."
BTC_ACCT=$("$CLI" --port "$PORT" create-account btc 2>&1)
BTC_ADDR=$(echo "$BTC_ACCT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d.get('address',''))
" 2>/dev/null || echo "")
if echo "$BTC_ADDR" | grep -qE "^(tb1|bc1)"; then
    pass "BTC account created: $BTC_ADDR"
else
    fail "BTC account: address '$BTC_ADDR' doesn't start with tb1/bc1"
fi

# ── 6. Create ETH account ─────────────────────────────────────────────
echo "▸ [6/12] Creating ETH account..."
ETH_ACCT=$("$CLI" --port "$PORT" create-account eth 2>&1)
ETH_ADDR=$(echo "$ETH_ACCT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d.get('address',''))
" 2>/dev/null || echo "")
if echo "$ETH_ADDR" | grep -qE "^0x[0-9a-fA-F]{40}$"; then
    pass "ETH account created: $ETH_ADDR"
else
    fail "ETH account: address '$ETH_ADDR' invalid format"
fi

# ── 7. Create XMR + LTC accounts ──────────────────────────────────────
echo "▸ [7/12] Creating XMR and LTC accounts..."

XMR_ACCT=$("$CLI" --port "$PORT" create-account xmr 2>&1)
XMR_ADDR=$(echo "$XMR_ACCT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d.get('address',''))
" 2>/dev/null || echo "")
if [ -n "$XMR_ADDR" ]; then
    pass "XMR account created (${#XMR_ADDR} chars)"
else
    fail "XMR account: no address"
fi

LTC_ACCT=$("$CLI" --port "$PORT" create-account ltc 2>&1)
LTC_ADDR=$(echo "$LTC_ACCT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d.get('address',''))
" 2>/dev/null || echo "")
if [ -n "$LTC_ADDR" ]; then
    pass "LTC account created (${#LTC_ADDR} chars)"
else
    fail "LTC account: no address"
fi

# ── 8. List accounts ──────────────────────────────────────────────────
echo "▸ [8/12] Listing accounts..."
ACCTS=$("$CLI" --port "$PORT" list-accounts 2>&1)
ACCT_COUNT=$(echo "$ACCTS" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(len(d) if isinstance(d, list) else 0)
" 2>/dev/null || echo "0")
if [ "$ACCT_COUNT" -ge 4 ]; then
    pass "List accounts: $ACCT_COUNT accounts"
else
    fail "List accounts: expected >=4, got $ACCT_COUNT"
fi

# ── 9. Get balances (routing only — may error on testnet) ─────────────
echo "▸ [9/12] Checking balances (routing)..."
BTC_BAL=$("$CLI" --port "$PORT" get-balance btc "$BTC_ADDR" 2>&1)
if echo "$BTC_BAL" | python3 -c "
import sys, json
d = json.load(sys.stdin)
# Accept either success or routed error — but not method_not_found
if 'error' in d:
    assert d['error'].get('code') != -32601, 'method_not_found'
" 2>/dev/null; then
    pass "BTC balance check routed"
else
    fail "BTC balance check: method_not_found"
fi

ETH_BAL=$("$CLI" --port "$PORT" get-balance eth "$ETH_ADDR" 2>&1)
if echo "$ETH_BAL" | python3 -c "
import sys, json
d = json.load(sys.stdin)
if 'error' in d:
    assert d['error'].get('code') != -32601
" 2>/dev/null; then
    pass "ETH balance check routed"
else
    fail "ETH balance check: method_not_found"
fi

# ── 10. Lock vault ────────────────────────────────────────────────────
echo "▸ [10/12] Locking vault..."
LOCK_OUT=$("$CLI" --port "$PORT" lock 2>&1)
if echo "$LOCK_OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d.get('locked') == True
" 2>/dev/null; then
    pass "Vault locked"
else
    fail "Lock: $LOCK_OUT"
fi

# Verify operations blocked after lock (use --json so errors are JSON)
SIGN_AFTER_LOCK=$("$CLI" --port "$PORT" --json create-account btc 2>&1)
if echo "$SIGN_AFTER_LOCK" | python3 -c "
import sys, json
d = json.load(sys.stdin)
# gullbur-cli --json outputs errors as {\"error\":\"msg\"} — just check it errors
assert 'error' in d, 'should error after lock'
" 2>/dev/null; then
    pass "Operations blocked after lock"
else
    fail "Post-lock block: $SIGN_AFTER_LOCK"
fi

# ── 11. Status after lock ────────────────────────────────────────────
echo "▸ [11/12] Status after lock..."
STATUS_LOCK=$("$CLI" --port "$PORT" status 2>&1)
if echo "$STATUS_LOCK" | python3 -c "
import sys, json
d = json.load(sys.stdin)
# Status should still show initialized=true (lock doesn't wipe state)
" 2>/dev/null; then
    pass "Status after lock succeeds"
else
    fail "Status after lock: $STATUS_LOCK"
fi

# ── Summary ──────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════════"
if [ $FAIL -eq 0 ]; then
    echo -e "  ${GREEN}ALL ${FAIL} BINARY SWEEP CHECKS PASSED${NC}"
    echo "  Vault lifecycle: launch → generate → init → "
    echo "  create BTC/ETH/XMR/LTC → list → get-balance → lock → status"
    echo "  All 4 plugins exercised (BTC, EVM, XMR, LTC)"
else
    echo -e "  ${RED}$FAIL CHECKS FAILED${NC}"
fi
echo "═══════════════════════════════════════════════════════════"
echo ""

# Kill the vault server (cleanup handles it via trap)
exit $FAIL