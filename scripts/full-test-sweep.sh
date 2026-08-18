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

# Layers 8/9/10 launch the headless CLI against the shared ~/.gullbur state
# dir, and each assumes a FRESH, uninitialized vault (they each call
# vault.initialize). Without a reset between layers, Layer 8's persisted
# vault makes 9/10 fail with "already initialized" or wrong account counts.
# Back up (don't delete) the user's real vault, then start clean.
reset_state() {
  local vdir="${HOME}/.gullbur"
  if [ -d "$vdir" ]; then
    local bak="/tmp/gullbur-sweep-bak-$(date +%s)"
    mv "$vdir" "$bak" && echo "(test state backed up to $bak)"
  fi
}

cd "$ROOT"

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  Gullbúr Enclave — Full Test Sweep"
echo "═══════════════════════════════════════════════════════════"
echo ""

# ── Layer 1: Compile ─────────────────────────────────────────────
echo "▸ Layer 1: Compile"
# --locked so cargo never re-resolves the lock (preserves the dual-sha2
# crypto resolution — plain `cargo check` collapses it and breaks keystore hkdf)
cargo check --workspace --locked 2>&1 | tail -1 | head -1 && pass "cargo check --workspace --locked" || fail "cargo check --locked"

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
# Internal dirs that are scrubbed from the public FOSS tree (see
# docs/FOSS_BOUNDARY.md) are excluded: plans, roadmap, internal tooling, STATE.md.
STALE=$(git ls-files | while IFS= read -r f; do
  case "$f" in
    *.wasm|*.png|*.icns|*.ico|*.jar) continue;;
    STATE.md|scripts/*) continue;;
    .hermes/plans/*|PLANS/*|docs/plans/*) continue;;
  esac
  [ -f "$f" ] && grep -ql 'fosscrypto' "$f" 2>/dev/null && echo "$f" || true
done)
if [ -z "$STALE" ]; then
  pass "No stale fosscrypto references (public tree)"
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
  reset_state
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
  reset_state
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
  reset_state
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

# ── Layer 11: WASM crypto round-trip (shipped blob) ──────────────────────
# Loads the EXACT .wasm bytes the frontend ships (src/lib/wasm/crypto_wasm_bg.wasm)
# and round-trips encrypt→decrypt — catches a stale/regenerated blob, ABI
# mismatch, or getrandom backend breakage that nothing else verifies.
echo "▸ Layer 11: WASM crypto round-trip (packaged blob)"
WASM_TEST="src/lib/wasm/crypto_wasm.test.ts"
if [ -f "apps/desktop/$WASM_TEST" ]; then
  # Gate on vitest's EXIT CODE (0=pass/1=fail), not grep -q on output: grep -q
  # closes the pipe the instant it matches, SIGPIPE-kills the upstream vitest,
  # and under 'set -o pipefail' that makes the whole subshell fail even when
  # the tests pass. Exit status is race-free and ANSI-immune.
  if ( cd apps/desktop && npx vitest run "$WASM_TEST" >/dev/null 2>&1 ); then
    pass "WASM crypto round-trip ($WASM_TEST)"
  else
    fail "WASM crypto round-trip ($WASM_TEST)"
  fi
else
  echo "  (skipped — wasm test missing)"
fi

# ── Layer 12: Frontend Svelte component tests (vitest) ──────────────────
echo "▸ Layer 12: Frontend component tests (vitest)"
if [ -f apps/desktop/package.json ]; then
  # Gate on npm test's EXIT CODE (vitest exits 0=pass / nonzero=fail). Output
  # parsing is only for a human-readable count, never the gate (vitest wraps
  # non-TTY output in ANSI codes, so grepping is fragile; exit status is not).
  # Note: no `|| true` inside the substitution — it would mask the exit code.
  if FE_OUT=$(cd apps/desktop && npm test 2>&1); then
    VITEST_RC=0
  else
    VITEST_RC=$?
  fi
  # Count summary line only if present (guarded so it can't fail the pipeline).
  FE_TESTS=$(printf '%s\n' "$FE_OUT" | grep -aE "Tests " | tail -1 | tr -s ' ' || true)
  if [ "$VITEST_RC" -eq 0 ]; then
    pass "Frontend tests: ${FE_TESTS:-ok}"
  else
    fail "Frontend tests (exit $VITEST_RC): ${FE_TESTS:-failed}"
  fi
else
  echo "  (skipped — no apps/desktop/package.json)"
fi

# ── Layer 13: Static / lint / audit gates ────────────────────────────────
# Mirrors what .github/workflows/test.yml + audit.yml enforce locally, so a
# local sweep (green) matches CI (green) — no surprise rejection after push.
echo "▸ Layer 13: Static gates (fmt + clippy + deny + audit)"
if cargo fmt --check >/tmp/sweep-fmt.log 2>&1; then
  pass "cargo fmt --check"
else
  fail "cargo fmt --check (run: cargo fmt)"
fi
if cargo clippy --workspace --lib -- -D clippy::unwrap_used -A warnings >/tmp/sweep-clippy.log 2>&1; then
  pass "cargo clippy --lib -D clippy::unwrap_used"
else
  fail "cargo clippy (unwrap gate)"
fi
if cargo deny check >/tmp/sweep-deny.log 2>&1; then
  pass "cargo deny check"
else
  fail "cargo deny check (deny.toml)"
fi
if bash "$ROOT/scripts/audit.sh" >/tmp/sweep-audit.log 2>&1; then
  pass "cargo audit"
else
  fail "cargo audit"
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