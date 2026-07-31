/**
 * Background Service Worker — Native Messaging Bridge.
 *
 * DUMB PIPE RULE: This script does NOT parse JSON-RPC, hold state,
 * or perform any cryptographic/routing logic. Its sole job is:
 *
 *   1. Receive raw payload from content script
 *   2. Stamp the dApp origin onto the envelope
 *   3. Forward to native host (extension-relay Rust crate)
 *   4. Return raw response to content script
 */

const NATIVE_HOST = "com.gullbur.wallet.relay";

// ── Message handler: content script → native host ─────────────────────────

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  // Only handle NATIVE_REQUEST messages — ignore everything else
  if (message?.type !== "NATIVE_REQUEST") {
    return false; // not handled, don't keep channel open
  }

  const { id, method, params } = message;

  // SECURITY: stamp the dApp origin onto the envelope.
  // sender.origin is provided by Chrome — it IS the dApp URL.
  // We do NOT validate it here; the Rust native host does that.
  const origin = sender?.origin ?? "unknown";

  const envelope = {
    origin,          // stamped by service worker, verified by Rust
    method,          // raw eth_* method — NOT parsed
    params: params ?? null,  // raw params — NOT parsed
    id,              // request correlation ID
  };

  // DUMB PIPE: forward raw envelope to native host.
  // chrome.runtime.sendNativeMessage handles JSON serialization.
  chrome.runtime.sendNativeMessage(NATIVE_HOST, envelope, (response) => {
    if (chrome.runtime.lastError) {
      console.error(
        `[foss-wallet] Native host error: ${chrome.runtime.lastError.message}`,
      );
      sendResponse({
        id,
        error: {
          code: -32000,
          message: `Native host unavailable: ${chrome.runtime.lastError.message}`,
        },
      });
      return;
    }

    // DUMB PIPE: forward raw response back to content script.
    // No inspection, no manipulation — just return what the host gave us.
    sendResponse({
      id: response?.id ?? id,
      result: response?.result ?? null,
      error: response?.error ?? null,
    });
  });

  // Return true to keep the message channel open for async sendResponse
  return true;
});

// ── Extension lifecycle ───────────────────────────────────────────────────

chrome.runtime.onInstalled.addListener(() => {
  console.log("[gullbur] Gullbúr Enclave extension installed");
  console.log(`[gullbur] Native host: ${NATIVE_HOST}`);
});

// ── Native host connection test ───────────────────────────────────────────

chrome.runtime.onStartup?.addListener(() => {
  chrome.runtime.sendNativeMessage(
    NATIVE_HOST,
    { origin: "chrome-extension://self", method: "eth_chainId", params: null, id: 0 },
    () => {
      if (chrome.runtime.lastError) {
        console.warn(
          `[foss-wallet] Native host not available on startup: ${chrome.runtime.lastError.message}`,
        );
      } else {
        console.log("[foss-wallet] Native host connected");
      }
    },
  );
});
