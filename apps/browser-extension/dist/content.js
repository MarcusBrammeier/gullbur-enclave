/**
 * Content script — EIP-6963 provider injection.
 *
 * DUMB PIPE RULE: This script does NOT parse JSON-RPC payloads,
 * hold transaction state, or perform any cryptographic logic.
 * It forwards raw payloads between dApp ↔ background service worker.
 */

// ── UUID generator (dumb, no deps) ────────────────────────────────────────

function generateUUID() {
  const buf = new Uint8Array(16);
  crypto.getRandomValues(buf);
  buf[6] = (buf[6] & 0x0f) | 0x40;
  buf[8] = (buf[8] & 0x3f) | 0x80;
  const hex = Array.from(buf, (b) => b.toString(16).padStart(2, "0"));
  return [
    hex.slice(0, 4).join(""),
    hex.slice(4, 6).join(""),
    hex.slice(6, 8).join(""),
    hex.slice(8, 10).join(""),
    hex.slice(10).join(""),
  ].join("-");
}

// ── Unique request ID generator ───────────────────────────────────────────

let requestId = 0;

// ── Inject EIP-6963 provider into dApp page ──────────────────────────────

function injectProvider() {
  const script = document.createElement("script");
  script.textContent = `
    (() => {
      // Wait for page to be ready, then announce
      const announce = () => {
        const info = {
          uuid: "${generateUUID()}",
          name: "FOSS Crypto Wallet",
          icon: "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMzIiIGhlaWdodD0iMzIiIHZpZXdCb3g9IjAgMCAzMiAzMiIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iMzIiIGhlaWdodD0iMzIiIHJ4PSI2IiBmaWxsPSIjMUExQjJFIi8+PHRleHQgeD0iMTYiIHk9IjIyIiBmb250LXNpemU9IjE4IiB0ZXh0LWFuY2hvcj0ibWlkZGxlIiBmaWxsPSJ3aGl0ZSIgZm9udC1mYW1pbHk9Im1vbm9zcGFjZSI+8J+SuzwvdGV4dD48L3N2Zz4=",
          rdns: "io.gullbur.wallet",
        };

        const detail = Object.freeze({ info });

        // EIP-6963: announce provider
        window.dispatchEvent(
          new CustomEvent("eip6963:announceProvider", { detail })
        );

        // Legacy: set window.ethereum for non-EIP-6963 dApps (MetaMask-compatible)
        const provider = {
          isFossCrypto: true,
          isMetaMask: true,           // Spoof: legacy dApps require this
          chainId: "0x1",

          request: async ({ method, params }) => {
            // DUMB PIPE: forward raw payload to content script
            return new Promise((resolve, reject) => {
              const id = Date.now() + Math.random();
              const message = { type: "ETH_REQUEST", id, method, params };
              window.postMessage(message, "*");

              const handler = (event) => {
                if (event.data?.type === "ETH_RESPONSE" && event.data.id === id) {
                  window.removeEventListener("message", handler);
                  if (event.data.error) {
                    reject(new Error(event.data.error.message || "Unknown error"));
                  } else {
                    resolve(event.data.result);
                  }
                }
              };
              window.addEventListener("message", handler);
            });
          },

          on: (_event, _callback) => {
            // Event subscription stub — not needed for EIP-6963 baseline
          },

          removeListener: () => {
            // No-op stub
          },
        };

        Object.defineProperty(window, "ethereum", {
          value: provider,
          writable: false,
          configurable: false,
        });

        // Channel B: Next-Gen vault_* API for EIP-6963-aware dApps
        const vaultProvider = {
          isFossCrypto: true,
          rdns: "io.gullbur.wallet",

          executeBatch: async (transactions) => {
            return new Promise((resolve, reject) => {
              const id = Date.now() + Math.random();
              window.postMessage({ type: "ETH_REQUEST", id, method: "vault_executeBatch", params: { txs: transactions } }, "*");
              const handler = (event) => {
                if (event.data?.type === "ETH_RESPONSE" && event.data.id === id) {
                  window.removeEventListener("message", handler);
                  if (event.data.error) reject(new Error(event.data.error.message));
                  else resolve(event.data.result);
                }
              };
              window.addEventListener("message", handler);
            });
          },

          requestSessionKey: async (policy) => {
            return new Promise((resolve, reject) => {
              const id = Date.now() + Math.random();
              window.postMessage({ type: "ETH_REQUEST", id, method: "vault_requestSessionKey", params: { policy } }, "*");
              const handler = (event) => {
                if (event.data?.type === "ETH_RESPONSE" && event.data.id === id) {
                  window.removeEventListener("message", handler);
                  if (event.data.error) reject(new Error(event.data.error.message));
                  else resolve(event.data.result);
                }
              };
              window.addEventListener("message", handler);
            });
          },

          simulateAndSend: async (tx) => {
            return new Promise((resolve, reject) => {
              const id = Date.now() + Math.random();
              window.postMessage({ type: "ETH_REQUEST", id, method: "vault_simulateAndSend", params: { tx } }, "*");
              const handler = (event) => {
                if (event.data?.type === "ETH_RESPONSE" && event.data.id === id) {
                  window.removeEventListener("message", handler);
                  if (event.data.error) reject(new Error(event.data.error.message));
                  else resolve(event.data.result);
                }
              };
              window.addEventListener("message", handler);
            });
          },
        };

        Object.defineProperty(window, "vault", {
          value: vaultProvider,
          writable: false,
          configurable: false,
        });
      };

      if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", announce);
      } else {
        announce();
      }
    })();
  `;

  (document.head || document.documentElement).appendChild(script);
  script.remove();
}

// ── dApp message relay to background ─────────────────────────────────────

window.addEventListener("message", (event) => {
  if (event.source !== window) return;
  if (event.data?.type !== "ETH_REQUEST") return;

  const { id, method, params } = event.data;

  // DUMB PIPE: forward raw payload to background service worker.
  // No parsing, no validation, no state — just forward.
  chrome.runtime.sendMessage(
    { type: "NATIVE_REQUEST", id, method, params },
    (response) => {
      // Forward raw response back to dApp
      window.postMessage(
        {
          type: "ETH_RESPONSE",
          id,
          result: response?.result ?? null,
          error: response?.error ?? null,
        },
        "*",
      );
    },
  );
});

// ── Inject on load ────────────────────────────────────────────────────────

injectProvider();
