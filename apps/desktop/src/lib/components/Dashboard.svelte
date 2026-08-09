<script lang="ts">
  import type { Account, NetworkSpec } from '../types';
  import { vault, createAccount, refreshBalances, refreshNetworkBalance, setSelectedNetwork, getAccountLabel, setAccountLabel, getNetworkUnit } from '../vault.svelte.ts';
  import { truncateAddress, formatBalance, getNetworkBadge } from '../utils';
  import { fade } from 'svelte/transition';
  import Send from './Send.svelte';
  import Receive from './Receive.svelte';
  import Portfolio from './Portfolio.svelte';

  let view = $state<'accounts' | 'portfolio'>('accounts');
  let showSend = $state(false);
  let showReceive = $state(false);
  let sendingAccount: Account | null = $state(null);
  let editingLabelAddress: string | null = $state(null);
  let editingLabelValue: string = $state('');
  let creatingAccount = $state(false);

  // Tab indicator position — set on mount and on view change
  let tabBarEl = $state<HTMLDivElement | null>(null);
  let tabIndicator = $state<{ left: number; width: number }>({ left: 0, width: 0 });

  function updateTabIndicator(view: 'accounts' | 'portfolio') {
    requestAnimationFrame(() => {
      if (!tabBarEl) return;
      const btns = tabBarEl.querySelectorAll('button');
      const idx = view === 'accounts' ? 0 : 1;
      const btn = btns[idx] as HTMLElement | undefined;
      if (btn) {
        tabIndicator = { left: btn.offsetLeft, width: btn.offsetWidth };
      }
    });
  }

  $effect(() => {
    updateTabIndicator(view);
  });

  /** Networks filtered by testnet-only mode */
  let filteredNetworks = $derived(
    vault.testnetOnly
      ? vault.networks.filter((n: NetworkSpec) => n.is_testnet)
      : vault.networks
  );

  /** Group networks for optgroup display */
  let mainnetGroup = $derived(filteredNetworks.filter((n: NetworkSpec) => !n.is_testnet));
  let testnetGroup = $derived(filteredNetworks.filter((n: NetworkSpec) => n.is_testnet));

  /** Auto-reset selected network if current choice gets filtered out */
  $effect(() => {
    if (filteredNetworks.length > 0 && !filteredNetworks.find((n) => n.id === vault.selectedNetwork)) {
      vault.selectedNetwork = filteredNetworks[0].id;
    }
  });

  /** Inline account label editing */
  function startEditing(acct: Account) {
    editingLabelAddress = acct.address;
    editingLabelValue = getAccountLabel(acct.address) ?? '';
    setTimeout(() => {
      const el = document.querySelector('.label-input') as HTMLInputElement;
      el?.focus();
      el?.select();
    }, 0);
  }

  function saveLabel(addr: string) {
    if (editingLabelAddress === addr) {
      setAccountLabel(addr, editingLabelValue);
      editingLabelAddress = null;
      editingLabelValue = '';
    }
  }

  function handleLabelKeydown(e: KeyboardEvent, addr: string) {
    if (e.key === 'Enter') {
      e.preventDefault();
      saveLabel(addr);
    } else if (e.key === 'Escape') {
      editingLabelAddress = null;
      editingLabelValue = '';
    }
  }

  function displayName(acct: Account): string {
    return getAccountLabel(acct.address) ?? truncateAddress(acct.address);
  }

  /** Accounts filtered by the selected network */
  let filteredAccounts = $derived(
    vault.accounts.filter((a: Account) => a.network === vault.selectedNetwork)
  );

  /** Next available account index for the selected network */
  let nextIndex = $derived(
    filteredAccounts.length === 0
      ? 0
      : Math.max(...filteredAccounts.map((a: Account) => a.index ?? 0)) + 1
  );

  function onNetworkSelect(e: Event) {
    const sel = e.target as HTMLSelectElement;
    setSelectedNetwork(sel.value);
  }

  async function handleCreateAccount() {
    if (creatingAccount) return;
    creatingAccount = true;
    try { await createAccount(vault.selectedNetwork, nextIndex); }
    catch (err) { console.error('Failed to create account:', err); }
    finally { creatingAccount = false; }
  }

  async function handleRefresh() {
    // Refresh the currently selected network only
    try { await refreshNetworkBalance(vault.selectedNetwork); }
    catch (err) { console.error('Failed to refresh balances:', err); }
  }

  async function handleRefreshAll() {
    try { await refreshBalances(); }
    catch (err) { console.error('Failed to refresh all balances:', err); }
  }

  function openSend(acct: Account) { sendingAccount = acct; showSend = true; }
  function closeSend() { showSend = false; sendingAccount = null; }
  function openReceive() { showReceive = true; }
  function closeReceive() { showReceive = false; }
</script>

<div class="flex flex-col" style="gap: var(--rhythm-lg);">
  <!-- Network Selector -->
  <div class="card">
    <div class="flex items-center justify-between" style="margin-bottom: var(--rhythm-md);">
      <h2 class="text-lg font-semibold">🌐 Networks</h2>
      {#if !vault.connected}
        <span class="text-xs text-red-400 flex items-center gap-1">
          <span class="inline-block w-1.5 h-1.5 rounded-full bg-red-400"></span> Disconnected
        </span>
      {/if}
    </div>
    <select class="input-field w-full" value={vault.selectedNetwork} onchange={onNetworkSelect} disabled={!vault.connected || filteredNetworks.length === 0}>
      {#if filteredNetworks.length === 0}
        <option value="">No networks available</option>
      {:else}
        {#if mainnetGroup.length > 0}
          <optgroup label="Mainnets">
            {#each mainnetGroup as net (net.id)}
              <option value={net.id}>{net.name ?? net.id} ({net.unit ?? net.symbol ?? ''})</option>
            {/each}
          </optgroup>
        {/if}
        {#if testnetGroup.length > 0}
          <optgroup label="Testnets">
            {#each testnetGroup as net (net.id)}
              <option value={net.id}>{net.name ?? net.id} ({net.unit ?? net.symbol ?? ''})</option>
            {/each}
          </optgroup>
        {/if}
      {/if}
    </select>
  </div>

  <!-- Error Banner -->
  {#if vault.error}
    <div class="bg-red-900/30 border border-red-800 rounded-lg px-4 py-3 text-sm text-red-300">⚠️ {vault.error}</div>
  {/if}

  <!-- View Tabs with sliding pill -->
  <div class="relative flex gap-1 p-1 bg-surface/50 rounded-lg mb-2" role="tablist" bind:this={tabBarEl}>
    <div
      class="absolute top-1 bottom-1 rounded-md bg-accent/15 transition-all"
      style="left: {tabIndicator.left}px; width: {tabIndicator.width}px; transition: left 180ms cubic-bezier(0.16, 1, 0.3, 1), width 180ms cubic-bezier(0.16, 1, 0.3, 1);"
    ></div>
    <button
      class="relative flex-1 px-4 py-2 text-sm font-medium rounded-md z-10 transition-colors"
      class:text-accent={view === 'accounts'}
      class:text-secondary={view !== 'accounts'}
      onclick={() => view = 'accounts'}
      role="tab"
      aria-selected={view === 'accounts'}
    >
      📂 Accounts
    </button>
    <button
      class="relative flex-1 px-4 py-2 text-sm font-medium rounded-md z-10 transition-colors"
      class:text-accent={view === 'portfolio'}
      class:text-secondary={view !== 'portfolio'}
      onclick={() => view = 'portfolio'}
      role="tab"
      aria-selected={view === 'portfolio'}
    >
      📊 Portfolio
    </button>
  </div>

  {#if view === 'portfolio'}
    <Portfolio />
  {:else}

  <!-- Accounts List -->
  <div class="card">
    <div class="flex items-center justify-between" style="margin-bottom: var(--rhythm-md);">
      <h2 class="text-lg font-semibold">💰 Accounts</h2>
      <span class="text-xs text-muted">{filteredAccounts.length} account{filteredAccounts.length !== 1 ? 's' : ''}</span>
    </div>

    {#if filteredAccounts.length === 0}
      <div class="text-center py-10">
        <div class="text-4xl mb-3">🪪</div>
        <p class="text-secondary mb-2">No accounts yet — tap "Create Account" to add your first wallet.</p>
        <p class="text-muted text-sm mb-6">Create your first account on {vault.selectedNetwork}.</p>
        <button class="btn-primary" disabled={!vault.connected || creatingAccount} onclick={handleCreateAccount}>
          {creatingAccount ? '⏳ Creating...' : '+ Create Account'}
        </button>
      </div>
    {:else}
      <div style="display: flex; flex-direction: column; gap: var(--rhythm-sm);">
        {#each filteredAccounts as account, i (account.id)}
          {@const badge = getNetworkBadge(account.network)}
          <div class="bg-surface/50 border border-strong rounded-lg p-4 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3" style="animation: fade-up 300ms both; animation-delay: {i * 40}ms;">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 mb-1">
                {#if editingLabelAddress === account.address}
                  <input
                    class="label-input bg-surface-hover border border-vault-500 rounded px-2 py-0.5 text-sm font-mono text-primary w-48 outline-none focus:ring-1 focus:ring-vault-400"
                    type="text"
                    placeholder="Account label..."
                    bind:value={editingLabelValue}
                    onblur={() => saveLabel(account.address)}
                    onkeydown={(e) => handleLabelKeydown(e, account.address)}
                  />
                {:else}
                  <button
                    class="font-mono text-sm text-primary truncate hover:text-vault-400 transition-colors text-left"
                    onclick={() => startEditing(account)}
                    title={account.address}
                  >
                    {displayName(account)}
                  </button>
                {/if}
                <span class="text-xs px-2 py-0.5 rounded-full font-medium {badge.color}">{badge.label}</span>
              </div>
              <div class="text-sm text-secondary">
                {#if account.balanceError}
                  <span class="text-red-400 text-xs">⚠ {account.balanceError}</span>
                {:else}
                  <span class="font-mono text-vault-400">{formatBalance(account.balance)} {getNetworkUnit(account.network)}</span>
                {/if}
              </div>
            </div>
            <div class="flex gap-2 shrink-0">
              <button class="btn-secondary text-sm" onclick={() => openReceive()}>📥 Receive</button>
              <button class="btn-primary text-sm" disabled={!vault.connected} onclick={() => openSend(account)}>Send</button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Quick Actions -->
  <div class="card">
    <h2 class="text-lg font-semibold" style="margin-bottom: var(--rhythm-md);">⚡ Quick Actions</h2>
    <div style="display: flex; flex-wrap: wrap; gap: var(--rhythm-sm);">
      <button class="btn-primary" disabled={!vault.connected || creatingAccount} onclick={handleCreateAccount}>
          {creatingAccount ? '⏳ Creating...' : '+ Create Account'}
        </button>
      <button class="btn-secondary" disabled={!vault.connected} onclick={handleRefresh}>🔄 Refresh</button>
      <button class="btn-ghost" disabled={!vault.connected} onclick={handleRefreshAll}>🔄 Refresh All</button>
    </div>
  </div>

  <!-- Vault Status -->
  <div class="text-xs text-muted text-center">Vault: {vault.vaultStatus} {vault.initialized ? '• Initialized' : '• Not initialized'}</div>
{/if}
</div>

<!-- Send Modal -->
{#if showSend && sendingAccount}
  <Send account={sendingAccount} onclose={closeSend} />
{/if}

<!-- Receive Modal -->
{#if showReceive}
  <Receive onclose={closeReceive} />
{/if}