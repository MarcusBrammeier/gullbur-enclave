/**
 * Application constants for Gullbúr Enclave Core.
 */

/** Demo mode flag — true when running in browser demo mode (no Tauri IPC).
 *  Set via URL param `?demo=true`, `window.__DEMO__=true`, or localStorage.
 *  The banner check reads `__DEMO__` so toggling devtools works instantly. */
export const IS_DEMO: boolean =
  typeof window !== 'undefined' &&
  (window as any).__DEMO__ === true;

/** Boot demo mode on page load if URL param or localStorage is set. */
if (typeof window !== 'undefined' && !(window as any).__DEMO__) {
  const urlParams = new URLSearchParams(window.location.search);
  if (urlParams.get('demo') === 'true' || localStorage.getItem('gullbur_demo') === 'true') {
    (window as any).__DEMO__ = true;
  }
}

/** Default IPC port for vault-core WebSocket server */
export const VAULT_IPC_PORT = 19876;
