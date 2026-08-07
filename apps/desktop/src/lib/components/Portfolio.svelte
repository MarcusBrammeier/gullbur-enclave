<script lang="ts">
  import type { Account, TxRecord } from '../types';
  import { vault, refreshBalances, getTransactionHistory, getNetworkSpec, getNetworkUnit } from '../vault.svelte.ts';
  import { truncateAddress, formatBalance, getNetworkBadge, networkIcon } from '../utils';
  import TransactionHistory from './TransactionHistory.svelte';

  let selectedAccountId = $state<string | null>(null);
  let refreshing = $state(false);
  let txHistory = $state<TxRecord[]>([]);
  let txLoading = $state(false);

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

  /** Fetch tx history when selected account changes */
  $effect(() => {
    const acct = selectedAccount;
    if (acct) {
      txLoading = true;
      txHistory = [];
      getTransactionHistory(acct.address, acct.network).then((txs) => {
        txHistory = txs;
        txLoading = false;
      }).catch(() => {
        txLoading = false;
      });
    } else {
      txHistory = [];
      txLoading = false;
    }
  });

  async function handleRefresh() {
    refreshing = true;
    try { await refreshBalances(); } catch {}
    // Re-fetch tx history if an account is selected
    const acct = selectedAccount;
    if (acct) {
      txLoading = true;
      try {
        const txs = await getTransactionHistory(acct.address, acct.network);
        txHistory = txs;
      } catch {}
      txLoading = false;
    }
    refreshing = false;
  }
</script>

<div class="flex flex-col gap-6">
  <!-- Portfolio Summary -->
  <div class="card">
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold">📊 Portfolio</h2>
      <span class="text-xs text-muted">{totalAccounts} account{totalAccounts !== 1 ? 's' : ''}</span>
    </div>

    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
      {#each Array.from(accountsByNetwork.entries()) as [networkId, accts]}
        {@const net = getNetworkSpec(networkId)}
        {@const badge = getNetworkBadge(networkId)}
        {@const totalBalance = accts.reduce((sum, a) => {
          const val = parseFloat(a.balance?.confirmed ?? '0');
          return sum + (isNaN(val) ? 0 : val);
        }, 0)}
        <div class="bg-surface/40 border border-strong rounded-xl p-4 hover:border-hover transition-colors">
          <div class="flex items-center gap-2 mb-2">
            <span class="text-lg">{networkIcon(networkId)}</span>
            <span class="text-sm font-medium text-primary">{net?.name ?? networkId}</span>
            <span class="text-xs px-1.5 py-0.5 rounded-full font-medium {badge.color}">{badge.label}</span>
          </div>
          <div class="text-xl font-mono font-bold text-vault-400">
            {totalBalance.toLocaleString(undefined, { maximumFractionDigits: 4 })} {net?.symbol ?? badge.label}
          </div>
          <div class="text-xs text-muted mt-1">
            {accts.length} account{accts.length !== 1 ? 's' : ''}
          </div>
        </div>
      {/each}
    </div>

    {#if accountsByNetwork.size === 0}
      <div class="text-center py-8 text-muted text-sm">
        No accounts yet. Create one from the Accounts tab.
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
        <p class="text-muted text-sm">No accounts yet.</p>
      </div>
    {:else}
      <div class="space-y-2">
        {#each vault.accounts as account (account.id)}
          {@const badge = getNetworkBadge(account.network)}
          {@const isSelected = selectedAccountId === `${account.network}-${account.address}`}
          <button
            class="w-full text-left bg-surface/30 border rounded-lg p-3 flex items-center justify-between gap-3 hover:bg-surface/50 hover:border-hover transition-colors {isSelected ? 'border-vault-500/50 bg-surface/60' : 'border-strong/50'}"
            onclick={() => selectedAccountId = isSelected ? null : `${account.network}-${account.address}`}
          >
            <div class="flex items-center gap-3 min-w-0">
              <span class="text-lg shrink-0">{networkIcon(account.network)}</span>
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span class="font-mono text-sm text-primary truncate">{truncateAddress(account.address)}</span>
                  <span class="text-xs px-1.5 py-0.5 rounded-full font-medium shrink-0 {badge.color}">{badge.label}</span>
                </div>
                <div class="text-xs text-muted mt-0.5">
                  Path: {account.path ?? 'BIP-44'}
                </div>
              </div>
            </div>
            <div class="text-right shrink-0">
              <div class="font-mono text-sm font-medium text-vault-400">
                {account.balanceError
                  ? '⚠' 
                  : formatBalance(account.balance)} {account.balanceError ? '' : getNetworkUnit(account.network)}
              </div>
              {#if account.balanceError}
                <div class="text-xs text-red-400 max-w-[200px] truncate" title={account.balanceError}>balance error</div>
              {:else if account.balance?.unconfirmed && parseFloat(account.balance.unconfirmed) > 0}
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
        <button class="text-xs text-muted hover:text-primary" onclick={() => selectedAccountId = null}>
          ✕ Close
        </button>
      </div>
      <p class="text-xs text-muted mb-4 font-mono truncate">{selectedAccount.address}</p>
      <TransactionHistory transactions={txHistory} loading={txLoading} />
    </div>
  {/if}
</div>
