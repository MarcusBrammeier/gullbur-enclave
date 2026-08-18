/**
 * Vault state store — Svelte 5 runes module.
 *
 * Uses object form $state({...}) which is valid for module export in Svelte 5.
 * Components access via `vault.connected`, `vault.accounts`, etc.
 */
import type { IpcClient } from './IpcClient';
import type {
  Account,
  FeeEstimate,
  NetworkSpec,
  VaultStatusResponse,
  VaultInitializeResponse,
  VaultGetBalanceResponse,
  VaultGetTransactionHistoryResponse,
  VaultGenerateMnemonicResponse,
  VaultValidateAddressResponse,
  VaultEstimateFeeResponse,
  VaultSimulateTransferResponse,
  TxRecord,
} from './types';
import { IS_DEMO, VAULT_IPC_PORT } from './constants';
import { pushError } from './toasts.svelte.ts';
import { setAddressBookIpc } from './addressBook';

// ── Reactive state (object form — valid for module export in Svelte 5) ─────

export const vault = $state({
  connected: false,
  initialized: false,
  isDemo: IS_DEMO,
  vaultStatus: 'Disconnected' as string,
  networks: [] as NetworkSpec[],
  accounts: [] as Account[],
  selectedNetwork: '' as string,
  error: null as string | null,
  torEnabled: false,
  testnetOnly: true,
  authStatus: 'unauthenticated' as 'unauthenticated' | 'biometric_unlocked' | 'password_unlocked' | 'hardware_required',
  authTimeout: 30,
  authStartedAt: 0,
  theme: 'dark' as 'light' | 'dark' | 'system',
  accent: 'emerald' as AccentTheme,
  showBetaWarning: false,
});

let client: IpcClient | null = $state(null);

/**
 * Set the vault error message AND surface it as a 3s toast.
 * Centralizes `vault.error = ...` so every catch site gets toast coverage
 * without duplicating the push call.
 */
export function setVaultError(msg: string | unknown): void {
  const text = msg instanceof Error ? msg.message : String(msg ?? '');
  vault.error = text;
  if (text) pushError(text);
}

// Guards against concurrent connect() calls so the auto-connect $effect and a
// manual "Connect to Vault" click can't each spawn their own probe/retry
// sockets at the same time (that stacking caused the "Insufficient resources"
// flood). A single in-flight connect is shared; subsequent callers await it.
let inFlightConnect: Promise<void> | null = null;

// ── Derived values ─────────────────────────────────────────────────────────

export const accountCount = () => vault.accounts.length;
export const networkCount = () => vault.networks.length;
export const isReady = () => vault.connected && vault.initialized;

/** Look up a network spec by its ID from the vault's network list */
export function getNetworkSpec(networkId: string): NetworkSpec | undefined {
  return vault.networks.find((n: NetworkSpec) => n.id === networkId);
}

/** Display unit for a network (e.g. BTC, ETH, XMR) falls back to ID uppercase */
export function getNetworkUnit(networkId: string): string {
  const net = getNetworkSpec(networkId);
  return net?.unit ?? net?.symbol ?? networkId.toUpperCase();
}

// ── Helpers ────────────────────────────────────────────────────────────────

async function getClient(): Promise<IpcClient> {
  if (!client || !vault.connected) {
    throw new Error('Not connected to vault');
  }
  return client;
}

async function refreshStatus(): Promise<void> {
  const c = client;
  if (!c) return;

  const result = (await c.call('vault.status', {})) as VaultStatusResponse;

  vault.initialized = result.initialized ?? false;
  vault.networks = result.networks ?? [];
  vault.accounts = result.accounts ?? [];

  if (result.status) {
    vault.vaultStatus = result.status;
  } else if (result.initialized) {
    vault.vaultStatus = 'Initialized';
  } else {
    vault.vaultStatus = 'Connected';
  }

  if (!vault.selectedNetwork && vault.networks.length > 0) {
    vault.selectedNetwork = vault.networks[0].id;
  }
}

// ── Actions ─────────────────────────────────────────────────────────────────

export async function connect(): Promise<void> {
  if (client && vault.connected) {
    try { await refreshStatus(); } catch { disconnect(); }
    return;
  }

  // De-duplicate concurrent connects: if one is already in flight, await it
  // instead of starting a parallel probe/retry storm.
  if (inFlightConnect) {
    return inFlightConnect;
  }

  vault.error = null;
  vault.vaultStatus = 'Starting IPC server…';

  const run = (async () => {
    try {
    // The vault IPC server is auto-launched during app startup.
    // We just connect directly to the running server.
    let ipcPort = VAULT_IPC_PORT;
    if (IS_DEMO) {
      // Demo mode uses mock — skip real IPC
    }

    // 1. Ensure the IPC server is actually listening by invoking the
    //    Tauri command. Returns the port or errors immediately.
    if (!IS_DEMO) {
          const { invoke } = await import('@tauri-apps/api/core');
          // Retry launch_ipc_server with exponential backoff. The command is
          // idempotent — if the server is already running it returns Ok(port)
          // immediately. This handles the case where a previous process crash
          // left the port in TIME_WAIT and the first bind attempt fails, giving
          // the kernel time to release it on retry.
          const MAX_LAUNCH_RETRIES = 3;
          let lastErr: unknown;
          for (let attempt = 0; attempt < MAX_LAUNCH_RETRIES; attempt++) {
            if (attempt > 0) {
              vault.vaultStatus = `Starting IPC server… (retry ${attempt + 1}/${MAX_LAUNCH_RETRIES})`;
              await new Promise(r => setTimeout(r, 500 * Math.pow(2, attempt)));
            }
            try {
              const port = await Promise.race([
                invoke<number>('launch_ipc_server'),
                new Promise<never>((_, reject) =>
                  setTimeout(() => reject(new Error('launch_ipc_server timed out after 10s')), 10_000)
                ),
              ]);
              ipcPort = port;
              lastErr = undefined;
              break;
            } catch (e) {
              lastErr = e;
            }
          }
          if (lastErr) {
            vault.vaultStatus = 'IPC server failed to start';
            setVaultError(lastErr);
            throw new Error(`IPC server launch failed: ${lastErr instanceof Error ? lastErr.message : String(lastErr)}`);
          }
        }

    // ── 1b. Wait for the IPC server to actually be listening ──────────
    // launch_ipc_server now waits for TcpListener::bind() to complete (v0.0.8
    // oneshot fix) before returning, so by the time we get here the socket is
    // almost always listening. Keep a bounded probe with exponential backoff
    // as a safety net for platform timing (Android), but NEVER blind-flood the
    // loop with one WebSocket per 200ms — that is what made WebKit throw
    // "Insufficient resources" when the server was momentarily not up.
    if (!IS_DEMO) {
      console.log('[vault] Waiting for IPC server to listen...');
      let portReady = false;
      const MAX_PROBE_ATTEMPTS = 6;
      for (let attempt = 0; attempt < MAX_PROBE_ATTEMPTS; attempt++) {
        try {
          const testSock = new WebSocket(`ws://127.0.0.1:${ipcPort}`);
          await new Promise<void>((resolve, reject) => {
            const t = setTimeout(() => { testSock.close(); reject(new Error('timeout')); }, 500);
            testSock.onopen = () => { clearTimeout(t); testSock.close(); resolve(); };
            testSock.onerror = () => { clearTimeout(t); testSock.close(); reject(new Error('error')); };
          });
          portReady = true;
          console.log(`[vault] IPC port ${ipcPort} is listening (attempt ${attempt + 1})`);
          break;
        } catch {
          // Exponential backoff: 200ms, 400ms, 800ms, 1.6s, 3.2s — never tighter.
          await new Promise(r => setTimeout(r, 200 * Math.pow(2, attempt)));
        }
      }
      if (!portReady) {
        throw new Error(`IPC server port ${ipcPort} never became available`);
      }
    }

    // 2. Connect the WebSocket client with retry
    let IpcClientClass: new () => IpcClient;

    if (IS_DEMO) {
      const { MockIpcClient } = await import('./MockIpcClient');
      IpcClientClass = MockIpcClient as unknown as new () => IpcClient;
    } else {
      const { IpcClient: RealIpcClient } = await import('./IpcClient');
      IpcClientClass = RealIpcClient;
    }

    // Retry the WebSocket connection with exponential backoff — the IPC server
    // on Android may not be listening immediately after launch_ipc_server returns.
    let lastError: unknown;
    const maxAttempts = 5;
    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      if (attempt > 0) {
        vault.vaultStatus = `Connecting… (attempt ${attempt + 1}/${maxAttempts})`;
        await new Promise(r => setTimeout(r, 100 * Math.pow(2, attempt)));
      }
      try {
        client = new IpcClientClass();
        await client.connect(ipcPort);
        lastError = undefined;
        break;
      } catch (e) {
        lastError = e;
        client = null;
      }
    }

    if (lastError) throw lastError;

    vault.connected = true;
    vault.vaultStatus = 'Connected';
    await refreshStatus();
    // Wire IPC client into address book for encrypted storage
    setAddressBookIpc(client);
  } catch (e) {
    vault.connected = false;
    vault.vaultStatus = 'Disconnected';
    setVaultError(e);
    console.error('[vault] connect failed:', vault.error);
    throw e; // re-throw so callers like handleGenerate() know it failed
  } finally {
    inFlightConnect = null;
  }
  })();

  inFlightConnect = run;
  return run;
}

export async function initialize(seedPhrase: string, passphrase?: string): Promise<string | null> {
  const c = await getClient();
  vault.error = null;
  vault.vaultStatus = 'Initializing vault…';

  try {
    const result = (await c.call('vault.initialize', { seed_phrase: seedPhrase, passphrase: passphrase ?? '' })) as VaultInitializeResponse;
    if (!result.success) throw new Error('Vault initialization returned failure');
    vault.initialized = true;
    vault.vaultStatus = 'Initialized';
    await refreshStatus();
    return result.mnemonic ?? null;
  } catch (e) {
    vault.vaultStatus = 'Initialization failed';
    setVaultError(e);
    throw e;
  }
}

export async function generateMnemonic(): Promise<string> {
  const c = await getClient();
  vault.error = null;
  try {
    // Stage the fresh phrase in Rust so vault.initialize can consume it without
    // the UI re-sending the seed back over IPC.
    const result = (await c.call('vault.stage_mnemonic', {})) as VaultGenerateMnemonicResponse;
    return result.mnemonic;
  } catch (e) {
    setVaultError(e);
    throw e;
  }
}

/**
 * Clear any staged (Rust-held) mnemonic. Called when the user backs out of
 * the generate flow without initializing, so no seed lingers in Rust memory.
 */
export async function clearStagedMnemonic(): Promise<void> {
  const c = await getClient();
  try {
    await c.call('vault.clear_staged', {});
  } catch { /* best-effort */ }
}

/**
 * Initialize the vault from the Rust-staged mnemonic (generated wallet flow).
 *
 * Sends an EMPTY seed_phrase — the backend consumes the phrase it already holds
 * from the prior `stage_mnemonic` call, so the generated seed is never
 * re-submitted from the UI over IPC.
 */
export async function initializeFromStaged(passphrase?: string): Promise<string | null> {
  const c = await getClient();
  vault.error = null;
  vault.vaultStatus = 'Initializing vault…';
  try {
    const result = (await c.call('vault.initialize', {
      seed_phrase: '',
      passphrase: passphrase ?? '',
    })) as VaultInitializeResponse;
    if (!result.success) throw new Error('Vault initialization returned failure');
    vault.initialized = true;
    vault.vaultStatus = 'Initialized';
    await refreshStatus();
    return result.mnemonic ?? null;
  } catch (e) {
    vault.vaultStatus = 'Initialization failed';
    setVaultError(e);
    throw e;
  }
}

export async function createAccount(network: string, index: number): Promise<Account> {
  const c = await getClient();
  vault.error = null;
  try {
    const result = await c.call('vault.create_account', { network, index }) as Account;
    vault.accounts = [...vault.accounts, result];
    return result;
  } catch (e) {
    setVaultError(e);
    throw e;
  }
}

/** Single-flight guard: coalesces concurrent refreshBalances calls so rapid
 *  triggers (e.g. create-account then refresh) don't fire parallel RPC storms. */
let refreshInFlight: Promise<void> | null = null;

export async function refreshBalances(): Promise<void> {
  // Coalesce: if a refresh is already running, await it instead of duplicating.
  if (refreshInFlight) return refreshInFlight;

  const run = (async () => {
    const c = await getClient();
    vault.error = null;
    try {
      const results = await Promise.allSettled(
        vault.accounts.map(async (acct: Account) => {
          try {
            const result = (await c.call('vault.get_balance', {
              network: acct.network,
              address: acct.address,
            })) as VaultGetBalanceResponse;
            // Clear any previous balance error on success
            return { ...acct, balance: result.balance, balanceError: undefined };
          } catch (err) {
            // Preserve existing balance but mark the error
            return { ...acct, balanceError: err instanceof Error ? err.message : String(err) };
          }
        }),
      );
      const updated: Account[] = [];
      const errors: string[] = [];
      for (const result of results) {
        if (result.status === 'fulfilled') {
          updated.push(result.value);
        } else {
          errors.push(result.reason?.message ?? String(result.reason));
        }
      }
      vault.accounts = updated;
      // Merge successfully refreshed accounts back into the master list
      // without dropping accounts whose balance requests failed.
      const refreshedAddresses = new Set(updated.map((a: Account) => a.address));
      vault.accounts = [
        ...updated,
        ...vault.accounts.filter((a: Account) => !refreshedAddresses.has(a.address)),
      ];
      if (errors.length > 0) {
        console.warn('[refreshBalances] Some networks failed:', errors.join('; '));
      }
    } catch (e) {
      setVaultError(e);
    }
  })();

  refreshInFlight = run;
  try {
    await run;
  } finally {
    refreshInFlight = null;
  }
}

/// Refresh balances for a single network only.
/// Used by the "Refresh" button when a specific network is selected.
export async function refreshNetworkBalance(network: string): Promise<void> {
  const c = await getClient();
  vault.error = null;
  try {
    const networkAccounts = vault.accounts.filter((a: Account) => a.network === network);
    const results = await Promise.allSettled(
      networkAccounts.map(async (acct: Account) => {
        try {
          const result = (await c.call('vault.get_balance', {
            network: acct.network,
            address: acct.address,
          })) as VaultGetBalanceResponse;
          return { ...acct, balance: result.balance, balanceError: undefined };
        } catch (err) {
          return { ...acct, balanceError: err instanceof Error ? err.message : String(err) };
        }
      }),
    );
    const updated: Account[] = [];
    for (const result of results) {
      if (result.status === 'fulfilled') {
        updated.push(result.value);
      }
    }
    // Merge updated accounts back into vault.accounts
    const updatedAddresses = new Set(updated.map((a) => a.address));
    vault.accounts = [
      ...updated,
      ...vault.accounts.filter((a: Account) => a.network !== network || !updatedAddresses.has(a.address)),
    ];
  } catch (e) {
    setVaultError(e);
  }
}

/** Fetch transaction history for a specific account. */
export async function getTransactionHistory(
  address: string,
  network: string,
  limit: number = 50,
): Promise<TxRecord[]> {
  const c = await getClient();
  try {
    const result = (await c.call('vault.get_transaction_history', {
      address,
      network,
      limit,
    })) as VaultGetTransactionHistoryResponse;
    const raw = result.transactions ?? [];
    const unit = getNetworkUnit(network);
    return raw.map((tx) => ({
      ...tx,
      direction: (tx.from?.toLowerCase() === address.toLowerCase()
        ? 'sent'
        : 'received') as 'sent' | 'received',
      unit: tx.unit ?? unit,
    }));
  } catch (e) {
    setVaultError(e);
    return [];
  }
}

/// Clear all accounts for a given network and re-fetch fresh.
/// Used after creating a new account to ensure it shows up immediately.
export async function refreshAccounts(network?: string): Promise<void> {
  const c = await getClient();
  try {
    const result = (await c.call('vault.status', {})) as VaultStatusResponse;
    if (network) {
      // Refresh only accounts for this network
      const networkAccounts = (result.accounts ?? []).filter((a: Account) => a.network === network);
      const otherAccounts = vault.accounts.filter((a: Account) => a.network !== network);
      vault.accounts = [...otherAccounts, ...networkAccounts];
    } else {
      vault.accounts = result.accounts ?? [];
    }
    vault.networks = result.networks ?? [];
  } catch (e) {
    setVaultError(e);
  }
}

export function disconnect(): void {
  client?.disconnect();
  client = null;
  vault.connected = false;
  // Don't reset vault.initialized — that's a property of the vault engine,
  // not the WebSocket connection. Resetting it causes the UI to flash
  // between VaultInit and Dashboard on disconnect.
  vault.vaultStatus = 'Disconnected';
  vault.networks = [];
  vault.accounts = [];
  vault.selectedNetwork = '';
  vault.error = null;
}

export function setSelectedNetwork(value: string): void {
  vault.selectedNetwork = value;
}

export function setTheme(theme: 'light' | 'dark' | 'system'): void {
  vault.theme = theme;
  // Apply immediately — don't rely solely on the $effect in App.svelte,
  // which may not fire if the OptionsBar mounts/unmounts conditionally.
  if (typeof document !== 'undefined') {
    let resolved: string;
    if (theme === 'system') {
      resolved = window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    } else {
      resolved = theme;
    }
    document.documentElement.setAttribute('data-theme', resolved);
    localStorage.setItem('foss_wallet_theme', theme);
  }
}

export type AccentTheme = 'emerald' | 'violet' | 'amber' | 'cyan' | 'rose';

export function getAccentTheme(): AccentTheme {
  if (typeof window === 'undefined') return 'emerald';
  const saved = localStorage.getItem('foss_wallet_accent');
  if (saved === 'violet' || saved === 'amber' || saved === 'cyan' || saved === 'rose') {
    return saved;
  }
  return 'emerald';
}

export function setAccentTheme(accent: AccentTheme): void {
  if (typeof document === 'undefined') return;
  document.documentElement.setAttribute('data-accent', accent);
  localStorage.setItem('foss_wallet_accent', accent);
  // Live-refresh in case the app is already running with a cached value.
  vault.accent = accent;
}

// ── Transaction pipeline actions ───────────────────────────────────────────

export async function validateAddress(address: string, network: string): Promise<boolean> {
  const c = await getClient();
  vault.error = null;
  const result = (await c.call('vault.validate_address', { address, network })) as VaultValidateAddressResponse;
  return result.valid;
}

export async function estimateFee(
  network: string,
  recipient: string,
  amount: string,
): Promise<FeeEstimate[]> {
  const c = await getClient();
  vault.error = null;
  const result = (await c.call('vault.estimate_fee', { network, recipient, amount })) as VaultEstimateFeeResponse;
  return result.estimates ?? [];
}

export async function signTransaction(params: {
  from: string;
  to: string;
  amount: string;
  network: string;
  feeLevel: string;
}): Promise<unknown> {
  const c = await getClient();
  vault.error = null;
  return await c.call('vault.sign_transaction', params);
}

export async function broadcastTransaction(signedTx: unknown): Promise<unknown> {
  const c = await getClient();
  vault.error = null;
  return await c.call('vault.broadcast_transaction', { signed_tx: signedTx });
}

// ── Simulation ────────────────────────────────────────────────────────────

export async function simulateTransfer(
  network: string,
  from: string,
  to: string,
  value: string,
): Promise<VaultSimulateTransferResponse> {
  vault.error = null;
  try {
    // Tauri invoke path (desktop)
    if (!IS_DEMO) {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke('simulate_transfer', { network, from, to, value }) as VaultSimulateTransferResponse;
      return result;
    }
    // Demo mock path
    await new Promise(r => setTimeout(r, 500));
    return {
      success: true,
      gasUsed: 21000,
      gasEstimate: '21000',
      returnData: '0x',
      revertReason: null,
    };
  } catch (e) {
    setVaultError(e);
    return {
      success: false,
      gasUsed: 0,
      gasEstimate: '0',
      returnData: '',
      revertReason: vault.error,
    };
  }
}

// ── Account labels (client-side localStorage) ──────────────────────────────

const LABEL_KEY = 'foss_wallet_labels';

function loadLabels(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(LABEL_KEY) ?? '{}');
  } catch { return {}; }
}

function saveLabels(labels: Record<string, string>): void {
  localStorage.setItem(LABEL_KEY, JSON.stringify(labels));
}

export function getAccountLabel(address: string): string | null {
  return loadLabels()[address] ?? null;
}

export function setAccountLabel(address: string, label: string): void {
  const labels = loadLabels();
  if (label.trim()) {
    labels[address] = label.trim();
  } else {
    delete labels[address];
  }
  saveLabels(labels);
  // Update in-memory account objects
  for (const acct of vault.accounts) {
    if (acct.address === address) {
      acct.label = labels[address] ?? undefined;
    }
  }
}