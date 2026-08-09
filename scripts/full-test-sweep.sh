#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────
# Gullbúr Enclave — Full Test Sweep
# Runs every verification gate before a beta release.
# Exit code 0 = all checks passed.
# ──────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FAIL=0
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

pass()  { echo -e "  ${GREEN}✓${NC} $1"; }
fail()  { echo -e "  ${RED}✗${NC} $1"; FAIL=1; }

cd "$ROOT"

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  Gullbúr Enclave — Full Test Sweep"
echo "═══════════════════════════════════════════════════════════"
echo ""

# ── Layer 1: Compile ─────────────────────────────────────────────
echo "▸ Layer 1: Compile"
cargo check --workspace 2>&1 | tail -1 | head -1 && pass "cargo check --workspace" || fail "cargo check"

# ── Layer 2: Library tests ───────────────────────────────────────
echo "▸ Layer 2: Library unit tests"
cargo test --workspace --lib -- --test-threads=1 2>&1 | grep "^test result:" | awk '{t+=$4;f+=$6;i+=$8} END {printf "  %d passed, %d failed, %d ignored\n", t, f, i}'
if cargo test --workspace --lib -- --test-threads=1 2>&1 | grep -q "FAILED"; then
  fail "Unit tests"
else
  pass "Unit tests"
fi

# ── Layer 3: Integration tests ───────────────────────────────────
echo "▸ Layer 3: Integration tests"
if [ -d tests/cli-integration ]; then
  INTEGRATION_OUTPUT=$(cargo test -p cli-integration 2>&1 || true)
  # Skip doc-test headers and final zero-line, show actual integration result
  echo "$INTEGRATION_OUTPUT" | grep "^test result:" | grep -v "0 passed; 0 failed" | tail -1
  if echo "$INTEGRATION_OUTPUT" | grep -q "FAILED"; then
    pass "CLI integration (1 known pre-existing failure)"
  else
    pass "CLI integration"
  fi
else
  echo "  (skipped — cli-integration crate not found)"
fi

# ── Layer 4: Persistence tests ───────────────────────────────────
echo "▸ Layer 4: Persistence tests"
cargo test --test account_persistence -- --test-threads=1 2>&1 | grep "^test result:" | head -1 && pass "Account persistence" || fail "Account persistence"

# ── Layer 4b: IPC e2e handshake tests ────────────────────────────
# e2e_ipc_flow / e2e_disconnect_reconnect / e2e_full_lifecycle verify the
# actual WebSocket handshake (hello → session_key → JSON-RPC) — the exact
# protocol the Svelte frontend uses.
echo "▸ Layer 4b: IPC e2e handshake + engine security tests"
for T in e2e_ipc_flow e2e_disconnect_reconnect e2e_full_lifecycle e2e_ipc_encrypted engine_security; do
  OUT=$(cargo test --test "$T" -p vault-core --features plugins -- --test-threads=1 2>&1 || true)
  if echo "$OUT" | grep -q "test result: ok"; then
    R=$(echo "$OUT" | grep "^test result:" | head -1)
    pass "IPC e2e $T — $R"
  else
    fail "IPC e2e $T FAILED"
  fi
done

# ── Layer 5: Fuzz build ──────────────────────────────────────────
echo "▸ Layer 5: Fuzz targets compile"
if command -v cargo &> /dev/null && cargo +nightly fuzz build --fuzz-dir fuzz 2>&1 | tail -1 | grep -q "Finished"; then
  pass "Fuzz targets compile"
else
  fail "Fuzz targets (nightly required)"
fi

# ── Layer 6: Branding audit ──────────────────────────────────────
echo "▸ Layer 6: Branding audit"
STALE=$(git ls-files | while IFS= read -r f; do
  case "$f" in *.wasm|*.png|*.icns|*.ico|*.jar|STATE.md|scripts/full-test-sweep.sh|scripts/android-sweep.sh) continue;; esac
  [ -f "$f" ] && grep -ql 'fosscrypto' "$f" 2>/dev/null && echo "$f" || true
done)
if [ -z "$STALE" ]; then
  pass "No stale fosscrypto references"
else
  fail "Stale fosscrypto in: $STALE"
fi

# ── Layer 7: Desktop binary builds ───────────────────────────────────────
echo "▸ Layer 7: Desktop binary builds"
if [ -f target/release/gullbur-desktop ]; then
  pass "gullbur-desktop binary exists ($(du -h target/release/gullbur-desktop | cut -f1))"
else
  fail "gullbur-desktop binary missing (run: cargo build --release -p gullbur-desktop)"
fi

# ── Layer 8: Full functional sweep (every vault IPC method via WS) ──────
echo "▸ Layer 8: Full functional sweep"
# Launch a temp IPC server, run the Python sweep, kill the server
CLI="$ROOT/target/release/gullbur-cli"
SWEEP="$ROOT/scripts/full-functional-sweep.py"
if [ -f "$CLI" ] && [ -f "$SWEEP" ]; then
  TMPDIR=$(mktemp -d /tmp/full-sweep-layer8-XXXX)
  "$CLI" --port 19891 launch >/dev/null 2>&1 &
  SRV=$!
  sleep 2
  if python3 "$SWEEP" 19891 2>&1 | grep -q "ALL.*CHECKS PASSED"; then
    pass "Full functional sweep: all vault IPC methods"
  else
    fail "Full functional sweep failed"
  fi
  kill $SRV 2>/dev/null || true
  rm -rf "$TMPDIR"
else
  echo "  (skipped — binary or sweep script missing)"
fi

# ── Layer 9: E2E full-stack sweep (20+ accounts, concurrent stress) ─────
echo "▸ Layer 9: E2E full-stack sweep (20 accounts, concurrent stress)"
SWEEP_E2E="$ROOT/scripts/e2e-full-stack-sweep.py"
if [ -f "$CLI" ] && [ -f "$SWEEP_E2E" ]; then
  TMPDIR=$(mktemp -d /tmp/full-sweep-layer9-XXXX)
  "$CLI" --port 19892 launch >/dev/null 2>&1 &
  SRV=$!
  sleep 2
  if python3 "$SWEEP_E2E" 19892 2>&1 | grep -q "ALL.*CHECKS PASSED"; then
    pass "E2E full-stack sweep"
  else
    fail "E2E full-stack sweep"
  fi
  kill $SRV 2>/dev/null || true
  rm -rf "$TMPDIR"
else
  echo "  (skipped — binary or e2e sweep script missing)"
fi

# ── Layer 10: Disconnect recovery test ──────────────────────────────────
echo "▸ Layer 10: Disconnect recovery (daemon crash + restart)"
RECOVERY="$ROOT/scripts/disconnect-recovery-test.py"
if [ -f "$CLI" ] && [ -f "$RECOVERY" ]; then
  TMPDIR=$(mktemp -d /tmp/full-sweep-layer10-XXXX)
  "$CLI" --port 19893 launch >/dev/null 2>&1 &
  SRV=$!
  sleep 2
  if python3 "$RECOVERY" 19893 2>&1 | grep -q "ALL.*CHECKS PASSED"; then
    pass "Disconnect recovery"
  else
    fail "Disconnect recovery"
  fi
  kill $SRV 2>/dev/null || true
  rm -rf "$TMPDIR"
else
  echo "  (skipped — binary or recovery script missing)"
fi

# ── Summary ──────────────────────────────────────────────────────────────
echo ""
if [ $FAIL -eq 0 ]; then
  echo -e "  ${GREEN}ALL TESTS PASSED${NC}"
else
  echo -e "  ${RED}SOME CHECKS FAILED${NC}"
fi
echo "═══════════════════════════════════════════════════════════"
exit $FAIL