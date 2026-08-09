#!/usr/bin/env python3
"""
Gullbur Enclave - Daemon Crash & Disconnect Recovery Test

Simulates the IPC daemon being forcefully killed mid-session (as if the
Tauri backend crashed or the user force-quit), then validates that a
new daemon process can be started and the frontend can reconnect cleanly.

This is the exact scenario the connect() retry loop + exponential backoff
in vault.svelte.ts was designed to handle.

Exit code 0 = all checks passed.
"""
import json
import os
import signal
import subprocess
import sys
import time
from websockets.sync.client import connect

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19876
CLI = os.path.join(os.path.dirname(__file__), "..", "target/release/gullbur-cli")
TIMEOUT = 15.0
PASS = 0
FAIL = 0
FAILURES = []
SERVER_PID = None


def ws_call(method: str, params: dict, port=PORT) -> dict:
    with connect(f"ws://127.0.0.1:{port}", open_timeout=TIMEOUT) as ws:
        ws.send(json.dumps({"type": "hello"}))
        raw = ws.recv(timeout=TIMEOUT)
        msg = json.loads(raw)
        assert msg.get("type") == "session_key", f"expected session_key, got {raw[:80]}"
        req = {"jsonrpc": "2.0", "method": method, "params": params, "id": 1}
        ws.send(json.dumps(req))
        resp_raw = ws.recv(timeout=TIMEOUT)
        resp = json.loads(resp_raw)
        if "error" in resp:
            return {"error": resp["error"]}
        return resp.get("result", {})


def check(label: str, ok: bool, detail: str = ""):
    global PASS, FAIL
    if ok:
        PASS += 1
        print(f"  {chr(10003)} {label}")
    else:
        FAIL += 1
        FAILURES.append(f"{label}: {detail}")
        print(f"  {chr(10007)} {label}: {detail}")


def get_server_pid():
    """Find the IPC server process PID by checking /proc/net/tcp."""
    try:
        port_hex = f"{PORT:04x}"
        with open("/proc/net/tcp") as f:
            for line in f:
                parts = line.strip().split()
                if len(parts) >= 10 and port_hex in parts[1]:
                    pid_hex = parts[9]
                    if pid_hex != "00":
                        return int(pid_hex, 16)
        return None
    except Exception:
        return None


print("=============================================")
print("  Gullbur Enclave - Daemon Crash & Recovery")
print("  Port: %d" % PORT)
print("=============================================")
print("")

old_pid = get_server_pid()
check("server process found (PID %s)" % old_pid, old_pid is not None)

# Phase 1: Baseline verification
print("")
print("> [1] Baseline: server healthy")
mn = ws_call("vault.generate_mnemonic", {})
words = mn.get("mnemonic", "").split()
check("generate_mnemonic works (24 words)", len(words) == 24, "got %d" % len(words))

init = ws_call("vault.initialize", {"seed_phrase": mn.get("mnemonic", ""), "passphrase": ""})
check("vault.initialize succeeds", init.get("success") is True, str(init))

acct = ws_call("vault.create_account", {"network": "bitcoin", "index": 0})
check("create_account works", "address" in acct, str(acct)[:60])

# Phase 2: Force-kill the daemon
print("")
print("> [2] Force-kill daemon (SIGKILL)")
if old_pid:
    try:
        os.kill(old_pid, signal.SIGKILL)
        for _ in range(20):
            try:
                os.kill(old_pid, 0)
                time.sleep(0.1)
            except ProcessLookupError:
                break
        check("daemon PID %d killed" % old_pid, True)
    except Exception as e:
        check("killing daemon", False, str(e))
else:
    check("cannot kill server (no PID)", False)
    sys.exit(1)

# Phase 3: Verify old connection is dead
print("")
print("> [3] Old connection is dead")
try:
    with connect(f"ws://127.0.0.1:{PORT}", open_timeout=2) as ws:
        ws.send(json.dumps({"type": "hello"}))
        raw = ws.recv(timeout=3)
        check("old daemon gone", False, "unexpected response: %s" % raw[:80])
except (ConnectionRefusedError, OSError, TimeoutError) as e:
    check("old daemon gone: connection refused", True, str(e)[:60])

# Phase 4: Restart the daemon
print("")
print("> [4] Restart daemon")
new_pid = None
try:
    proc = subprocess.Popen(
        [CLI, "--port", str(PORT), "launch"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2)
    for attempt in range(8):
        new_pid = get_server_pid()
        if new_pid:
            break
        time.sleep(0.2 * (2 ** attempt))
    check("daemon restarted (PID %s)" % new_pid,
          new_pid is not None and new_pid != old_pid,
          "old=%s new=%s" % (old_pid, new_pid))
except Exception as e:
    check("restart daemon", False, str(e))

# Phase 5: Reconnect and verify
print("")
print("> [5] Reconnect + verify")
if new_pid is not None:
    try:
        st = ws_call("vault.status", {})
        check("reconnect: status returns data",
              "plugin_ids" in st and "networks" in st, str(st)[:80])
        check("reconnect: vault still initialized",
              st.get("initialized") is True, str(st))
        acct2 = ws_call("vault.create_account", {"network": "ethereum", "index": 1})
        check("reconnect: create_account after restart", "address" in acct2, str(acct2)[:60])
    except Exception as e:
        check("reconnect + functional verification", False, str(e))
else:
    check("reconnect tests skipped (no new PID)", False)

# Phase 6: Cleanup
print("")
print("> [6] Cleanup")
if new_pid:
    try:
        os.kill(new_pid, signal.SIGTERM)
        time.sleep(0.5)
        try:
            os.kill(new_pid, 0)
            os.kill(new_pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        check("test daemon (PID %d) terminated" % new_pid, True)
    except Exception as e:
        check("cleanup", False, str(e))

# Summary
print("")
print("=============================================")
t = PASS + FAIL
if FAIL == 0:
    print("  ALL %d/%d CHECKS PASSED" % (PASS, PASS))
else:
    print("  %d/%d PASS, %d/%d FAIL" % (PASS, t, FAIL, t))
    for f in FAILURES:
        print("    FAIL %s" % f)
print("=============================================")
sys.exit(1 if FAIL > 0 else 0)