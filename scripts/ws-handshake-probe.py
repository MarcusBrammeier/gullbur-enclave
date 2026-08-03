#!/usr/bin/env python3
"""Gullbúr Enclave — Live WebSocket handshake + RPC probe.

Closes the testing hole where the sweep scripts only checked that port
19876 was *listening* (/proc/net/tcp) but never verified the actual IPC
handshake succeeds. This connects over a real WebSocket to a RUNNING
binary and asserts the exact protocol the Svelte frontend uses:

    1. WS connect to ws://127.0.0.1:<port>
    2. send {"type":"hello"}            (loopback trust, no token)
    3. expect {"type":"session_key",...}
    4. send a JSON-RPC request
    5. expect a JSON-RPC response (result present)

Exit 0 = handshake + RPC verified. Requires the `websockets` package.
"""
import sys
import json

from websockets.sync.client import connect

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19876
METHOD = sys.argv[2] if len(sys.argv) > 2 else "vault.generate_mnemonic"
TIMEOUT = 8.0


def handshake_ok(port: int, method: str) -> bool:
    uri = f"ws://127.0.0.1:{port}"
    try:
        with connect(uri, open_timeout=TIMEOUT) as ws:
            # 1. hello handshake
            ws.send(json.dumps({"type": "hello"}))
            raw = ws.recv(timeout=TIMEOUT)
            msg = json.loads(raw)
            if msg.get("type") != "session_key" or not msg.get("key"):
                print(f"  [FAIL] expected session_key, got: {raw[:120]!r}")
                return False
            print(f"  [ok] session_key exchanged ({len(msg['key'])} hex chars)")

            # 2. JSON-RPC round-trip over the live socket
            req = {
                "jsonrpc": "2.0",
                "method": method,
                "params": {},
                "id": 1,
            }
            ws.send(json.dumps(req))
            resp_raw = ws.recv(timeout=TIMEOUT)
            resp = json.loads(resp_raw)
            # Server may encrypt when encrypt=true + plain request -> plain response,
            # but generate_mnemonic over hello (plain) stays plain (is_encrypted=false).
            if "error" in resp:
                print(f"  [FAIL] RPC {method} returned error: {resp['error']}")
                return False
            if "result" not in resp:
                print(f"  [FAIL] RPC {method} missing result: {resp_raw[:120]!r}")
                return False
            print(f"  [ok] RPC {method} -> result returned")
            return True
    except Exception as e:  # noqa: BLE001 - probe reports any failure
        print(f"  [FAIL] handshake probe exception: {e}")
        return False


if __name__ == "__main__":
    ok = handshake_ok(PORT, METHOD)
    print(f"  => WS handshake + RPC on :{PORT}: {'PASS' if ok else 'FAIL'}")
    sys.exit(0 if ok else 1)
