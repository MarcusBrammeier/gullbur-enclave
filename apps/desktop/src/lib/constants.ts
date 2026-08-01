/**
 * Application constants for Gullbúr Enclave Core.
 */

/** Demo mode flag — true when running in browser demo mode (no Tauri IPC) */
export const IS_DEMO: boolean =
  typeof window !== 'undefined' && (window as any).__DEMO__ === true;

/** Default IPC port for vault-core WebSocket server */
export const VAULT_IPC_PORT = 19876;
