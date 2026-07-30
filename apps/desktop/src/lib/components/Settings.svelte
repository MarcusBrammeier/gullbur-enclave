<script lang="ts">
  import { vault } from '../vault.svelte.ts';
  import { invoke } from '@tauri-apps/api/core';
  import DebugReport from './DebugReport.svelte';
  import ConsoleLog from './ConsoleLog.svelte';
  import { setTheme } from '../vault.svelte.ts';

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let autoLockSecs = $state(
    typeof localStorage !== 'undefined'
      ? parseInt(localStorage.getItem('foss_wallet_autolock') ?? '30', 10)
      : 30
  );
  let torEnabled = $state(vault.torEnabled);
  let testnetOnly = $state(vault.testnetOnly);
  let lockResult = $state<string | null>(null);
  let saving = $state(false);
  let showSeedConfirm = $state(false);
  let seedPhrase = $state('');
  let seedLoading = $state(false);
  let seedError = $state('');
  let seedCopied = $state(false);
  let seedRevealed = $state(false);
  let showDebugReport = $state(false);
  let showTestnetWarning = $state(false);
  let pendingTestnetToggle = $state(false);
  let showConsole = $state(false);

  // Sync auto-lock to localStorage on change
  $effect(() => {
    localStorage.setItem('foss_wallet_autolock', String(autoLockSecs));
  });

  async function handleLock() {
    lockResult = null;
    try {
      await invoke('lock_vault');
      vault.authStatus = 'unauthenticated';
      lockResult = 'locked';
      setTimeout(() => { lockResult = null; }, 2000);
    } catch {
      lockResult = 'error';
    }
  }

  async function handleTorToggle() {
    torEnabled = !torEnabled;
    try {
      await invoke('toggle_tor', { enabled: torEnabled });
      vault.torEnabled = torEnabled;
    } catch {
      torEnabled = !torEnabled;
    }
  }

  function handleTestnetToggle() {
    vault.testnetOnly = !testnetOnly;
    testnetOnly = vault.testnetOnly;
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }

  async function handleShowSeed() {
    showSeedConfirm = false;
    seedLoading = true;
    seedError = '';
    try {
      seedPhrase = await invoke('get_seed_phrase');
      seedRevealed = false;
    } catch (e) {
      seedError = e instanceof Error ? e.message : String(e);
    } finally {
      seedLoading = false;
    }
  }

  async function copySeed() {
    try {
      await navigator.clipboard.writeText(seedPhrase);
      seedCopied = true;
      setTimeout(() => seedCopied = false, 2000);
    } catch { /* fallback handled by user manually */ }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
  role="dialog"
  aria-modal="true"
  aria-label="Settings"
  onclick={handleBackdropClick}
  onkeydown={(e) => { if (e.key === 'Escape') onclose(); }}
>
  <div class="bg-gray-900 border border-gray-700 rounded-2xl shadow-2xl max-w-md w-full mx-4 p-6" onclick={(e) => e.stopPropagation()}>
    <div class="flex items-center justify-between mb-5">
      <h2 class="text-lg font-semibold">⚙️ Settings</h2>
      <button class="text-gray-500 hover:text-gray-300 text-xl leading-none" onclick={onclose}>&times;</button>
    </div>

    <div class="space-y-5">
      <!-- Auto-lock -->
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Auto-Lock Timer</label>
        <div class="flex items-center gap-3">
          <input type="range" min="0" max="300" step="5" bind:value={autoLockSecs} class="flex-1 accent-vault-500" />
          <span class="text-sm font-mono text-gray-400 w-16 text-right">{autoLockSecs === 0 ? 'Off' : `${autoLockSecs}s`}</span>
        </div>
        <p class="text-xs text-gray-600 mt-1">Vault locks automatically after inactivity (0 = disabled)</p>
      </div>

      <hr class="border-gray-800" />

      <!-- Tor toggle -->
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-medium text-gray-300">Tor Proxy</p>
          <p class="text-xs text-gray-500">Route RPC through Tor SOCKS5</p>
        </div>
        <button
          class="relative w-11 h-6 rounded-full transition-colors {torEnabled ? 'bg-vault-600' : 'bg-gray-700'}"
          onclick={handleTorToggle}
          role="switch"
          aria-checked={torEnabled}
        >
          <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {torEnabled ? 'translate-x-5' : ''}"></span>
        </button>
      </div>

      <hr class="border-gray-800" />

      <!-- Theme Selector -->
      <div>
        <p class="text-sm font-medium text-gray-300 mb-2">🎨 Theme</p>
        <div class="flex gap-2">
          {#each ['dark', 'light', 'system'] as t}
            <button
              class="flex-1 py-2 px-3 rounded-lg text-sm font-medium transition-all
                {vault.theme === t
                  ? 'bg-vault-600 text-white shadow-sm'
                  : 'bg-gray-800 text-gray-300 hover:bg-gray-700 border border-gray-700/50'}"
              onclick={() => setTheme(t as 'light' | 'dark' | 'system')}
            >
              {t === 'dark' ? '🌙 Dark' : t === 'light' ? '☀️ Light' : '💻 System'}
            </button>
          {/each}
        </div>
      </div>

      <hr class="border-gray-800" />

      <!-- Testnet toggle with beta warning -->
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-medium text-gray-300">Testnet-Only Mode</p>
          <p class="text-xs text-gray-500">{testnetOnly ? 'Only test networks shown' : 'Mainnets and testnets both visible'}</p>
        </div>
        <button
          class="relative w-11 h-6 rounded-full transition-colors {testnetOnly ? 'bg-amber-600' : 'bg-gray-700'}"
          onclick={() => {
            if (testnetOnly) {
              // Turning testnet-only OFF — show beta warning
              pendingTestnetToggle = true;
              showTestnetWarning = true;
            } else {
              handleTestnetToggle();
            }
          }}
          role="switch"
          aria-checked={testnetOnly}
        >
          <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {testnetOnly ? 'translate-x-5' : ''}"></span>
        </button>
      </div>

      <hr class="border-gray-800" />

      <!-- Lock vault -->
      <div>
        <p class="text-sm font-medium text-gray-300 mb-2">Security</p>
        {#if lockResult === 'locked'}
          <p class="text-xs text-vault-400 mb-2">✅ Vault locked</p>
        {:else if lockResult === 'error'}
          <p class="text-xs text-red-400 mb-2">❌ Lock failed</p>
        {/if}
        <button class="btn-secondary text-sm w-full" disabled={!vault.connected || vault.authStatus === 'unauthenticated'} onclick={handleLock}>
          🔒 Lock Vault Now
        </button>
      </div>

      <hr class="border-gray-800" />

      <!-- Seed Phrase Re-export -->
      <div>
        <p class="text-sm font-medium text-gray-300 mb-2">📝 Seed Recovery</p>
        {#if !showSeedConfirm && !seedPhrase}
          <button class="btn-secondary text-sm w-full" disabled={!vault.connected || !vault.initialized} onclick={() => showSeedConfirm = true}>
            Show Seed Phrase
          </button>
        {:else if showSeedConfirm && !seedPhrase}
          <div class="bg-amber-900/20 border border-amber-700/30 rounded-xl p-4 text-xs text-amber-300 space-y-2">
            <p>⚠️ <strong>Your seed phrase gives full access to your wallet.</strong></p>
            <p>Never share it. Never type it into any website. Anyone with these words can steal your funds.</p>
            <p class="text-gray-400">Only reveal in a private, secure environment.</p>
            <div class="flex gap-2 mt-3">
              <button class="btn-secondary text-xs flex-1" onclick={() => showSeedConfirm = false}>Cancel</button>
              <button class="bg-amber-600 hover:bg-amber-500 text-black text-xs font-semibold py-2 px-3 rounded-lg flex-1" onclick={handleShowSeed}>I Understand — Reveal</button>
            </div>
          </div>
        {:else if seedLoading}
          <p class="text-xs text-gray-400">Loading seed phrase...</p>
        {:else if seedError}
          <p class="text-xs text-red-400">{seedError}</p>
          <button class="btn-secondary text-xs mt-2" onclick={() => { showSeedConfirm = true; seedError = ''; }}>Try Again</button>
        {:else if seedPhrase}
          {#if !seedRevealed}
            <button class="btn-secondary text-sm w-full" onclick={() => seedRevealed = true}>👁️ Click to Reveal</button>
          {:else}
            <div class="bg-gray-800 border border-gray-700 rounded-lg p-3 font-mono text-xs text-gray-200 leading-relaxed break-all select-all">
              {seedPhrase}
            </div>
            <div class="flex gap-2 mt-2">
              <button class="btn-secondary text-xs flex-1" onclick={copySeed}>
                {seedCopied ? '✅ Copied!' : '📋 Copy'}
              </button>
              <button class="btn-secondary text-xs flex-1" onclick={() => { seedPhrase = ''; seedRevealed = false; }}>
                Hide
              </button>
            </div>
          {/if}
        {/if}
      </div>

      <hr class="border-gray-800" />

      <!-- Donate -->
      <div class="text-center">
        <a href="https://github.com/sponsors/YOUR_USERNAME" target="_blank" rel="noopener"
           class="inline-flex items-center gap-1 text-xs text-vault-400 hover:text-vault-300 transition-colors">
          ❤️ Donate — support open-source development
        </a>
      </div>

      <hr class="border-gray-800" />

      <!-- Vault file management -->
      <div>
        <p class="text-sm font-medium text-gray-300 mb-2">💾 Vault File</p>
        <button
          class="btn-secondary text-sm w-full"
          onclick={async () => {
            try {
              const { save } = await import('@tauri-apps/plugin-dialog');
              const dest = await save({
                title: 'Export Keystore',
                defaultPath: 'gullbur-keystore.bin',
                filters: [{ name: 'Keystore', extensions: ['bin'] }],
              });
              if (!dest) return;
              const { invoke } = await import('@tauri-apps/api/core');
              const bytes = await invoke('export_current_keystore', { destination: dest });
              alert(`✅ Keystore exported (${bytes} bytes) to:\n${dest}`);
            } catch (e) {
              alert('❌ Export failed: ' + (e instanceof Error ? e.message : String(e)));
            }
          }}
        >
          📤 Export Current Keystore…
        </button>
        <p class="text-xs text-gray-600 mt-1">Save your encrypted keystore file to a custom location</p>
      </div>

      <hr class="border-gray-800" />

      <!-- Debug Console -->
      <div>
        <p class="text-sm font-medium text-gray-300 mb-2">📟 Debug Console</p>
        <button
          class="btn-secondary text-sm w-full"
          onclick={() => showConsole = true}
        >
          Open IPC Console
        </button>
        <p class="text-xs text-gray-600 mt-1">Live log of all JSON-RPC calls — green for success, red for errors</p>
      </div>

      <hr class="border-gray-800" />

      <!-- Debug Report -->
      <div>
        <p class="text-sm font-medium text-gray-300 mb-2">🔍 Debug Report</p>
        <button
          class="btn-secondary text-sm w-full"
          onclick={() => showDebugReport = true}
        >
          Generate Debug Report
        </button>
        <p class="text-xs text-gray-600 mt-1">Creates a privacy-safe report for bug triage — review, redact, and share</p>
      </div>

      <hr class="border-gray-800" />

      <!-- Bug Reporter -->
      <div>
        <p class="text-sm font-medium text-gray-300 mb-2">🐛 Beta Feedback</p>
        <button
          class="btn-secondary text-sm w-full"
          onclick={async () => {
            try {
              await invoke('report_bug', { description: '' });
            } catch (e) {
              // Fallback: open manually
              const url = 'https://github.com/gullbur/gullbur/issues/new';
              window.open(url, '_blank');
            }
          }}
        >
          Report a Bug
        </button>
        <p class="text-xs text-gray-600 mt-1">Opens a pre-filled GitHub issue with crash data</p>
      </div>

      <hr class="border-gray-800" />

      <!-- Version -->
      <div class="text-xs text-gray-600 text-center">
        v0.1.0-beta
      </div>
    </div>
  </div>
</div>

{#if showDebugReport}
<DebugReport onclose={() => showDebugReport = false} />
{/if}

{#if showConsole}
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
  role="dialog"
  aria-modal="true"
  onclick={() => showConsole = false}
>
  <div class="bg-gray-900 border border-gray-700 rounded-2xl shadow-2xl max-w-2xl w-full mx-4 p-6 h-[80vh] flex flex-col" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') showConsole = false; }}>
    <div class="flex items-center justify-between mb-4 shrink-0">
      <h2 class="text-lg font-semibold">📟 IPC Console</h2>
      <button class="text-gray-500 hover:text-gray-300 text-xl leading-none" onclick={() => showConsole = false}>&times;</button>
    </div>
    <div class="flex-1 overflow-hidden">
      <ConsoleLog />
    </div>
  </div>
</div>
{/if}

<!-- Testnet Beta Warning Modal -->
{#if showTestnetWarning}
<div
  class="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm"
  role="alertdialog"
  aria-modal="true"
  onclick={() => { if (!pendingTestnetToggle) showTestnetWarning = false; }}
>
  <div class="bg-gray-900 border border-amber-700/50 rounded-2xl shadow-2xl max-w-sm w-full mx-4 p-6" onclick={(e) => e.stopPropagation()}>
    <div class="text-center space-y-4">
      <div class="text-4xl">⚠️</div>
      <h3 class="text-lg font-semibold text-amber-400">Mainnet Access</h3>
      <div class="text-sm text-gray-300 space-y-2">
        <p><strong>This is beta software.</strong> Gullbúr Enclave Core v0.1.0 has not been audited and may contain bugs.</p>
        <p>Disabling testnet-only mode will show real mainnet accounts. Only proceed if you understand the risks and are testing with small amounts.</p>
      </div>
      <div class="flex gap-3 mt-4">
        <button class="btn-secondary text-sm flex-1" onclick={() => { showTestnetWarning = false; pendingTestnetToggle = false; }}>
          Cancel
        </button>
        <button class="bg-amber-600 hover:bg-amber-500 text-black text-sm font-semibold flex-1 py-2 px-4 rounded-lg transition-all"
          onclick={() => {
            showTestnetWarning = false;
            pendingTestnetToggle = false;
            vault.testnetOnly = false;
            testnetOnly = false;
          }}>
          I Understand — Continue
        </button>
      </div>
      <p class="text-xs text-gray-500">This warning will appear each time you re-enable mainnets.</p>
    </div>
  </div>
</div>
{/if}