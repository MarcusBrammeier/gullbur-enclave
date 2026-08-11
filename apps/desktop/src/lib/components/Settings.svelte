<script lang="ts">
  import { vault } from '../vault.svelte.ts';
  import { invoke } from '@tauri-apps/api/core';
  import { fade, scale } from 'svelte/transition';
  import DebugReport from './DebugReport.svelte';
  import ConsoleLog from './ConsoleLog.svelte';
  import { iconHtml } from '../icons';
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
    if (e.target === e.currentTarget) handleClose();
  }

  async function handleShowSeed() {
    showSeedConfirm = false;
    seedLoading = true;
    seedError = '';
    try {
      // Fetch on demand; clear immediately after the reveal moment (Hide / close)
      // so the seed does not persist in component state longer than needed.
      const phrase = await invoke('get_seed_phrase');
      seedPhrase = typeof phrase === 'string' ? phrase : String(phrase ?? '');
      seedRevealed = false;
    } catch (e) {
      seedError = e instanceof Error ? e.message : String(e);
    } finally {
      seedLoading = false;
    }
  }

  function clearSeed() {
    seedPhrase = '';
    seedRevealed = false;
  }

  function handleClose() {
    // Never let the revealed seed persist in memory once the modal closes.
    clearSeed();
    onclose();
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
  transition:fade={{ duration: 150 }}
  role="dialog"
  aria-modal="true"
  aria-label="Settings"
  tabindex="-1"
  onclick={handleBackdropClick}
  onkeydown={(e) => { if (e.key === 'Escape') handleClose(); }}
>
  <div class="bg-surface-dim border border-strong rounded-2xl shadow-2xl max-w-md w-full mx-4 p-6"
    transition:scale={{ start: 0.97, duration: 180 }}
    role="document">
    <div class="flex items-center justify-between mb-5">
      <h2 class="text-lg font-semibold">{@html iconHtml('settings', 'w-5 h-5 inline-block mr-1.5')}Settings</h2>
      <button class="text-muted hover:text-primary text-xl leading-none" onclick={handleClose}>&times;</button>
    </div>

    <div class="space-y-5">
      <!-- Auto-lock -->
      <div>
        <label class="block text-sm font-medium text-primary mb-1" for="auto-lock-range">Auto-Lock Timer</label>
        <div class="flex items-center gap-3">
          <input id="auto-lock-range" type="range" min="0" max="300" step="5" bind:value={autoLockSecs} class="flex-1 accent-vault-500" />
          <span class="text-sm font-mono text-secondary w-16 text-right">{autoLockSecs === 0 ? 'Off' : `${autoLockSecs}s`}</span>
        </div>
        <p class="text-xs text-muted mt-1">Vault locks automatically after inactivity (0 = disabled)</p>
      </div>

      <hr class="border-default" />

      <!-- Tor toggle -->
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-medium text-primary">Tor Proxy</p>
          <p class="text-xs text-muted">Route RPC through Tor SOCKS5</p>
        </div>
        <button
          class="relative w-11 h-6 rounded-full transition-colors {torEnabled ? 'bg-vault-600' : 'bg-surface-hover'}"
          onclick={handleTorToggle}
          role="switch"
          aria-checked={torEnabled}
          aria-label="Toggle Tor proxy"
        >
          <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {torEnabled ? 'translate-x-5' : ''}"></span>
        </button>
      </div>

      <hr class="border-default" />

      <!-- Testnet toggle with beta warning -->
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-medium text-primary">Testnet-Only Mode</p>
          <p class="text-xs text-muted">{testnetOnly ? 'Only test networks shown' : 'Mainnets and testnets both visible'}</p>
        </div>
        <button
          class="relative w-11 h-6 rounded-full transition-colors {testnetOnly ? 'bg-amber-600' : 'bg-surface-hover'}"
          aria-label="Toggle testnet-only mode"
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

      <hr class="border-default" />

      <!-- Lock vault -->
      <div>
        <p class="text-sm font-medium text-primary mb-2">Security</p>
        {#if lockResult === 'locked'}
          <p class="text-xs text-vault-400 mb-2">{@html iconHtml('check', 'w-3.5 h-3.5 inline-block mr-1')}Vault locked</p>
        {:else if lockResult === 'error'}
          <p class="text-xs text-red-400 mb-2">{@html iconHtml('alertCircle', 'w-3.5 h-3.5 inline-block mr-1')}Lock failed</p>
        {/if}
        <button class="btn-secondary text-sm w-full" disabled={!vault.connected || vault.authStatus === 'unauthenticated'} onclick={handleLock}>
          {@html iconHtml('lock', 'w-4 h-4 inline-block mr-1.5')}Lock Vault Now
        </button>
      </div>

      <hr class="border-default" />

      <!-- Seed Phrase Re-export -->
      <div>
        <p class="text-sm font-medium text-primary mb-2">{@html iconHtml('book', 'w-4 h-4 inline-block mr-1.5')}Seed Recovery</p>
        {#if !showSeedConfirm && !seedPhrase}
          <button class="btn-secondary text-sm w-full" disabled={!vault.connected || !vault.initialized} onclick={() => showSeedConfirm = true}>
            Show Seed Phrase
          </button>
        {:else if showSeedConfirm && !seedPhrase}
          <div class="bg-amber-900/20 border border-amber-700/30 rounded-xl p-4 text-xs text-amber-300 space-y-2">
            <p>{@html iconHtml('alertTriangle', 'w-4 h-4 inline-block mr-1')}<strong>Your seed phrase gives full access to your wallet.</strong></p>
            <p>Never share it. Never type it into any website. Anyone with these words can steal your funds.</p>
            <p class="text-secondary">Only reveal in a private, secure environment.</p>
            <div class="flex gap-2 mt-3">
              <button class="btn-secondary text-xs flex-1" onclick={() => showSeedConfirm = false}>Cancel</button>
              <button class="bg-amber-600 hover:bg-amber-500 text-black text-xs font-semibold py-2 px-3 rounded-lg flex-1" onclick={handleShowSeed}>I Understand — Reveal</button>
            </div>
          </div>
        {:else if seedLoading}
          <p class="text-xs text-secondary">Loading seed phrase...</p>
        {:else if seedError}
          <p class="text-xs text-red-400">{seedError}</p>
          <button class="btn-secondary text-xs mt-2" onclick={() => { showSeedConfirm = true; seedError = ''; }}>Try Again</button>
        {:else if seedPhrase}
          {#if !seedRevealed}
            <button class="btn-secondary text-sm w-full" onclick={() => seedRevealed = true}>{@html iconHtml('eye', 'w-4 h-4 inline-block mr-1.5')}Click to Reveal</button>
          {:else}
            <div class="bg-surface border border-strong rounded-lg p-3 font-mono text-xs text-primary leading-relaxed break-all select-all">
              {seedPhrase}
            </div>
            <div class="flex gap-2 mt-2">
              <button class="btn-secondary text-xs flex-1" onclick={copySeed}>
                {#if seedCopied}
                  {@html iconHtml('check', 'w-3.5 h-3.5 inline-block mr-1')}Copied!
                {:else}
                  {@html iconHtml('copy', 'w-3.5 h-3.5 inline-block mr-1')}Copy
                {/if}
              </button>
              <button class="btn-secondary text-xs flex-1" onclick={() => { clearSeed(); }}>
                Hide
              </button>
            </div>
          {/if}
        {/if}
      </div>

      <hr class="border-default" />

      <!-- Donate -->
      <div class="text-center">
        <a href="https://github.com/MarcusBrammeier/gullbur-enclave" target="_blank" rel="noopener"
           class="inline-flex items-center gap-1 text-xs text-vault-400 hover:text-vault-300 transition-colors">
          {@html iconHtml('heart', 'w-3.5 h-3.5 inline-block')} Donate — support open-source development
        </a>
      </div>

      <hr class="border-default" />

      <!-- Vault file management -->
      <div>
        <p class="text-sm font-medium text-primary mb-2">{@html iconHtml('download', 'w-4 h-4 inline-block mr-1.5')}Vault File</p>
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
              alert(`✓ Keystore exported (${bytes} bytes) to:\n${dest}`);
            } catch (e) {
              alert('✗ Export failed: ' + (e instanceof Error ? e.message : String(e)));
            }
          }}
        >
          {@html iconHtml('upload', 'w-4 h-4 inline-block mr-1.5')}Export Current Keystore…
        </button>
        <p class="text-xs text-muted mt-1">Save your encrypted keystore file to a custom location</p>
      </div>

      <hr class="border-default" />

      <!-- Debug Console -->
      <div>
        <p class="text-sm font-medium text-primary mb-2">{@html iconHtml('terminal', 'w-4 h-4 inline-block mr-1.5')}Debug Console</p>
        <button
          class="btn-secondary text-sm w-full"
          onclick={() => showConsole = true}
        >
          Open IPC Console
        </button>
        <p class="text-xs text-muted mt-1">Live log of all JSON-RPC calls — green for success, red for errors</p>
      </div>

      <hr class="border-default" />

      <!-- Debug Report -->
      <div>
        <p class="text-sm font-medium text-primary mb-2">{@html iconHtml('search', 'w-4 h-4 inline-block mr-1.5')}Debug Report</p>
        <button
          class="btn-secondary text-sm w-full"
          onclick={() => showDebugReport = true}
        >
          Generate Debug Report
        </button>
        <p class="text-xs text-muted mt-1">Creates a privacy-safe report for bug triage — review, redact, and share</p>
      </div>

      <hr class="border-default" />

      <!-- Bug Reporter -->
      <div>
        <p class="text-sm font-medium text-primary mb-2">{@html iconHtml('bug', 'w-4 h-4 inline-block mr-1.5')}Beta Feedback</p>
        <button
          class="btn-secondary text-sm w-full"
          onclick={async () => {
            try {
              await invoke('report_bug', { description: '' });
            } catch (e) {
              // Fallback: open manually
              const url = 'https://github.com/MarcusBrammeier/gullbur-enclave/issues/new';
              window.open(url, '_blank');
            }
          }}
        >
          Report a Bug
        </button>
        <p class="text-xs text-muted mt-1">Opens a pre-filled GitHub issue with crash data</p>
      </div>

      <hr class="border-default" />

      <!-- Version -->
      <div class="text-xs text-muted text-center">
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
  aria-label="IPC Console"
  tabindex="-1"
  onclick={(e) => { if (e.target === e.currentTarget) showConsole = false; }}
  onkeydown={(e) => { if (e.key === 'Escape') showConsole = false; }}
>
  <div class="bg-surface-dim border border-strong rounded-2xl shadow-2xl max-w-2xl w-full mx-4 p-6 h-[80vh] flex flex-col" role="document">
    <div class="flex items-center justify-between mb-4 shrink-0">
      <h2 class="text-lg font-semibold">{@html iconHtml('terminal', 'w-5 h-5 inline-block mr-1.5')}IPC Console</h2>
      <button class="text-muted hover:text-primary text-xl leading-none" onclick={() => showConsole = false}>&times;</button>
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
  tabindex="-1"
  onclick={() => { if (!pendingTestnetToggle) showTestnetWarning = false; }}
  onkeydown={(e) => { if (e.key === 'Escape') showTestnetWarning = false; }}
>
  <div class="bg-surface-dim border border-amber-700/50 rounded-2xl shadow-2xl max-w-sm w-full mx-4 p-6" role="document">
    <div class="text-center space-y-4">
      <div class="text-4xl">{@html iconHtml('alertTriangle', 'w-10 h-10 inline-block text-amber-400')}</div>
      <h3 class="text-lg font-semibold text-amber-400">Mainnet Access</h3>
      <div class="text-sm text-primary space-y-2">
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
      <p class="text-xs text-muted">This warning will appear each time you re-enable mainnets.</p>
    </div>
  </div>
</div>
{/if}