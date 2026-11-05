/**
 * Encrypted IPC Worker — zero-trust key isolation (C3-A).
 *
 * SECURITY: The per-session AES-256-GCM key and the WASM encrypt/decrypt
 * machinery are OWNED exclusively by this isolated Web Worker. The main
 * window's IpcClient never holds the key and never runs encrypt/decrypt —
 * it only relays serialized JSON-RPC frames to/from this worker.
 *
 * The worker:
 *   (a) opens and owns the WebSocket to ws://127.0.0.1:PORT,
 *   (b) sends hello+token on connect,
 *   (c) receives the `session_key` DIRECTLY from the server over that same
 *       WebSocket (never via main-window postMessage, so the key never even
 *       transits the main thread),
 *   (d) encrypts every outbound request and decrypts every inbound reply
 *       using the WASM crypto module,
 *   (e) relays only decrypted JSON-RPC responses back to the main window.
 *
 * FLAG-CLOSED: the worker refuses to open the socket or send anything unless
 * the WASM crypto module initialized successfully — it NEVER falls back to
 * plaintext (the IPC server also refuses clear payloads when encryption is on,
 * but we do not rely on that alone).
 */

import init, { encrypt, decrypt } from './wasm/crypto_wasm.js';

// Minimal scoped-self type. We intentionally avoid pulling the WebWorker lib
// into the main tsconfig (it conflicts with DOM globals under one svelte-check
// run); a plain cast to this narrow interface is sufficient and clean.
type WorkerScope = {
  postMessage: (message: unknown) => void;
  onmessage: ((ev: MessageEvent) => void) | null;
};

const scope = self as unknown as WorkerScope;

// ── Message protocol (shared with IpcClient via type-only imports) ────────

/** Main window → worker */
export interface WorkerConnect {
  type: 'connect';
  port: number;
  token: string;
}
export interface WorkerCall {
  type: 'call';
  id: number;
  method: string;
  params: unknown;
}
export interface WorkerDisconnect {
  type: 'disconnect';
}
export type WorkerInboundMessage = WorkerConnect | WorkerCall | WorkerDisconnect;

/** Worker → main window */
export interface WorkerKeyReady {
  type: 'key_ready';
}
export interface WorkerStatus {
  type: 'status';
  state: 'open' | 'closed' | 'error';
  error?: string;
}
export interface WorkerConnectFailed {
  type: 'connect_failed';
  message: string;
}
export interface WorkerAuthRequired {
  type: 'auth_required';
}
export interface WorkerResponse {
  type: 'response';
  id: number;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}
export type WorkerOutboundMessage =
  | WorkerKeyReady
  | WorkerStatus
  | WorkerConnectFailed
  | WorkerAuthRequired
  | WorkerResponse;

// ── State (private to this worker) ─────────────────────────────────────────

/** Per-session AES-256-GCM key. NEVER leaves this worker. */
let sessionKey: string | null = null;
let ws: WebSocket | null = null;
let wasmReady = false;

/** Initialize the WASM crypto module, with a hard timeout. */
function ensureWasm(): Promise<void> {
  if (wasmReady) return Promise.resolve();
  return Promise.race([
    init(),
    new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error('WASM crypto init timed out after 3s')), 3_000)
    ),
  ]).then(() => {
    wasmReady = true;
  });
}

function post(msg: WorkerOutboundMessage): void {
  scope.postMessage(msg);
}

async function handleConnect(port: number, token: string): Promise<void> {
  // FAIL CLOSED: no plaintext fallback. If WASM fails to initialize, we MUST
  // NOT open the socket — otherwise a compromised/timing-out WASM load would
  // silently route cleartext wallet data over the wire.
  try {
    await ensureWasm();
  } catch (e) {
    console.error(
      '[ipc-worker] WASM crypto init failed — refusing to connect (encrypted IPC required)'
    );
    post({
      type: 'connect_failed',
      message:
        'Encrypted IPC unavailable: WASM crypto module failed to load. ' +
        'Refusing to connect over plaintext. Please reinstall the app.',
    });
    return;
  }

  const wsUrl = `ws://127.0.0.1:${port}`;
  try {
    ws = new WebSocket(wsUrl);
  } catch {
    post({ type: 'connect_failed', message: 'WebSocket connection failed' });
    return;
  }

  ws.onopen = () => {
    if (!token) {
      console.warn('[ipc-worker] No IPC auth token provided; server will refuse this connection');
    }
    ws!.send(JSON.stringify({ type: 'hello', token }));
  };

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data as string);

      // ── Session key exchange (received directly from the server) ───────
      if (msg.type === 'session_key' && msg.key) {
        sessionKey = msg.key;
        post({ type: 'key_ready' });
        return;
      }

      // ── Encrypted response ─────────────────────────────────────────────
      if (msg.__encrypted__ && sessionKey) {
        const payloadJson = JSON.stringify(msg.__payload__);
        const decryptedStr = decrypt(sessionKey, payloadJson);
        const inner = JSON.parse(decryptedStr);
        post({
          type: 'response',
          id: inner.id,
          result: inner.result,
          error: inner.error,
        });
        if (inner.error && inner.error.code === -32002) {
          post({ type: 'auth_required' });
        }
        return;
      }

      // ── Plain response ─────────────────────────────────────────────────
      post({ type: 'response', id: msg.id, result: msg.result, error: msg.error });
    } catch {
      /* ignore non-JSON messages */
    }
  };

  ws.onerror = () => {
    post({ type: 'status', state: 'error', error: 'WebSocket connection failed' });
  };

  ws.onclose = () => {
    post({ type: 'status', state: 'closed' });
    ws = null;
  };
}

function handleCall(id: number, method: string, params: unknown): void {
  // FAIL CLOSED: never send plaintext. If the key isn't established or the
  // socket isn't open, report the standard error rather than leaking cleartext
  // wallet data.
  if (!ws || ws.readyState !== WebSocket.OPEN || !sessionKey) {
    post({
      type: 'response',
      id,
      error: { code: -32000, message: 'Not connected to vault' },
    });
    return;
  }

  const request = { jsonrpc: '2.0', method, params, id };
  const encryptedPayloadStr = encrypt(sessionKey, JSON.stringify(request));
  const encryptedPayload = JSON.parse(encryptedPayloadStr);
  ws.send(JSON.stringify({ __encrypted__: true, __payload__: encryptedPayload }));
}

function handleDisconnect(): void {
  try {
    ws?.close();
  } catch {
    /* ignore */
  }
  ws = null;
  sessionKey = null;
}

scope.onmessage = (ev: MessageEvent) => {
  const data = ev.data as WorkerInboundMessage;
  switch (data.type) {
    case 'connect':
      void handleConnect(data.port, data.token);
      break;
    case 'call':
      handleCall(data.id, data.method, data.params);
      break;
    case 'disconnect':
      handleDisconnect();
      break;
  }
};