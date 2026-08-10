<script lang="ts">
  import type { Account, NetworkSpec } from '../types';
  import { vault, createAccount, refreshBalances, refreshNetworkBalance, setSelectedNetwork, getAccountLabel, setAccountLabel, getNetworkUnit } from '../vault.svelte.ts';
  import { truncateAddress, formatBalance, getNetworkBadge } from '../utils';
  import { fade } from 'svelte/transition';
  import { iconHtml } from '../icons';
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

  /** Guard against undefined or null account ids that would cause `each_key_duplicate` */
  let accountKey = $derived((a: Account) => a.id ?? `fallback-${a.network}-${a.index}`);

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

  /** Computed total balance across all accounts */
  let totalBalance = $derived(
    vault.accounts.reduce((s: number, a: Account) => s + parseFloat(a.balance || '0'), 0)
  );
  let formattedTotal = $derived(
    '$' + totalBalance.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  );
</script>

<div class="flex flex-col" style="gap: var(--rhythm-lg);">
  <!-- Global Balance Hero Card -->
  <div class="hero-balance card">
    <span class="text-xs text-muted" style="letter-spacing: 0.05em; text-transform: uppercase; font-weight: 500;">Total Balance</span>
    <div class="hero-amount">
      <span class="text-hero">{formattedTotal}</span>
    </div>
    <div class="flex items-center gap-3" style="margin-top: var(--rhythm-xs);">
      <button class="btn-primary text-sm flex items-center gap-1.5" onclick={() => { const a = vault.accounts[0]; if (a) { sendingAccount = a; showSend = true; } }}>
        {@html iconHtml('send')} Send
      </button>
      <button class="btn-secondary text-sm flex items-center gap-1.5" onclick={openReceive}>
        {@html iconHtml('receive')} Receive
      </button>
    </div>
  </div>

  <!-- Network Selector -->
  <div class="card">
    <div class="flex items-center justify-between" style="margin-bottom: var(--rhythm-md);">
      <h2 class="text-lg font-semibold">{@html iconHtml('globe')} Networks</h2>
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
  <div class="relative flex gap-1 p-1 bg-surface/50 rounded-lg" role="tablist" bind:this={tabBarEl}>
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
      {@html iconHtml('layout')} Accounts
    </button>
    <button
      class="relative flex-1 px-4 py-2 text-sm font-medium rounded-md z-10 transition-colors"
      class:text-accent={view === 'portfolio'}
      class:text-secondary={view !== 'portfolio'}
      onclick={() => view = 'portfolio'}
      role="tab"
      aria-selected={view === 'portfolio'}
    >
      {@html iconHtml('bolt')} Portfolio
    </button>
  </div>

  {#if view === 'portfolio'}
    <Portfolio />
  {:else}

  <!-- Accounts List -->
  <div class="card">
    <div class="flex items-center justify-between" style="margin-bottom: var(--rhythm-md);">
      <h2 class="text-lg font-semibold">{@html iconHtml('wallet')} Accounts</h2>
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
        {#each filteredAccounts as account, i (accountKey(account))}
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
                  <span class="font-mono" style="font-size: var(--text-xl); font-weight: 200; letter-spacing: -0.02em; color: var(--accent);">{formatBalance(account.balance)} <span style="font-size: var(--text-sm); font-weight: 400; color: var(--text-muted);">{getNetworkUnit(account.network)}</span></span>
                {/if}
              </div>
            </div>
            <div class="flex gap-2 shrink-0">
              <button class="btn-secondary text-sm flex items-center gap-1.5" onclick={() => openReceive()}>{@html iconHtml('receive')} Receive</button>
              <button class="btn-primary text-sm flex items-center gap-1.5" disabled={!vault.connected} onclick={() => openSend(account)}>{@html iconHtml('send')} Send</button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Quick Actions -->
  <div class="card">
    <h2 class="text-lg font-semibold" style="margin-bottom: var(--rhythm-md);">{@html iconHtml('bolt')} Quick Actions</h2>
    <div style="display: flex; flex-wrap: wrap; gap: var(--rhythm-sm);">
      <button class="btn-primary flex items-center gap-1.5" disabled={!vault.connected || creatingAccount} onclick={handleCreateAccount}>
          {@html iconHtml('plus')} {creatingAccount ? 'Creating...' : 'Create Account'}
        </button>
      <button class="btn-secondary flex items-center gap-1.5" disabled={!vault.connected} onclick={handleRefresh}>{@html iconHtml('refresh')} Refresh</button>
      <button class="btn-ghost flex items-center gap-1.5" disabled={!vault.connected} onclick={handleRefreshAll}>{@html iconHtml('refresh')} All</button>
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