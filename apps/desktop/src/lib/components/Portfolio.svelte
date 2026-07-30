<script lang="ts">
  import type { Account, NetworkSpec } from '../types';
  import { vault, refreshBalances } from '../vault.svelte.ts';
  import TransactionHistory from './TransactionHistory.svelte';

  let selectedAccountId = $state<string | null>(null);
  let refreshing = $state(false);

  /** All accounts grouped by network */
  let accountsByNetwork = $derived.by(() => {
    const map = new Map<string, Account[]>();
    for (const acct of vault.accounts) {
      const list = map.get(acct.network) ?? [];
      list.push(acct);
      map.set(acct.network, list);
    }
    return map;
  });

  /** Total accounts count */
  let totalAccounts = $derived(vault.accounts.length);

  /** Selected account object */
  let selectedAccount = $derived(
    selectedAccountId
      ? vault.accounts.find((a) => `${a.network}-${a.address}` === selectedAccountId) ?? null
      : null
  );

  function getNetworkSpec(networkId: string): NetworkSpec | undefined {
    return vault.networks.find((n: NetworkSpec) => n.id === networkId);
  }

  function getNetworkBadge(networkId: string): { label: string; color: string } {
    switch (networkId) {
      case 'bitcoin':
      case 'bitcoin-testnet':   return { label: 'BTC', color: 'bg-orange-600 text-orange-100' };
      case 'litecoin':
      case 'litecoin-testnet':  return { label: 'LTC', color: 'bg-silver-500 text-gray-900' };
      case 'monero':
      case 'monero-stagenet':   return { label: 'XMR', color: 'bg-orange-500 text-orange-100' };
      case 'ethereum':
      case 'sepolia':            return { label: 'ETH', color: 'bg-blue-600 text-blue-100' };
      case 'polygon':            return { label: 'POL', color: 'bg-purple-600 text-purple-100' };
      case 'arbitrum':           return { label: 'ARB', color: 'bg-sky-600 text-sky-100' };
      case 'base':               return { label: 'BASE', color: 'bg-blue-500 text-blue-100' };
      case 'optimism':           return { label: 'OP', color: 'bg-red-500 text-red-100' };
      case 'bnb':                return { label: 'BNB', color: 'bg-yellow-500 text-yellow-100' };
      default:                   return { label: networkId.toUpperCase(), color: 'bg-gray-600 text-gray-100' };
    }
  }

  function truncateAddress(addr: string): string {
    if (addr.length <= 12) return addr;
    return `${addr.slice(0, 8)}...${addr.slice(-6)}`;
  }

  function formatBalance(balance: { confirmed: string; unconfirmed?: string } | null): string {
    if (!balance) return '0';
    const val = parseFloat(balance.confirmed);
    if (isNaN(val)) return '0';
    return val.toLocaleString(undefined, { maximumFractionDigits: 8 });
  }

  function getNetworkUnit(networkId: string): string {
    const net = getNetworkSpec(networkId);
    return net?.symbol ?? networkId.toUpperCase();
  }

  function networkIcon(networkId: string): string {
    if (networkId.includes('bitcoin')) return '₿';
    if (networkId.includes('monero')) return 'ɱ';
    if (networkId.includes('litecoin')) return 'Ł';
    return '◆';
  }

  async function handleRefresh() {
    refreshing = true;
    try { await refreshBalances(); } catch {}
    refreshing = false;
  }
</script>

<div class="flex flex-col gap-6">
  <!-- Portfolio Summary -->
  <div class="card">
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold">📊 Portfolio</h2>
      <span class="text-xs text-gray-500">{totalAccounts} account{totalAccounts !== 1 ? 's' : ''}</span>
    </div>

    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
      {#each Array.from(accountsByNetwork.entries()) as [networkId, accts]}
        {@const net = getNetworkSpec(networkId)}
        {@const badge = getNetworkBadge(networkId)}
        {@const totalBalance = accts.reduce((sum, a) => {
          const val = parseFloat(a.balance?.confirmed ?? '0');
          return sum + (isNaN(val) ? 0 : val);
        }, 0)}
        <div class="bg-gray-800/40 border border-gray-700 rounded-xl p-4 hover:border-gray-600 transition-colors">
          <div class="flex items-center gap-2 mb-2">
            <span class="text-lg">{networkIcon(networkId)}</span>
            <span class="text-sm font-medium text-gray-200">{net?.name ?? networkId}</span>
            <span class="text-xs px-1.5 py-0.5 rounded-full font-medium {badge.color}">{badge.label}</span>
          </div>
          <div class="text-xl font-mono font-bold text-vault-400">
            {totalBalance.toLocaleString(undefined, { maximumFractionDigits: 4 })} {net?.symbol ?? badge.label}
          </div>
          <div class="text-xs text-gray-500 mt-1">
            {accts.length} account{accts.length !== 1 ? 's' : ''}
          </div>
        </div>
      {/each}
    </div>

    {#if accountsByNetwork.size === 0}
      <div class="text-center py-8 text-gray-500 text-sm">
        No accounts yet. Create one from the Dashboard.
      </div>
    {/if}
  </div>

  <!-- All Accounts Detail -->
  <div class="card">
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold">🔑 All Accounts</h2>
      <button
        class="btn-secondary text-xs px-3 py-1.5"
        disabled={refreshing || !vault.connected}
        onclick={handleRefresh}
      >
        {refreshing ? '🔄 Refreshing...' : '🔄 Refresh'}
      </button>
    </div>

    {#if vault.accounts.length === 0}
      <div class="text-center py-8">
        <div class="text-4xl mb-3">🪪</div>
        <p class="text-gray-500 text-sm">No accounts yet.</p>
      </div>
    {:else}
      <div class="space-y-2">
        {#each vault.accounts as account (account.address)}
          {@const badge = getNetworkBadge(account.network)}
          {@const isSelected = selectedAccountId === `${account.network}-${account.address}`}
          <button
            class="w-full text-left bg-gray-800/30 border rounded-lg p-3 flex items-center justify-between gap-3 hover:bg-gray-800/50 hover:border-gray-600 transition-colors {isSelected ? 'border-vault-500/50 bg-gray-800/60' : 'border-gray-700/50'}"
            onclick={() => selectedAccountId = isSelected ? null : `${account.network}-${account.address}`}
          >
            <div class="flex items-center gap-3 min-w-0">
              <span class="text-lg shrink-0">{networkIcon(account.network)}</span>
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span class="font-mono text-sm text-gray-200 truncate">{truncateAddress(account.address)}</span>
                  <span class="text-xs px-1.5 py-0.5 rounded-full font-medium shrink-0 {badge.color}">{badge.label}</span>
                </div>
                <div class="text-xs text-gray-500 mt-0.5">
                  Path: {account.path ?? 'BIP-44'}
                </div>
              </div>
            </div>
            <div class="text-right shrink-0">
              <div class="font-mono text-sm font-medium text-vault-400">
                {formatBalance(account.balance)} {getNetworkUnit(account.network)}
              </div>
              {#if account.balance?.unconfirmed && parseFloat(account.balance.unconfirmed) > 0}
                <div class="text-xs text-yellow-400">+{account.balance.unconfirmed} pending</div>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Transaction History for Selected Account -->
  {#if selectedAccount}
    <div class="card">
      <div class="flex items-center justify-between mb-2">
        <h2 class="text-lg font-semibold">📋 Transaction History</h2>
        <button class="text-xs text-gray-500 hover:text-gray-300" onclick={() => selectedAccountId = null}>
          ✕ Close
        </button>
      </div>
      <p class="text-xs text-gray-500 mb-4 font-mono truncate">{selectedAccount.address}</p>
      <TransactionHistory transactions={[]} loading={false} />
      <p class="text-xs text-gray-600 text-center mt-4">
        Transaction history requires view-key scanning (Phase 2).
      </p>
    </div>
  {/if}
</div>
