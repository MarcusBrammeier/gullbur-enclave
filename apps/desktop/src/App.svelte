<script lang="ts">
  import './app.css';
  import { vault, connect, disconnect } from './lib/vault.svelte.ts';
  import { pushError } from './lib/toasts.svelte.ts';
  import { invoke } from '@tauri-apps/api/core';
  import { themeEngine } from './lib/themeEngine.svelte.ts';
  import VaultInit from './lib/components/VaultInit.svelte';
  import Dashboard from './lib/components/Dashboard.svelte';
  import StatusBar from './lib/components/StatusBar.svelte';
  import AuthPrompt from './lib/components/AuthPrompt.svelte';
  import Settings from './lib/components/Settings.svelte';
  import OptionsBar from './lib/components/OptionsBar.svelte';
  import Toasts from './lib/components/Toasts.svelte';

  let showSettings = $state(false);
  let errorMessage = $state<string | null>(null);
  let connecting = $state(false);

  // Hydrate theme & options on mount
  $effect(() => {
    const saved = localStorage.getItem('foss_wallet_theme');
    if (saved === 'light') {
      vault.theme = 'light';
    } else if (saved === 'system') {
      vault.theme = 'system';
    }
    applyTheme(vault.theme);
  });

  // Auto-connect to the vault IPC server on mount
  $effect(() => {
    connecting = true;
    vault.vaultStatus = 'Connecting…';
    connect()
      .then(() => { connecting = false; })
      .catch((e: unknown) => {
        connecting = false;
        console.warn('[auto-connect] IPC connection failed:', e);
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
      resolved = typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    } else {
      resolved = theme;
    }
    
    // Delegate to themeEngine for full token injection + fallback safety
    if (resolved === 'dark') {
      themeEngine.applyTheme('dark-slate');
    } else {
      themeEngine.applyTheme('light-slate');
    }

    document.documentElement.setAttribute('data-theme', resolved);
    localStorage.setItem('foss_wallet_theme', theme);
  }

  // Global error boundary — catches unhandled async rejections and render errors
  if (typeof window !== 'undefined') {
    window.addEventListener('unhandledrejection', (e: PromiseRejectionEvent) => {
      console.error('[ErrorBoundary] Unhandled rejection:', e.reason);
      const msg = e.reason instanceof Error ? e.reason.message : String(e.reason);
      errorMessage = msg;
      pushError(msg);
    });
    window.addEventListener('error', (e: ErrorEvent) => {
      console.error('[ErrorBoundary] Unhandled error:', e.error ?? e.message);
      const msg = e.error instanceof Error ? e.error.message : e.message;
      errorMessage = msg;
      pushError(msg);
    });
  }

  function dismissError() { errorMessage = null; }

  function handleGlobalKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && showSettings) {
      showSettings = false;
    }
    if (e.key === 'L' && e.ctrlKey && e.shiftKey) {
      e.preventDefault();
      if (vault.connected) invoke('lock_vault').catch(() => {});
    }
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<main class="min-h-screen flex flex-col">
  <!-- Global toast notifications -->
  <Toasts />

  <!-- Header -->
  <header class="border-b border-default px-4 py-3 sm:px-6 sm:py-4">
    <div class="max-w-6xl mx-auto flex flex-wrap items-center gap-x-3 gap-y-2">
      <div class="flex items-center gap-3 shrink-0">
        <span class="text-vault-500 text-2xl">🔐</span>
        <h1 class="text-lg sm:text-xl font-bold tracking-tight">Gullbúr Enclave</h1>
      </div>
      <div class="hidden sm:block flex-1" aria-hidden="true"></div>
      {#if vault.connected}
        <div class="w-full sm:w-auto order-last sm:order-none">
          <OptionsBar />
        </div>
      {/if}
      <div class="flex items-center gap-2 sm:gap-4 ml-auto">
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
          <button class="btn-primary text-sm" disabled={connecting} onclick={connect}>
            {connecting ? 'Connecting…' : 'Connect to Vault'}
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
  <section class="flex-1 max-w-6xl mx-auto w-full px-4 py-5 sm:p-6 overflow-y-auto">
    {#if !vault.initialized}
      <VaultInit />
    {:else}
      <Dashboard />
    {/if}
  </section>

  <!-- Footer -->
  <footer class="border-t border-default px-4 py-3 sm:px-6">
    <div class="max-w-6xl mx-auto">
      <StatusBar />
    </div>
  </footer>
</main>

<!-- Auth overlay -->
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
