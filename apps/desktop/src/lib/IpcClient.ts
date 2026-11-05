/**
 * IPC Client — connects to the vault-core WebSocket server.
 *
 * SECURITY (C3-A, zero-trust key isolation): all cryptography — the per-session
 * AES-256-GCM key and the WASM encrypt/decrypt — lives inside an isolated
 * sandboxed Web Worker (`ipcWorker.ts`). The worker opens and owns the
 * WebSocket, receives the session key directly from the server, and encrypts/
 * decrypts every frame. This class NEVER holds the session key and never runs
 * encrypt/decrypt in main-window JS; it only relays serialized JSON-RPC
 * requests to the worker and surfaces the worker's decrypted responses.
 *
 * Public API (connect / call / disconnect / onAuthRequired) is unchanged so
 * callers (vault.svelte.ts, MockIpcClient) are unaffected.
 */

import type {
  WorkerInboundMessage,
  WorkerOutboundMessage,
} from './ipcWorker';

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

export interface WorkerResponseMsg {
  type: 'response';
  id: number;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void };

export class IpcClient {
  private worker: Worker | null = null;
  private nextId = 1;
  private pending = new Map<number, Pending>();
  /** Callback for auth errors — called when -32002 is received */
  onAuthRequired?: () => void;
  private connectResolve: ((value: void) => void) | null = null;
  private connectReject: ((reason: Error) => void) | null = null;
  private connected = false;
  private connectTimer: ReturnType<typeof setTimeout> | null = null;
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

  private clearConnectTimer(): void {
    if (this.connectTimer) { clearTimeout(this.connectTimer); this.connectTimer = null; }
  }

  /** Reject the in-flight connect (worker boot failure, timeout, crash). */
  private failConnect(e: Error): void {
    this.clearConnectTimer();
    const rejectFn = this.connectReject;
    this.connectResolve = null;
    this.connectReject = null;
    this.worker?.terminate();
    this.worker = null;
    this.connected = false;
    rejectFn?.(e);
  }

  async connect(port?: number, authToken?: string): Promise<void> {
    if (typeof Worker === 'undefined') {
      // Fail closed: no Worker support means no isolated crypto.
      this.dbg('Worker is unavailable in this runtime — refusing to connect');
      throw new Error(
        'Encrypted IPC unavailable: worker isolation is not supported in this runtime. ' +
        'Refusing to connect over plaintext.'
      );
    }

    this.dbg(`connect() called, port=${port}`);
    this.connected = false;

    // 1. Spawn the isolated worker and wire its message bus.
    //    The worker initializes WASM and opens the WebSocket.
    let worker: Worker;
    try {
      worker = new Worker(new URL('./ipcWorker.ts', import.meta.url), { type: 'module' });
    } catch (e) {
      this.dbg(`Worker spawn failed: ${e instanceof Error ? e.message : String(e)}`);
      throw new Error(
        'Encrypted IPC unavailable: failed to start isolated crypto worker. ' +
        'Refusing to connect over plaintext.'
      );
    }
    this.worker = worker;

    return new Promise((resolve, reject) => {
      this.connectResolve = resolve;
      this.connectReject = reject;

      const wsPort = port ?? 19876;

      // Force-reject if the worker doesn't report key exchange in time.
      this.connectTimer = setTimeout(() => {
        this.dbg('CONNECT TIMEOUT — no session_key received');
        this.failConnect(
          new Error(`WebSocket connect timed out after ${IpcClient.CONNECT_TIMEOUT_MS}ms`)
        );
      }, IpcClient.CONNECT_TIMEOUT_MS);

      worker.onmessage = (event: MessageEvent) => {
        const msg = event.data as WorkerOutboundMessage;
        switch (msg.type) {
          case 'key_ready':
            this.clearConnectTimer();
            const resolveFn = this.connectResolve;
            this.connectResolve = null;
            this.connectReject = null;
            this.connected = true;
            this.dbg('Worker received session key — connection established');
            resolveFn?.();
            break;
          case 'connect_failed':
            this.failConnect(new Error(msg.message));
            break;
          case 'status':
            if (msg.state === 'closed') this.connected = false;
            break;
          case 'auth_required':
            try { this.onAuthRequired?.(); } catch { /* callback is best-effort */ }
            break;
          case 'response':
            this.handleResponse(msg);
            break;
        }
      };
      worker.onerror = (e) => {
        this.dbg(`Worker onerror: ${e.message}`);
        this.failConnect(new Error(`IPC crypto worker crashed: ${e.message}`));
      };

      const connectMsg: WorkerInboundMessage = { type: 'connect', port: wsPort, token: authToken ?? '' };
      this.sendToWorker(connectMsg);
      this.dbg(`Sent connect to worker for ws://127.0.0.1:${wsPort}`);
    });
  }

  private handleResponse(msg: WorkerResponseMsg): void {
    if (msg.error) {
      const pending = this.pending.get(msg.id);
      if (pending) {
        this.pending.delete(msg.id);
        pending.reject(new Error(msg.error.message));
      }
      return;
    }
    const pending = this.pending.get(msg.id);
    if (pending) {
      this.pending.delete(msg.id);
      pending.resolve(msg.result);
    }
  }

  async call(method: string, params: unknown = {}): Promise<unknown> {
    if (!this.connected || !this.worker) {
      this.log('receive', method, 'Not connected to vault', true);
      throw new Error("Not connected to vault");
    }

    const id = this.nextId++;

    return new Promise((resolve, reject) => {
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

      // The worker owns encryption + the socket; we only relay the plaintext
      // JSON-RPC request. sessionKey never exists on this thread.
      const sendMsg: WorkerInboundMessage = { type: 'call', id, method, params };
      this.sendToWorker(sendMsg);
      this.log('send', method, JSON.stringify(params).slice(0, 120), false);
    });
  }

  disconnect(): void {
    this.sendToWorker({ type: 'disconnect' });
    this.worker?.terminate();
    this.worker = null;
    this.connected = false;
    this.clearConnectTimer();
    this.connectResolve = null;
    this.connectReject = null;
    for (const [, p] of this.pending) {
      p.reject(new Error("Connection closed"));
    }
    this.pending.clear();
  }

  private sendToWorker(msg: WorkerInboundMessage): void {
    if (!this.worker) throw new Error("Not connected to vault");
    this.worker.postMessage(msg);
  }
}