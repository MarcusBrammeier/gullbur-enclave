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

// ── Reactive state (object form — valid for module export in Svelte 5) ─────

export const vault = $state({
  connected: false,
  initialized: false,
  vaultStatus: 'Disconnected' as string,
  networks: [] as NetworkSpec[],
  accounts: [] as Account[],
  selectedNetwork: '' as string,
  error: null as string | null,
  torEnabled: false,
  testnetOnly: true,
  authStatus: 'unauthenticated' as 'unauthenticated' | 'biometric_unlocked' | 'hardware_required',
  authTimeout: 30,
  authStartedAt: 0,
  theme: 'dark' as 'light' | 'dark' | 'system',
  showBetaWarning: false,
});

let client: IpcClient | null = $state(null);

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

  vault.error = null;
  vault.vaultStatus = 'Connecting…';

  try {
    // The vault IPC server is auto-launched during app startup.
    // We just connect directly to the running server.
    let ipcPort = VAULT_IPC_PORT;
    if (IS_DEMO) {
      // Demo mode uses mock — skip real IPC
    }

    // 1. Connect the WebSocket client with retry
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
    const maxAttempts = 10;
    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      if (attempt > 0) {
        vault.vaultStatus = `Connecting… (attempt ${attempt + 1}/${maxAttempts})`;
        await new Promise(r => setTimeout(r, 200 * Math.pow(1.5, attempt)));
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
  } catch (e) {
    vault.connected = false;
    vault.vaultStatus = 'Disconnected';
    vault.error = e instanceof Error ? e.message : String(e);
    console.error('[vault] connect failed:', vault.error);
    throw e; // re-throw so callers like handleGenerate() know it failed
  }
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
    vault.error = e instanceof Error ? e.message : String(e);
    throw e;
  }
}

export async function generateMnemonic(): Promise<string> {
  const c = await getClient();
  vault.error = null;
  try {
    const result = (await c.call('vault.generate_mnemonic', {})) as VaultGenerateMnemonicResponse;
    return result.mnemonic;
  } catch (e) {
    vault.error = e instanceof Error ? e.message : String(e);
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
    vault.error = e instanceof Error ? e.message : String(e);
    throw e;
  }
}

export async function refreshBalances(): Promise<void> {
  const c = await getClient();
  vault.error = null;
  try {
    // Use allSettled so a single network error doesn't cascade to all networks
    const results = await Promise.allSettled(
      vault.accounts.map(async (acct: Account) => {
        const result = (await c.call('vault.get_balance', {
          network: acct.network,
          address: acct.address,
        })) as VaultGetBalanceResponse;
        return { ...acct, balance: result.balance };
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
      // Show a more specific message — networks like XMR without wallet-rpc
      // are expected to silently return zero now.
      const nonZeroErrors = errors.filter(e => !e.includes('0 XMR') && !e.includes('wallet-rpc'));
      if (nonZeroErrors.length > 0) {
        vault.error = `${nonZeroErrors.length} network(s) failed to refresh`;
      }
    }
  } catch (e) {
    vault.error = e instanceof Error ? e.message : String(e);
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
        const result = (await c.call('vault.get_balance', {
          network: acct.network,
          address: acct.address,
        })) as VaultGetBalanceResponse;
        return { ...acct, balance: result.balance };
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
    vault.error = e instanceof Error ? e.message : String(e);
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
    vault.error = e instanceof Error ? e.message : String(e);
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
    vault.error = e instanceof Error ? e.message : String(e);
  }
}

export function disconnect(): void {
  client?.disconnect();
  client = null;
  vault.connected = false;
  vault.initialized = false;
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
    vault.error = e instanceof Error ? e.message : String(e);
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