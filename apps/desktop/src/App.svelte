<script lang="ts">
  import './app.css';
  import { vault, connect, disconnect, networkCount } from './lib/vault.svelte.ts';
  import { pushError } from './lib/toasts.svelte.ts';
  import { invoke } from '@tauri-apps/api/core';
  import { themeEngine } from './lib/themeEngine.svelte.ts';
  import type { AccentTheme } from './lib/vault.svelte.ts';
  import VaultInit from './lib/components/VaultInit.svelte';
  import Dashboard from './lib/components/Dashboard.svelte';
  import StatusBar from './lib/components/StatusBar.svelte';
  import AuthPrompt from './lib/components/AuthPrompt.svelte';
  import Settings from './lib/components/Settings.svelte';
  import Toasts from './lib/components/Toasts.svelte';
  import ConsoleLog from './lib/components/ConsoleLog.svelte';
  import DebugReport from './lib/components/DebugReport.svelte';
  import { iconHtml } from './lib/icons';
  import { IS_DEMO } from './lib/constants';

  let showSettings = $state(false);
  let errorMessage = $state<string | null>(null);
  let connecting = $state(false);

  // Console / Debug report state (for compact sidebar status bar)
  let showConsole = $state(false);
  let showDebugReport = $state(false);

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

  // ── Theme / Accent selector helpers (pulled from OptionsBar patterns) ───

  const builtinThemeIds = ['obsidian', 'dark-slate', 'light-studio'] as const;
  type BuiltinThemeId = (typeof builtinThemeIds)[number];
  type IconName = Parameters<typeof iconHtml>[0];
  const themeIcons: Record<BuiltinThemeId, IconName> = {
    obsidian: 'moon',
    'dark-slate': 'moon',
    'light-studio': 'sun',
  };

  function getThemeIcon(id: BuiltinThemeId): string {
    return iconHtml(themeIcons[id], 'w-3.5 h-3.5');
  }

  function handleThemeChange(id: BuiltinThemeId) {
    themeEngine.applyTheme(id);
    const isDark = id !== 'light-studio';
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
    localStorage.setItem('gullbur_theme', id);
  }

  function handleAccentChange(accent: AccentTheme) {
    vault.accent = accent;
    themeEngine.setAccent(accent);
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<!-- Demo mode warning banner — full width, above sidebar and main -->
{#if IS_DEMO}
  <div class="bg-amber-600/20 border-b border-amber-600/30 px-4 py-1.5 text-xs text-amber-400 text-center flex items-center justify-center gap-2 sticky top-0 z-50" style="backdrop-filter: blur(8px);">
    <span class="inline-block w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>
    <span><strong>GUI Test Mode</strong> — Core engine not connected. Mock data for visual review only.</span>
    <button
      class="text-amber-400/60 hover:text-amber-300 ml-2"
      onclick={() => { localStorage.removeItem('gullbur_demo'); (window as any).__DEMO__ = false; location.reload(); }}
      title="Exit demo mode"
    >✕</button>
  </div>
{/if}

<!-- Global toast notifications -->
<Toasts />

<!-- Flex row: Sidebar + Main content -->
<div class="flex min-h-screen">
  <!-- ─── Sidebar ───────────────────────────────────────────────────── -->
  <aside class="fixed left-0 top-0 w-64 h-screen flex flex-col bg-canvas border-r border-default z-30">
    
    <!-- Logo + App name -->
    <div class="flex items-center gap-2.5 px-4 py-4 border-b border-default shrink-0">
      {@html iconHtml('wallet')}
      <h1 class="text-base font-bold tracking-tight truncate">Gullbúr Enclave</h1>
    </div>

    <!-- Connection section -->
    <div class="px-4 py-3 space-y-2 border-b border-default">
      <div class="flex items-center gap-2 text-xs">
        <span
          class="inline-block w-2 h-2 rounded-full shrink-0"
          class:bg-vault-500={vault.connected}
          class:bg-red-500={!vault.connected}
        ></span>
        <span class="text-secondary truncate">{vault.vaultStatus}</span>
      </div>
      <div class="flex flex-col gap-1.5">
        {#if !vault.connected}
          <button class="btn-primary text-xs px-3 py-1.5 w-full" disabled={connecting} onclick={connect}>
            {connecting ? 'Connecting…' : 'Connect to Vault'}
          </button>
        {:else}
          <button class="btn-secondary text-xs px-3 py-1.5 w-full" onclick={disconnect}>
            Disconnect
          </button>
        {/if}
        {#if vault.connected && vault.initialized && vault.authStatus !== 'unauthenticated'}
          <button class="btn-secondary text-xs px-3 py-1.5 flex items-center justify-center gap-1.5 w-full" onclick={() => invoke('lock_vault')} title="Lock vault">
            {@html iconHtml('lock', 'w-3.5 h-3.5')}
            Lock
          </button>
        {/if}
      </div>
    </div>

    <!-- Network selector (only when vault has networks) -->
    {#if vault.initialized && vault.networks.length > 0}
      <div class="px-4 py-2 border-b border-default">
        <label class="block text-[10px] uppercase tracking-wider text-muted mb-1" for="network-select">Network</label>
        <select
          id="network-select"
          class="w-full text-xs bg-surface border border-strong/50 rounded-lg px-2 py-1.5 text-primary outline-none"
          bind:value={vault.selectedNetwork}
        >
          {#each vault.networks as net}
            <option value={net.id}>{net.name ?? net.id}</option>
          {/each}
        </select>
      </div>
    {/if}

    <!-- Theme selector -->
    <div class="px-4 py-2 space-y-2 border-b border-default">
      <span class="text-[10px] uppercase tracking-wider text-muted">Theme</span>
      <div class="flex rounded-lg overflow-hidden border border-strong/50">
        {#each builtinThemeIds as id}
          <button
            class="flex-1 px-2 py-1.5 text-xs font-medium transition-all
              {themeEngine.currentThemeId === id
                ? 'bg-accent text-white shadow-sm'
                : 'bg-surface text-secondary hover:bg-surface-hover hover:text-primary'}"
            onclick={() => handleThemeChange(id)}
            title={id === 'obsidian' ? 'OLED Tactical Dark' : id === 'dark-slate' ? 'Legacy Dark Slate' : 'Warm Light Studio'}
          >
            {@html getThemeIcon(id)}
          </button>
        {/each}
      </div>
    </div>

    <!-- Accent selector -->
    <div class="px-4 py-2 space-y-2 border-b border-default">
      <span class="text-[10px] uppercase tracking-wider text-muted">Accent</span>
      <div class="flex items-center gap-1.5">
        {#each (['emerald', 'violet', 'amber', 'cyan', 'rose'] as const) as accent}
          <button
            class="w-5 h-5 rounded-full border border-border-strong transition-all hover:scale-110
              {vault.accent === accent ? 'ring-2 ring-accent' : ''}"
            style="background: {accent === 'emerald' ? '#10b981' : accent === 'violet' ? '#8b5cf6' : accent === 'amber' ? '#f59e0b' : accent === 'cyan' ? '#06b6d4' : '#f43f5e'}"
            title={`Accent: ${accent}`}
            aria-label={`Accent ${accent}`}
            onclick={() => handleAccentChange(accent)}
          ></button>
        {/each}
      </div>
    </div>

    <!-- Settings button -->
    {#if vault.connected && vault.initialized && vault.authStatus !== 'unauthenticated'}
      <div class="px-4 py-2">
        <button class="btn-secondary text-xs px-3 py-1.5 flex items-center justify-center gap-1.5 w-full" onclick={() => showSettings = true} title="Settings">
          {@html iconHtml('settings', 'w-3.5 h-3.5')}
          Settings
        </button>
      </div>
    {/if}

    <!-- Spacer -->
    <div class="flex-1"></div>

    <!-- Compact StatusBar at bottom of sidebar -->
    <div class="border-t border-default px-4 py-2.5 space-y-1 shrink-0">
      <div class="text-[10px] text-muted">Gullbúr Enclave Core v0.1.0</div>
      <div class="flex items-center gap-2 text-[10px]">
        <button
          class="btn-secondary text-[10px] px-1.5 py-0.5 leading-none"
          onclick={() => showConsole = true}
          title="Live IPC console (JSON-RPC log)"
        >⌨ Console</button>
        <button
          class="btn-secondary text-[10px] px-1.5 py-0.5 leading-none"
          onclick={() => showDebugReport = true}
          title="Generate a privacy-safe debug report"
        >🛠 Debug</button>
        {#if vault.connected}
          <span class="flex items-center gap-1 text-muted ml-auto">
            <span class="inline-block w-1.5 h-1.5 rounded-full bg-vault-500"></span>
            {networkCount()} net
          </span>
        {/if}
      </div>
    </div>
  </aside>

  <!-- ─── Main content area ──────────────────────────────────────────── -->
  <main class="ml-64 flex-1 flex flex-col min-h-screen">
    <section class="flex-1 max-w-6xl mx-auto w-full px-4 py-5 sm:p-6 overflow-y-auto">
      {#if !vault.initialized}
        <VaultInit />
      {:else}
        <Dashboard />
      {/if}
    </section>

    <!-- Footer with StatusBar -->
    <footer class="border-t border-default px-4 py-3 sm:px-6">
      <div class="max-w-6xl mx-auto">
        <StatusBar />
      </div>
    </footer>
  </main>
</div>

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

<!-- IPC Console modal -->
{#if showConsole}
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
  role="dialog"
  aria-modal="true"
  aria-label="IPC Console"
  tabindex="-1"
  onclick={(e) => { if (e.target === e.currentTarget) showConsole = false; }}
  onkeydown={(e) => { if (e.key === 'Escape') showConsole = false; }}
>
  <div class="bg-surface-dim border border-strong rounded-2xl shadow-2xl max-w-2xl w-full mx-4 p-6 h-[80vh] flex flex-col" role="document">
    <div class="flex items-center justify-between mb-4 shrink-0">
      <h2 class="text-lg font-semibold">📟 IPC Console</h2>
      <button class="text-muted hover:text-primary text-xl leading-none" onclick={() => showConsole = false}>&times;</button>
    </div>
    <div class="flex-1 overflow-hidden">
      <ConsoleLog />
    </div>
  </div>
</div>
{/if}

<!-- Debug Report modal -->
{#if showDebugReport}
<DebugReport onclose={() => showDebugReport = false} />
{/if}