<script lang="ts">
  import { vault } from '../vault.svelte.ts';
  import { iconHtml } from '../icons';

  let password = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);

  async function unlockWithPassword() {
    if (busy || !password) return;
    busy = true;
    errorMsg = null;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('password_unlock_vault', { passphrase: password });
      vault.authStatus = 'password_unlocked';
      password = '';
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function unlockWithBiometric() {
    if (busy) return;
    busy = true;
    errorMsg = null;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('biometric_unlock_vault');
      vault.authStatus = 'biometric_unlocked';
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') void unlockWithPassword();
  }
</script>

<div
  class="min-h-screen flex items-center justify-center p-4"
  role="dialog"
  aria-modal="true"
  aria-label="Unlock wallet"
>
  <div class="w-full max-w-md bg-surface-dim border border-strong rounded-2xl shadow-2xl p-8">
    <div class="text-center mb-6">
      <div class="inline-flex items-center justify-center mb-3">{@html iconHtml('lock', 'w-9 h-9 text-amber-400')}</div>
      <h1 class="text-xl font-semibold text-primary mb-1">Wallet Locked</h1>
      <p class="text-sm text-secondary">Unlock with your passphrase or fingerprint to continue.</p>
    </div>

    <form
      class="space-y-3"
      onsubmit={(e) => { e.preventDefault(); void unlockWithPassword(); }}
    >
      <input
        type="password"
        class="w-full text-sm bg-surface border border-strong/50 rounded-lg px-3 py-2.5 text-primary placeholder:text-muted outline-none focus:border-accent focus:ring-1 focus:ring-accent"
        placeholder="Enter your passphrase"
        bind:value={password}
        onkeydown={handleKeydown}
        autocomplete="current-password"
      />

      {#if errorMsg}
        <div class="p-3 bg-red-900/30 border border-red-800 rounded-lg">
          <p class="text-xs font-mono text-red-400 break-words">{errorMsg}</p>
        </div>
      {/if}

      <button
        class="w-full px-4 py-2.5 rounded-lg text-sm font-medium text-white bg-accent hover:bg-accent/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        type="submit"
        disabled={busy || !password}
      >
        {busy ? 'Unlocking…' : 'Unlock with Password'}
      </button>
    </form>

    <div class="flex items-center gap-2 my-4">
      <span class="flex-1 h-px bg-strong/40"></span>
      <span class="text-xs text-muted uppercase tracking-wider">or</span>
      <span class="flex-1 h-px bg-strong/40"></span>
    </div>

    <button
      class="w-full px-4 py-2.5 rounded-lg text-sm font-medium bg-surface hover:bg-surface-hover text-primary border border-strong/50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      onclick={unlockWithBiometric}
      disabled={busy}
    >
      {@html iconHtml('fingerprint', 'w-4 h-4 inline-block mr-1.5')}Unlock with Fingerprint
    </button>
  </div>
</div>