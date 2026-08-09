<script lang="ts">
  import './app.css';
  import { vault, connect, disconnect } from './lib/vault.svelte.ts';
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
  import { iconHtml } from './lib/icons';
  import { IS_DEMO } from './lib/constants';

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

  // ── Sidebar state ────────────────────────────────────────────
  let sidebarOpen = $state(true);
  let sidebarCollapsed = $state(false);

  const builtinThemeIds = ['obsidian', 'dark-slate', 'light-studio'] as const;
  type BuiltinThemeId = (typeof builtinThemeIds)[number];

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

  function handleMotionChange(speed: 'instant' | 'normal' | 'expressive') {
    themeEngine.setMotionSpeed(speed);
  }

  let density = $state<'normal' | 'compact' | 'expanded'>(
    (typeof localStorage !== 'undefined' ? (localStorage.getItem('gullbur_density') as 'normal' | 'compact' | 'expanded' | null) : null) ?? 'normal'
  );
  function handleDensityChange(mode: 'normal' | 'compact' | 'expanded') {
    density = mode;
    document.documentElement.setAttribute('data-density', mode);
    localStorage.setItem('gullbur_density', mode);
  }

  $effect(() => {
    const id = themeEngine.currentThemeId;
    const isDark = id !== 'light-studio';
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
  });
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="flex min-h-screen">
  <!-- Sidebar -->
  <aside class="fixed left-0 top-0 w-64 h-screen flex flex-col bg-canvas border-r border-default z-30 overflow-y-auto">
    <!-- Logo area -->
    <div class="flex items-center gap-3 px-5 py-4 border-b border-default shrink-0">
      {@html iconHtml('wallet', 'w-6 h-6 text-accent')}
      <h1 class="text-lg font-bold tracking-tight">Gullbúr Enclave</h1>
    </div>

    <!-- Connection status + actions -->
    <div class="px-5 py-3 border-b border-default shrink-0 space-y-2">
      <div class="flex items-center gap-2">
        <span
          class="inline-block w-2 h-2 rounded-full"
          class:bg-vault-500={vault.connected}
          class:bg-red-500={!vault.connected}
        ></span>
        <span class="text-sm text-secondary">{vault.vaultStatus}</span>
      </div>
      {#if vault.connected && vault.initialized && vault.authStatus !== 'unauthenticated'}
        <button class="btn-secondary text-sm w-full flex items-center justify-center gap-1.5" onclick={() => invoke('lock_vault')}>
          {@html iconHtml('lock', 'w-4 h-4')} Lock Vault
        </button>
      {/if}
      {#if !vault.connected}
        <button class="btn-primary text-sm w-full" disabled={connecting} onclick={connect}>
          {connecting ? 'Connecting…' : 'Connect'}
        </button>
      {:else}
        <button class="btn-secondary text-sm w-full" onclick={disconnect}>
          Disconnect
        </button>
      {/if}
    </div>

    <!-- Theme Selector -->
    <div class="px-5 py-3 border-b border-default shrink-0">
      <p class="text-[11px] text-muted uppercase tracking-wider mb-2">Theme</p>
      <div class="flex rounded-lg overflow-hidden border border-strong/50">
        {#each builtinThemeIds as id}
          <button
            class="flex-1 px-2.5 py-1.5 text-xs font-medium transition-all
              {themeEngine.currentThemeId === id
                ? 'bg-accent text-white shadow-sm'
                : 'bg-surface text-secondary hover:bg-surface-hover hover:text-primary'}"
            onclick={() => handleThemeChange(id)}
            title={id === 'obsidian' ? 'OLED Tactical Dark' : id === 'dark-slate' ? 'Legacy Dark Slate' : 'Warm Light Studio'}
          >
            {@html iconHtml(id === 'light-studio' ? 'sun' : 'moon', 'w-3.5 h-3.5')}
          </button>
        {/each}
      </div>
    </div>

    <!-- Accent Selector -->
    <div class="px-5 py-3 border-b border-default shrink-0">
      <p class="text-[11px] text-muted uppercase tracking-wider mb-2">Accent</p>
      <div class="flex items-center gap-1.5">
        {#each (['emerald', 'violet', 'amber', 'cyan', 'rose'] as const) as accent}
          <button
            class="w-5 h-5 rounded-full border border-strong/50 transition-all hover:scale-110
              {vault.accent === accent ? 'ring-2 ring-accent ring-offset-1 ring-offset-canvas' : ''}"
            style="background: {accent === 'emerald' ? '#10b981' : accent === 'violet' ? '#8b5cf6' : accent === 'amber' ? '#f59e0b' : accent === 'cyan' ? '#06b6d4' : '#f43f5e'}"
            title={`Accent: ${accent}`}
            aria-label={`Accent ${accent}`}
            onclick={() => handleAccentChange(accent)}
          ></button>
        {/each}
      </div>
    </div>

    <!-- Motion Speed -->
    <div class="px-5 py-3 border-b border-default shrink-0">
      <p class="text-[11px] text-muted uppercase tracking-wider mb-2">Motion</p>
      <div class="flex rounded-lg overflow-hidden border border-strong/30 text-[11px]">
        <button
          class="flex-1 px-2 py-1 transition-colors {themeEngine.motionSpeed === 'instant' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
          onclick={() => handleMotionChange('instant')}
        >{@html iconHtml('zap', 'w-3 h-3 inline-block mr-0.5')}0ms</button>
        <button
          class="flex-1 px-2 py-1 transition-colors {themeEngine.motionSpeed === 'normal' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
          onclick={() => handleMotionChange('normal')}
        >{@html iconHtml('target', 'w-3 h-3 inline-block mr-0.5')}100ms</button>
        <button
          class="flex-1 px-2 py-1 transition-colors {themeEngine.motionSpeed === 'expressive' ? 'bg-accent text-white' : 'bg-surface text-secondary hover:text-primary'}"
          onclick={() => handleMotionChange('expressive')}
        >{@html iconHtml('sparkles', 'w-3 h-3 inline-block mr-0.5')}200ms</button>
      </div>
    </div>

    <!-- Settings -->
    <div class="px-5 py-3 shrink-0">
      <button class="btn-secondary text-sm w-full flex items-center justify-center gap-1.5" onclick={() => showSettings = true}>
        {@html iconHtml('settings', 'w-4 h-4')} Settings
      </button>
    </div>

    <!-- Spacer + version at bottom -->
    <div class="flex-1 min-h-0"></div>
    <div class="px-5 py-3 border-t border-default text-[11px] text-muted shrink-0">
      <span>v0.1.0-beta</span>
    </div>
  </aside>

  <!-- Main content area -->
  <main class="ml-64 flex-1 flex flex-col min-h-screen">
    <!-- Demo mode warning banner -->
    {#if IS_DEMO}
    <div class="bg-amber-600/20 border-b border-amber-600/30 px-4 py-1.5 text-xs text-amber-400 text-center flex items-center justify-center gap-2 sticky top-0 z-40" style="backdrop-filter: blur(8px);">
      <span class="inline-block w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>
      <span><strong>GUI Test Mode</strong> — Core engine not connected. Mock data for visual review only.</span>
      <button
        class="text-amber-400/60 hover:text-amber-300 ml-2"
        onclick={() => { localStorage.removeItem('gullbur_demo'); (window as any).__DEMO__ = false; location.reload(); }}
        title="Exit demo mode"
      >{@html iconHtml('close', 'w-4 h-4')}</button>
    </div>
  {/if}

  <!-- Global toast notifications -->
  <Toasts />

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
</div>

<!-- Auth overlay -->
<AuthPrompt />

<!-- Global error banner -->
{#if errorMessage}
<div class="fixed bottom-16 left-1/2 -translate-x-1/2 z-50 max-w-lg w-full mx-4">
  <div class="bg-red-900/90 border border-red-700 rounded-xl px-4 py-3 text-sm text-red-200 shadow-2xl flex items-start gap-3">
    <span>{@html iconHtml('alertCircle', 'w-5 h-5 text-red-300 shrink-0 mt-0.5')}</span>
    <span class="flex-1">{errorMessage}</span>
    <button class="text-red-400 hover:text-red-200 font-bold" onclick={dismissError}>{@html iconHtml('close', 'w-4 h-4')}</button>
  </div>
</div>
{/if}

<!-- Settings modal -->
{#if showSettings}
<Settings onclose={() => showSettings = false} />
{/if}
