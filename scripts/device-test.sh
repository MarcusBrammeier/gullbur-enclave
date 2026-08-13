#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────
# Gullbúr Enclave — Device / Emulator Test Runner (`device-test.sh`)
#
# Orchestrates the test suite across adb-connected Android targets (real
# device or emulator). Generates a single consolidated report.
#
# TEST SPLIT (honest attribution):
#   ON-DEVICE  (through adb-forward, hits the vault's real IPC server on
#               the hardware at ws://127.0.0.1:<hostport> → device:19876):
#     - ws-handshake-probe.py        real WS hello→session_key→RPC probe
#     - full-functional-sweep.py     33/33 checks, all IPC methods
#   HOST-SIDE  (correctly host processes; labeled separately in report):
#     - cargo test --workspace --lib  native Rust unit/engine suite
#     - e2e-full-stack-sweep.py       20-account concurrent stress
#     - disconnect-recovery-test.py   daemon crash→restart→reconnect
#   NOT CLAIMED AS ON-DEVICE: frontend vitest + WASM round-trip are
#   Node/jsdom tests and cannot meaningfully run under Android.
#
# Usage:
#   ./scripts/device-test.sh [--apk <path>] [--no-cargo] [--report-dir <dir>]
#
# Exit 0 = all selected tests passed; nonzero = at least one failed.
# ──────────────────────────────────────────────────────────────────────
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

APK="${APK:-}"
REPORT_DIR="${REPORT_DIR:-./test-reports}"
RUN_CARGO=1
APK_SRC="${APK_SRC:-apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk}"
PKG="com.gullbur.enclave"
ACTIVITY="com.gullbur.enclave/.MainActivity"
DEV_IPC=19876         # on-device vault IPC port (fixed)
HOST_FWD=19876        # host-side adb-forward port (same is fine)
BOOT_TIMEOUT=180

# ── arg parsing ──────────────────────────────────────────────────────
while [ $# -gt 0 ]; do
  case "$1" in
    --apk)        APK="$2"; shift 2;;
    --no-cargo)   RUN_CARGO=0; shift;;
    --report-dir) REPORT_DIR="$2"; shift 2;;
    -h|--help)
      # Print only the leading "#"-comment header (usage block).
      awk 'NR==1{next} /^# ───/{c++; if(c>1) exit} /^#!/{next} {sub(/^# ?/,""); print}' "$0"
      exit 0;;
    *) echo "Unknown arg: $1" >&2; exit 2;;
  esac
done

mkdir -p "$REPORT_DIR"
REPORT="$REPORT_DIR/test-report-$(date +%Y%m%d-%H%M%S).md"
PASS=0; FAIL=0
GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; NC='\033[0m'
note() { echo -e "${YELLOW}▸${NC} $*"; }
ok()   { echo -e "  ${GREEN}✓${NC} $*"; PASS=$((PASS+1)); }
bad()  { echo -e "  ${RED}✗${NC} $*"; FAIL=$((FAIL+1)); }
warn() { echo -e "${YELLOW}⚠${NC} $*"; }
rc()   { local label="$1" rc=$2; [ "$rc" -eq 0 ] && ok "$label" || bad "$label"; }

{
  echo "# Gullbúr Enclave — Test Report"
  echo ""
  echo "- **Generated:** $(date -Iseconds)"
  echo "- **Target:** $(adb -d get-state 2>/dev/null || echo 'n/a')"
  echo "- **APK:** ${APK:-$APK_SRC}"
} > "$REPORT"

adb_section() {
  local title="$1" inner_rc; shift
  {
    echo ""
    echo "## $title"
    echo ""
    echo '```'
    "$@"
    inner_rc=$?
    echo '```'
  } 2>&1 >> "$REPORT"
  # Propagate the inner command's exit status. The trailing '```' echo would
  # otherwise mask it as 0, making every section look like a pass.
  return $inner_rc
}

# ── 1. discover device ───────────────────────────────────────────────
note "Detecting adb device..."
adb start-server >/dev/null 2>&1
DEVICE_ID=$(adb devices | awk 'NR>1 && $2=="device"{print $1; exit}')
if [ -z "$DEVICE_ID" ]; then
  echo -e "${RED}✗ No adb device found. Connect a real device (USB debugging) or boot the emulator.${NC}" >&2
  echo "  Check: adb devices" >&2
  echo "  ## Device Discovery" >> "$REPORT"; echo "" >> "$REPORT"
  echo "FAILED — no adb device connected" >> "$REPORT"
  exit 3
fi
ok "Device connected: $DEVICE_ID ($(adb -s "$DEVICE_ID" shell getprop ro.product.model 2>/dev/null | tr -d '\r'))"

# Wait for boot to complete
note "Waiting for device boot..."
for i in $(seq 1 $((BOOT_TIMEOUT/3))); do
  B=$(adb -s "$DEVICE_ID" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')
  [ "$B" = "1" ] && { ok "Device booted"; break; }
  sleep 3
  [ "$i" = "$((BOOT_TIMEOUT/3))" ] && bad "Device did not finish booting in ${BOOT_TIMEOUT}s (sys.boot_completed)"
done

# ── 2. install APK ───────────────────────────────────────────────────
PKG_PATH="${APK:-$APK_SRC}"
if [ ! -f "$PKG_PATH" ]; then
  bad "APK not found: $PKG_PATH (build first, or pass --apk)"
  exit 4
fi
note "Installing APK: $PKG_PATH"
if adb -s "$DEVICE_ID" install -r "$PKG_PATH" >/dev/null 2>&1; then
  ok "APK installed"
else
  bad "APK install failed"
fi

# ── 3. launch app (auto-starts on-device IPC server on 19876) ────────
note "Launching $ACTIVITY..."
adb -s "$DEVICE_ID" shell am start -n "$ACTIVITY" >/dev/null 2>&1
sleep 3

DEV_ALIVE=$(adb -s "$DEVICE_ID" shell ps 2>/dev/null | grep -c "com.gullbur.enclave")
if [ "$DEV_ALIVE" -gt 0 ]; then
  ok "App process running on device"
else
  bad "App process not detected after launch — check the app didn't crash"
fi

# Verify on-device IPC server listening (port 19876)
note "Checking on-device IPC server (port $DEV_IPC)..."
IPC_OK=0
for i in $(seq 1 20); do
  # /proc/net/tcp: local-address column "<ip>:<HEXPORT>" (19876 = 4DA4);
  # state 0A = LISTEN. Grep for the LISTEN socket on 4DA4.
  if adb -s "$DEVICE_ID" shell cat /proc/net/tcp 2>/dev/null | grep -E "4DA4.*0A" ; then
    IPC_OK=1; break
  fi
  sleep 1
done
if [ "$IPC_OK" -eq 1 ]; then
  ok "On-device IPC server listening on 19876"
else
  # Persistent emulators retain app state but the IPC server may not have
  # (re)bound after a relaunch. Force-stop + relaunch once and re-check before
  # declaring failure — avoids flaky false-negatives from device teardown.
  warn "On-device IPC server not detected — relaunching app once..."
  adb -s "$DEVICE_ID" shell am force-stop "$PKG" >/dev/null 2>&1
  sleep 2
  adb -s "$DEVICE_ID" shell am start -n "$ACTIVITY" >/dev/null 2>&1
  IPC_OK=0
  for i in $(seq 1 25); do
    if adb -s "$DEVICE_ID" shell cat /proc/net/tcp 2>/dev/null | grep -E "4DA4.*0A" ; then
      IPC_OK=1; break
    fi
    sleep 1
  done
  if [ "$IPC_OK" -eq 1 ]; then
    ok "On-device IPC server listening after relaunch"
  else
    bad "On-device IPC server not detected on 19876 (TCP listen) after relaunch"
  fi
fi
# Warn-and-continue if not listening (the sweeps will fail with a clear WS
# error and the report reflects it) — but still set up forward and run.

# ── 4. adb-forward so ws://127.0.0.1:19876 → device:19876 ───────────
note "Setting up adb forward tcp:$HOST_FWD -> tcp:$DEV_IPC..."
adb -s "$DEVICE_ID" forward tcp:$HOST_FWD tcp:$DEV_IPC >/dev/null 2>&1
FWD=$(adb -s "$DEVICE_ID" forward --list 2>/dev/null | grep -c "$HOST_FWD")
if [ "$FWD" -ge 1 ]; then
  ok "adb forward active (localhost:$HOST_FWD -> device:$DEV_IPC)"
else
  bad "adb forward not established"
fi

# ── 5. ON-DEVICE sweeps (connect to forwarded on-device server) ──────
adb_section "On-Device: WebSocket Handshake Probe" \
  python3 scripts/ws-handshake-probe.py "$HOST_FWD"
rc "ws-handshake-probe (on-device)" $?

adb_section "On-Device: Full Functional Sweep (all IPC methods)" \
  python3 scripts/full-functional-sweep.py "$HOST_FWD"
rc "full-functional-sweep (on-device)" $?

# ── 6. HOST-SIDE suites ─────────────────────────────────────────────
if [ "$RUN_CARGO" -eq 1 ]; then
  note "Running host Rust suite (cargo test --workspace --lib)..."
  # Capture exit code WITHOUT `|| true` masking it (that would report success
  # on a failing suite).
  if CARGO_OUT=$(cargo test --workspace --lib -- --test-threads=1 2>&1); then
    CARGO_RUN=0
  else
    CARGO_RUN=$?
  fi
  adb_section "Host: cargo test --workspace --lib" printf '%s\n' "$CARGO_OUT"
  rc "cargo test --workspace --lib (host)" $CARGO_RUN
fi

# e2e-full-stack-sweep + disconnect-recovery launch/connect to a host vault and
# require the host CLI server to already be running (they assume one exists).
# Each gets its OWN fresh server instance: e2e leaves its server initialized+
# locked, which would pollute the disconnect test (which needs a clean vault).
HOST_CLI="$ROOT/target/release/gullbur-cli"

# --- E2E: fresh server on 19991 ---
if [ -x "$HOST_CLI" ]; then
  TMPDIR_E2E=$(mktemp -d /tmp/device-test-e2e-XXXX)
  "$HOST_CLI" --port 19991 launch >/dev/null 2>&1 &
  E2E_PID=$!
  sleep 2
  note "host e2e server up (19991)"
fi
adb_section "Host: E2E Full-Stack Sweep (20 accounts)" \
  python3 scripts/e2e-full-stack-sweep.py 19991
rc "e2e-full-stack-sweep (host, port 19991)" $?
[ -n "${E2E_PID:-}" ] && { kill "$E2E_PID" 2>/dev/null || true; rm -rf "$TMPDIR_E2E"; }

# --- Disconnect-recovery: its OWN fresh server on 19992 ---
if [ -x "$HOST_CLI" ]; then
  TMPDIR_DISC=$(mktemp -d /tmp/device-test-disc-XXXX)
  "$HOST_CLI" --port 19992 launch >/dev/null 2>&1 &
  DISC_PID=$!
  sleep 2
  note "host disconnect server up (19992)"
fi
adb_section "Host: Disconnect Recovery" \
  python3 scripts/disconnect-recovery-test.py 19992
rc "disconnect-recovery (host, port 19992)" $?
[ -n "${DISC_PID:-}" ] && { kill "$DISC_PID" 2>/dev/null || true; rm -rf "$TMPDIR_DISC"; }

# ── 7. frontend + WASM (host-side, labeled — NOT device) ────────────
note "Frontend + WASM tests (host, Node/jsdom — shown for completeness)..."
# Capture exit code without `|| true` masking it.
if FE_OUT=$(cd apps/desktop && npm test 2>&1); then
  FE_RC=0
else
  FE_RC=$?
fi
if [ "$FE_RC" -eq 0 ]; then
  FE_TESTS=$(printf '%s\n' "$FE_OUT" | grep -aE "Tests " | tail -1 | tr -s ' ')
  adb_section "Host: Frontend + WASM (Node/jsdom)" printf '%s\n' "$FE_OUT"
  ok "frontend + wasm (host) — ${FE_TESTS}"
else
  adb_section "Host: Frontend + WASM (Node/jsdom) [FAILED]" printf '%s\n' "$FE_OUT"
  bad "frontend + wasm (host) — failed"
fi

# ── 8. finalize report ──────────────────────────────────────────────
note "Cleaning up adb forward..."
adb -s "$DEVICE_ID" forward --remove tcp:$HOST_FWD >/dev/null 2>&1 || true

{
  echo ""
  echo "## Summary"
  echo ""
  echo "- **Passed:** $PASS"
  echo "- **Failed:** $FAIL"
  echo "- **Result:** $([ "$FAIL" -eq 0 ] && echo 'ALL PASSED' || echo 'FAILURES PRESENT')"
} >> "$REPORT"

echo ""
echo "═══════════════════════════════════════════════════"
if [ "$FAIL" -eq 0 ]; then echo -e "  ${GREEN}ALL DEVICE TESTS PASSED${NC}"; else echo -e "  ${RED}SOME DEVICE TESTS FAILED${NC}"; fi
echo "  Report: $REPORT"
echo "═══════════════════════════════════════════════════"
exit $(( FAIL > 0 ? 1 : 0 ))