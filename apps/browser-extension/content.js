/**
 * Content script — EIP-6963 provider injection.
 *
 * DUMB PIPE RULE: This script does NOT parse JSON-RPC payloads,
 * hold transaction state, or perform any cryptographic logic.
 * It forwards raw payloads between dApp ↔ background service worker.
 *
 * EIP-6963 uuid is generated ONCE in the background service worker and
 * persisted in chrome.storage.local — see background.js getWalletUuid().
 */

// ── Inject EIP-6963 provider into dApp page ──────────────────────────────

function injectProvider() {
  // Resolve the STABLE per-extension uuid from the background service worker
  // (background.js persists it once in chrome.storage.local). EIP-6963 requires
  // a stable uuid; regenerating per-page would give dApps a false provider id.
  chrome.runtime.sendMessage({ type: "GET_PROVIDER_UUID" }, ({ uuid }) => {
    const providerUuid = uuid;
    injectWithUuid(providerUuid);
  });
}

function injectWithUuid(providerUuid) {
  const script = document.createElement("script");
  script.textContent = `
    (() => {
      const PROVIDER_UUID = ${JSON.stringify(providerUuid)};
      const RDNS = "io.gullbur.wallet";

      const buildInfo = () =>
        Object.freeze({
          info: Object.freeze({
            uuid: PROVIDER_UUID,
            name: "Gullbúr Enclave",
            icon: "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMzIiIGhlaWdodD0iMzIiIHZpZXdCb3g9IjAgMCAzMiAzMiIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iMzIiIGhlaWdodD0iMzIiIHJ4PSI2IiBmaWxsPSIjMUExQjJFIi8+PHRleHQgeD0iMTYiIHk9IjIyIiBmb250LXNpemU9IjE4IiB0ZXh0LWFuY2hvcj0ibWlkZGxlIiBmaWxsPSJ3aGl0ZSIgZm9udC1mYW1pbHk9Im1vbm9zcGFjZSI+8J+SuzwvdGV4dD48L3N2Zz4=",
            rdns: RDNS,
          }),
        });

      // EIP-6963: announce provider
      const announce = () =>
        window.dispatchEvent(
          new CustomEvent("eip6963:announceProvider", buildInfo())
        );

      // EIP-6963: providers should listen for the "eip6963:requestProvider"
      // event and respond by announcing. A dApp that registers for
      // requestProvider must receive our announce payload in response.
      // One-way announce alone leaves strict EIP-6963 consumers stranded.
      window.addEventListener("eip6963:requestProvider", () => announce());

      if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", announce);
      } else {
        announce();
      }

      // ── Legacy window.ethereum (MetaMask-compatible) ──
      const provider = {
        isGullbur: true,
        isMetaMask: true,
        chainId: "0x1",
        request: async ({ method, params }) => {
          return new Promise((resolve, reject) => {
            const id = Date.now() + Math.random();
            const message = { type: "ETH_REQUEST", id, method, params };
            window.postMessage(message, "*");
            const handler = (event) => {
              if (event.data?.type === "ETH_RESPONSE" && event.data.id === id) {
                window.removeEventListener("message", handler);
                if (event.data.error) reject(new Error(event.data.error.message || "Unknown error"));
                else resolve(event.data.result);
              }
            };
            window.addEventListener("message", handler);
          });
        },
        on: () => {},
        removeListener: () => {},
      };
      Object.defineProperty(window, "ethereum", {
        value: provider, writable: false, configurable: false,
      });

      // ── Channel B: Next-Gen vault_* API ──
      const vaultProvider = {
        isGullbur: true,
        rdns: RDNS,
        executeBatch: (transactions) => requestVia("vault_executeBatch", { txs: transactions }),
        requestSessionKey: (policy) => requestVia("vault_requestSessionKey", { policy }),
        simulateAndSend: (tx) => requestVia("vault_simulateAndSend", { tx }),
      };
      function requestVia(method, params) {
        return new Promise((resolve, reject) => {
          const id = Date.now() + Math.random();
          window.postMessage({ type: "ETH_REQUEST", id, method, params }, "*");
          const handler = (event) => {
            if (event.data?.type === "ETH_RESPONSE" && event.data.id === id) {
              window.removeEventListener("message", handler);
              if (event.data.error) reject(new Error(event.data.error.message));
              else resolve(event.data.result);
            }
          };
          window.addEventListener("message", handler);
        });
      }
      Object.defineProperty(window, "vault", {
        value: vaultProvider, writable: false, configurable: false,
      });
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
