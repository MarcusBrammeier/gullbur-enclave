<script lang="ts">
  import { vault, getAccountLabel } from '../vault.svelte.ts';
  import { fade, scale } from 'svelte/transition';
  // qrcode has no TS types — runtime import only

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let selectedNetwork = $state(vault.selectedNetwork || (vault.networks[0]?.id ?? ''));
  let selectedAccountIdx = $state(0);
  let copiedAddr = $state<string | null>(null);

  let filteredAccounts = $derived(
    vault.accounts.filter((a) => a.network === selectedNetwork)
  );

  let currentAccount = $derived(filteredAccounts[selectedAccountIdx] ?? filteredAccounts[0] ?? null);

  let qrDataUrl = $state<string | null>(null);

  $effect(() => {
    if (!currentAccount) { qrDataUrl = null; return; }
    import('qrcode').then((mod) => {
      mod.toDataURL(currentAccount.address, {
        width: 256,
        margin: 2,
        color: { dark: '#fbbf24', light: '#00000000' },
      }).then((url: string) => { qrDataUrl = url; }).catch(() => { qrDataUrl = null; });
    }).catch(() => { qrDataUrl = null; });
  });

  function handleNetworkChange() {
    selectedNetwork = (document.getElementById('receive-network-select') as HTMLSelectElement)?.value;
  }

  async function copyAddress() {
    if (!currentAccount) return;
    try {
      await navigator.clipboard.writeText(currentAccount.address);
      copiedAddr = currentAccount.address;
      setTimeout(() => { if (copiedAddr === currentAccount!.address) copiedAddr = null; }, 2000);
    } catch {
      const ta = document.createElement('textarea');
      ta.value = currentAccount.address;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
      copiedAddr = currentAccount.address;
      setTimeout(() => { if (copiedAddr === currentAccount!.address) copiedAddr = null; }, 2000);
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onclose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onclose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="fixed inset-0 z-40 flex items-center justify-center bg-black/60 backdrop-blur-sm"
  transition:fade={{ duration: 150 }}
  role="dialog"
  aria-modal="true"
  aria-label="Receive funds"
  tabindex="-1"
  onclick={handleBackdropClick}
  onkeydown={(e) => { if (e.key === 'Escape') onclose(); }}
>
  <div class="bg-surface-dim border border-default rounded-2xl shadow-2xl max-w-sm w-full mx-4 p-6"
    transition:scale={{ start: 0.97, duration: 180 }}
    role="document">
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold">Receive</h2>
      <button class="text-muted hover:text-primary text-xl leading-none" onclick={onclose}>&times;</button>
    </div>

    <!-- Network selector -->
    <select id="receive-network-select" class="input-field w-full mb-4" value={selectedNetwork} onchange={handleNetworkChange}>
      {#each vault.networks as net (net.id)}
        <option value={net.id}>{net.name ?? net.id}</option>
      {/each}
    </select>

    <!-- Account selector (multi-account networks) -->
    {#if filteredAccounts.length > 1}
      <select class="input-field w-full mb-4" bind:value={selectedAccountIdx}>
        {#each filteredAccounts as acct, i}
          <option value={i}>{getAccountLabel(acct.address) ?? acct.address.slice(0, 10) + '…'}</option>
        {/each}
      </select>
    {/if}

    {#if currentAccount}
      <div class="flex flex-col items-center">
        <!-- QR code -->
        <div class="bg-black rounded-xl p-3 mb-4">
          {#if qrDataUrl}
            <img src={qrDataUrl} alt="Address QR code" class="w-48 h-48" />
          {:else}
            <div class="w-48 h-48 flex items-center justify-center text-muted text-sm">Loading QR…</div>
          {/if}
        </div>

        <!-- Address -->
        <div class="w-full bg-surface rounded-lg px-3 py-2 font-mono text-xs text-primary break-all text-center mb-4">
          {currentAccount.address}
        </div>

        <!-- Actions -->
        <div class="flex gap-3 w-full">
          <button class="btn-primary flex-1 text-sm flex items-center justify-center gap-2" onclick={copyAddress}>
            {copiedAddr === currentAccount.address ? '✅ Copied!' : '📋 Copy Address'}
          </button>
        </div>
      </div>
    {:else}
      <div class="text-center py-8">
        <p class="text-muted text-sm">No account on {selectedNetwork}.</p>
        <p class="text-muted text-xs mt-1">Create an account first from the Dashboard.</p>
      </div>
    {/if}
  </div>
</div>