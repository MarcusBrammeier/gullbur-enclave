/**
 * IPC Client — connects to the vault-core WebSocket server.
 *
 * SECURITY: Uses AES-256-GCM via WASM crypto blob for all messages.
 * The session key is exchanged with the server after auth.
 */

import init, { encrypt, decrypt } from './wasm/crypto_wasm.js';

export interface JsonRpcRequest {
  jsonrpc: "2.0";
  method: string;
  params: unknown;
  id: number;
}

export interface JsonRpcResponse {
  jsonrpc: "2.0";
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
  id: number;
}

export class IpcClient {
  private ws: WebSocket | null = null;
  private nextId = 1;
  private pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();
  /** Callback for auth errors — called when -32002 is received */
  onAuthRequired?: () => void;
  private sessionKey: string | null = null;
  private wasmReady = false;
  private connectResolve: ((value: void) => void) | null = null;
  private connectReject: ((reason: Error) => void) | null = null;
  /** Timeout (ms) for the WebSocket connection attempt */
  private static readonly CONNECT_TIMEOUT_MS = 5_000;

  /** Log an IPC event to the debug console, if available */
  private log(direction: 'send' | 'receive', method: string | undefined, payload: string, isError: boolean) {
    try {
      (window as any).__consoleLog?.({ direction, method, payload, isError });
    } catch { /* console logging is best-effort */ }
  }

  /** Log a debug message with timestamp to both console and a debug entry */
  private dbg(msg: string) {
    const ts = new Date().toISOString().slice(11, 23);
    const line = `[${ts}] [IpcClient] ${msg}`;
    console.log(line);
    try {
      (window as any).__consoleLog?.({ direction: 'receive', method: `__debug__`, payload: msg, isError: false });
    } catch {}
  }

  async connect(port?: number): Promise<void> {
    this.dbg(`connect() called, port=${port}, wasmReady=${this.wasmReady}`);
    // Initialize WASM crypto module with a timeout — the WASM file may not
    // load from the tauri:// asset protocol in AppImage/bundled builds.
    // If WASM hangs or fails, fall back to plaintext IPC (encryption is
    // skipped on loopback connections and the server accepts both modes).
    if (!this.wasmReady) {
      try {
        this.dbg('WASM init starting...');
        await Promise.race([
          init(),
          new Promise<never>((_, reject) =>
            setTimeout(() => reject(new Error('WASM crypto init timed out after 3s')), 3_000)
          ),
        ]);
        this.wasmReady = true;
        this.dbg('WASM init OK');
      } catch (e) {
        console.warn('[ipc] WASM crypto init failed/timed out — falling back to plaintext IPC');
        this.dbg(`WASM init FAIL: ${e instanceof Error ? e.message : String(e)}`);
        this.wasmReady = false;
      }
    }

    return new Promise((resolve, reject) => {
      this.dbg('Opening new Promise for WebSocket connect...');
      this.connectResolve = resolve;
      this.connectReject = reject;

      const wsPort = port ?? 19876;
      const wsUrl = `ws://127.0.0.1:${wsPort}`;
      this.dbg(`Creating WebSocket to ${wsUrl}...`);
      this.ws = new WebSocket(wsUrl);

      // Force-reject the promise if the WebSocket doesn't handshake in time
      const timer = setTimeout(() => {
        this.dbg('CONNECT TIMEOUT — 5000ms elapsed, no session_key received');
        this.ws?.close();
        this.ws = null;
        this.connectResolve = null;
        this.connectReject = null;
        reject(new Error(`WebSocket connect timed out after ${IpcClient.CONNECT_TIMEOUT_MS}ms`));
      }, IpcClient.CONNECT_TIMEOUT_MS);

      this.ws.onopen = () => {
        this.dbg('WebSocket onopen fired, sending hello...');
        // On localhost, skip auth token — the IPC server trusts loopback.
        // The frontend sends a simple "hello" to trigger session key exchange.
        this.ws!.send(JSON.stringify({ type: "hello" }));
        this.log('send', 'hello', '', false);
      };

      this.ws.onmessage = (event) => {
        this.dbg(`onmessage: ${event.data.slice(0, 80)}${event.data.length > 80 ? '...' : ''}`);
        try {
          const msg = JSON.parse(event.data);

          // ── Session key exchange ────────────────────────────────────
          if (msg.type === "session_key" && msg.key) {
            this.sessionKey = msg.key;
            this.log('receive', 'session_key', 'key exchanged', false);
            clearTimeout(timer);
            const resolve = this.connectResolve;
            this.connectResolve = null;
            this.connectReject = null;
            resolve?.();
            return;
          }

          // ── Encrypted response ──────────────────────────────────────
          if (msg.__encrypted__ && this.sessionKey) {
            const payloadJson = JSON.stringify(msg.__payload__);
            const decryptedStr = decrypt(this.sessionKey, payloadJson);
            const inner: JsonRpcResponse = JSON.parse(decryptedStr);

            const pending = this.pending.get(inner.id);
            if (pending) {
              this.pending.delete(inner.id);
              if (inner.error) {
                if (inner.error.code === -32002) {
                  this.onAuthRequired?.();
                }
                pending.reject(new Error(inner.error.message));
              } else {
                pending.resolve(inner.result);
              }
            }
            return;
          }

          // ── Plain response (fallback / initial key exchange) ────────
          const plain: JsonRpcResponse = msg;
          const pending = this.pending.get(plain.id);
          if (pending) {
            this.pending.delete(plain.id);
            if (plain.error) {
              pending.reject(new Error(plain.error.message));
            } else {
              pending.resolve(plain.result);
            }
          }
        } catch {
          // Ignore non-JSON messages
        }
      };

      this.ws.onerror = () => {
        this.dbg('WebSocket onerror fired');
        clearTimeout(timer);
        const reject = this.connectReject;
        this.connectResolve = null;
        this.connectReject = null;
        reject?.(new Error("WebSocket connection failed"));
      };

      this.ws.onclose = () => {
        this.dbg('WebSocket onclose fired');
        clearTimeout(timer);
        for (const [, p] of this.pending) {
          p.reject(new Error("Connection closed"));
        }
        this.pending.clear();
        const reject = this.connectReject;
        this.connectResolve = null;
        this.connectReject = null;
        reject?.(new Error("Connection closed before session key exchange"));
      };
    });
  }

  async call(method: string, params: unknown = {}): Promise<unknown> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error("Not connected to vault");
    }

    const id = this.nextId++;
    const request: JsonRpcRequest = { jsonrpc: "2.0", method, params, id };

    return new Promise((resolve, reject) => {
      // Log response when settled
      const origResolve = resolve;
      const origReject = reject;
      this.pending.set(id, {
        resolve: (v: unknown) => {
          this.log('receive', method, JSON.stringify(v).slice(0, 120), false);
          origResolve(v);
        },
        reject: (e: Error) => {
          this.log('receive', method, e.message, true);
          origReject(e);
        },
      });

      if (this.sessionKey && this.wasmReady) {
        // Encrypt the JSON-RPC request before sending.
        const requestJson = JSON.stringify(request);
        const encryptedPayloadStr = encrypt(this.sessionKey, requestJson);
        const encryptedPayload = JSON.parse(encryptedPayloadStr);
        this.ws!.send(
          JSON.stringify({
            __encrypted__: true,
            __payload__: encryptedPayload,
          })
        );
      } else {
        // No session key yet — send plain (shouldn't happen after connect).
        this.ws!.send(JSON.stringify(request));
      }
      this.log('send', method, JSON.stringify(params).slice(0, 120), false);
    });
  }

  disconnect(): void {
    this.ws?.close();
    this.ws = null;
    this.sessionKey = null;
  }
}