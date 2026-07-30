/**
 * Tauri v2 Isolation Pattern — WASM AES-GCM shim.
 *
 * This module runs inside the sandboxed isolation iframe. It:
 * 1. Receives a per-session AES-256-GCM key from the Rust backend
 * 2. Encrypts ALL outgoing IPC payloads before they reach the Rust IPC handler
 * 3. Decrypts ALL incoming IPC responses before they reach the Svelte frontend
 *
 * The key is shared only between this iframe and the Rust backend —
 * the main window's Svelte code NEVER has access to cryptographic keys.
 */

// ── State ─────────────────────────────────────────────────────────────────────

/** Per-session AES-GCM key — set once on first IPC call */
let sessionKey = null;

/** WASM crypto module bindings */
let wasmEncrypt = null;
let wasmDecrypt = null;
let wasmInitialized = false;

/** Initialize the WASM crypto module */
async function initCryptoWasm() {
  if (wasmInitialized) return;
  const wasm = await import('./crypto_wasm.js');
  await wasm.default('crypto_wasm_bg.wasm');
  wasmEncrypt = wasm.encrypt;
  wasmDecrypt = wasm.decrypt;
  wasmInitialized = true;
}

/**
 * Store the session key received from the Rust backend.
 * The key is a hex-encoded 64-char string — used directly by WASM crypto.
 */
async function importKey(rawKeyHex) {
  if (!wasmInitialized) await initCryptoWasm();
  sessionKey = rawKeyHex;
}

/**
 * Encrypt a JSON payload with AES-GCM via WASM.
 * Returns { iv_hex: string, data_b64: string } — matching the Rust EncryptedPayload format.
 */
async function encrypt(payload) {
  const jsonData = JSON.stringify(payload);
  const encryptedStr = wasmEncrypt(sessionKey, jsonData);
  return JSON.parse(encryptedStr);
}

/**
 * Decrypt an AES-GCM response via WASM.
 * Expects { iv_hex, data_b64 } — matching the Rust EncryptedPayload format.
 */
async function decrypt(encrypted) {
  const payloadJson = JSON.stringify(encrypted);
  const decryptedStr = wasmDecrypt(sessionKey, payloadJson);
  return JSON.parse(decryptedStr);
}

// ── Tauri Isolation Hook ──────────────────────────────────────────────────────

/**
 * This is the Tauri v2 Isolation Hook.
 * It is called by Tauri for EVERY IPC invoke() from the main window.
 *
 * @param {object} payload - { method: string, args: object }
 * @param {function} callback - Call with the (possibly modified) payload
 *
 * Flow:
 * 1. First call: extract session key from args (sent by Rust at init)
 * 2. All subsequent calls: encrypt args, pass encrypted payload to callback
 */
window.__TAURI_ISOLATION_HOOK__ = async (payload, callback) => {
  try {
    // ── Key exchange: first call carries session key ────────────────────
    if (!sessionKey && payload.args && payload.args.__session_key__) {
      await importKey(payload.args.__session_key__);
      // Remove the key from the payload before forwarding
      const { __session_key__, ...cleanArgs } = payload.args;
      callback({ method: payload.method, args: cleanArgs });
      return;
    }

    // ── Regular IPC: encrypt the args ──────────────────────────────────
    if (!sessionKey) {
      // No key yet — pass through (shouldn't happen with proper init)
      callback(payload);
      return;
    }

    const encrypted = await encrypt(payload.args);
    callback({
      method: payload.method,
      args: {
        __encrypted__: true,
        __payload__: encrypted,
      },
    });
  } catch (err) {
    // If encryption fails, pass through raw (fail open for debugging)
    console.error('[isolation] Encryption failed:', err);
    callback(payload);
  }
};

// ── Response Hook (decryption) ────────────────────────────────────────────────

/**
 * The response hook intercepts IPC responses coming BACK from the Rust backend.
 * It decrypts them before passing to the main window.
 */
window.__TAURI_ISOLATION_RESPONSE_HOOK__ = async (response, callback) => {
  try {
    if (
      sessionKey &&
      response &&
      typeof response === 'object' &&
      response.__encrypted__
    ) {
      const decrypted = await decrypt(response.__payload__);
      callback(decrypted);
    } else {
      callback(response);
    }
  } catch (err) {
    console.error('[isolation] Decryption failed:', err);
    callback(response);
  }
};