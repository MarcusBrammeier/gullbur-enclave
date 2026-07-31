<script lang="ts">
  import './app.css';
  import { vault, connect, disconnect } from './lib/vault.svelte.ts';
  import { invoke } from '@tauri-apps/api/core';
  import VaultInit from './lib/components/VaultInit.svelte';
  import Dashboard from './lib/components/Dashboard.svelte';
  import StatusBar from './lib/components/StatusBar.svelte';
  import AuthPrompt from './lib/components/AuthPrompt.svelte';
  import Settings from './lib/components/Settings.svelte';
  import Welcome from './lib/components/Welcome.svelte';

  let showSettings = $state(false);
  let errorMessage = $state<string | null>(null);

  // Hydrate theme from localStorage on mount
  $effect(() => {
    const saved = localStorage.getItem('foss_wallet_theme');
    if (saved === 'light' || saved === 'dark' || saved === 'system') {
      vault.theme = saved;
    }
    applyTheme(vault.theme);
  });

  // Auto-connect to the vault IPC server on mount
  $effect(() => {
    connect().catch((e: unknown) => {
      console.warn('[auto-connect] IPC connection failed:', e);
      // Status shows "Disconnected" — user can tap Connect to retry
    });
  });

  // Watch theme changes
  $effect(() => {
    if (typeof document !== 'undefined') {
      applyTheme(vault.theme);
    }
  });

  function applyTheme(theme: 'light' | 'dark' | 'system') {
    let resolved: string;
    if (theme === 'system') {
      resolved = window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    } else {
      resolved = theme;
    }
    document.documentElement.setAttribute('data-theme', resolved);
    localStorage.setItem('foss_wallet_theme', theme);
  }

  // Global error boundary — catches unhandled async rejections and render errors
  if (typeof window !== 'undefined') {
    window.addEventListener('unhandledrejection', (e: PromiseRejectionEvent) => {
      console.error('[ErrorBoundary] Unhandled rejection:', e.reason);
      errorMessage = e.reason instanceof Error ? e.reason.message : String(e.reason);
    });
    window.addEventListener('error', (e: ErrorEvent) => {
      console.error('[ErrorBoundary] Unhandled error:', e.error ?? e.message);
      errorMessage = e.error instanceof Error ? e.error.message : e.message;
    });
  }

  function dismissError() { errorMessage = null; }

  function handleGlobalKeydown(e: KeyboardEvent) {
    // Escape — close Settings if open
    if (e.key === 'Escape' && showSettings) {
      showSettings = false;
    }
    // Ctrl+Shift+L — lock vault
    if (e.key === 'L' && e.ctrlKey && e.shiftKey) {
      e.preventDefault();
      if (vault.connected) invoke('lock_vault').catch(() => {});
    }
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<Welcome />

<main class="min-h-screen flex flex-col">
  <!-- Header -->
  <header class="border-b border-gray-800 px-6 py-4">
    <div class="max-w-6xl mx-auto flex items-center justify-between">
      <div class="flex items-center gap-3">
        <span class="text-vault-500 text-2xl">🔐</span>
        <h1 class="text-xl font-bold tracking-tight">Gullbúr Enclave</h1>
      </div>
      <div class="flex items-center gap-4">
        <span class="flex items-center gap-2 text-sm">
          <span
            class="inline-block w-2 h-2 rounded-full"
            class:bg-vault-500={vault.connected}
            class:bg-red-500={!vault.connected}
          ></span>
          {vault.vaultStatus}
        </span>
        {#if vault.connected && vault.initialized && vault.authStatus !== 'unauthenticated'}
          <button class="btn-secondary text-sm px-2 py-1" onclick={() => invoke('lock_vault')} title="Lock vault">
            🔒
          </button>
          <button class="btn-secondary text-sm px-2 py-1" onclick={() => showSettings = true} title="Settings">
            ⚙️
          </button>
        {/if}
        {#if !vault.connected}
          <button class="btn-primary text-sm" onclick={connect}>
            Connect to Vault
          </button>
        {:else}
          <button class="btn-secondary text-sm" onclick={disconnect}>
            Disconnect
          </button>
        {/if}
      </div>
    </div>
  </header>

  <!-- Main Content -->
  <section class="flex-1 max-w-6xl mx-auto w-full p-6">
    {#if !vault.initialized}
      <VaultInit />
    {:else}
      <Dashboard />
    {/if}
  </section>

  <!-- Footer -->
  <footer class="border-t border-gray-800 px-6 py-3">
    <div class="max-w-6xl mx-auto">
      <StatusBar />
    </div>
  </footer>
</main>

<!-- Auth overlay (renders above everything) -->
<AuthPrompt />

<!-- Global error banner -->
{#if errorMessage}
<div class="fixed bottom-16 left-1/2 -translate-x-1/2 z-50 max-w-lg w-full mx-4">
  <div class="bg-red-900/90 border border-red-700 rounded-xl px-4 py-3 text-sm text-red-200 shadow-2xl flex items-start gap-3">
    <span>⚠️</span>
    <span class="flex-1">{errorMessage}</span>
    <button class="text-red-400 hover:text-red-200 font-bold" onclick={dismissError}>✕</button>
  </div>
</div>
{/if}

<!-- Settings modal -->
{#if showSettings}
<Settings onclose={() => showSettings = false} />
{/if}