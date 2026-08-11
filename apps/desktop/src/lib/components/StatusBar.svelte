<script lang="ts">
  import { vault, networkCount } from '../vault.svelte.ts';
  import TorToggle from './TorToggle.svelte';
  import UpdateBanner from './UpdateBanner.svelte';
  import DebugReport from './DebugReport.svelte';
  import ConsoleLog from './ConsoleLog.svelte';

  let showDebugReport = $state(false);
  let showConsole = $state(false);

  // ── Derived display values ─────────────────────────────────────────────

  const dotColor = $derived(vault.connected ? 'bg-vault-500' : 'bg-red-500');
  const dotShadow = $derived(vault.connected ? 'shadow-[0_0_6px_#22c55e]' : 'shadow-[0_0_6px_#ef4444]');
</script>

<footer class="border-t border-default px-6 py-3 pb-[env(safe-area-inset-bottom)]">
  <div class="max-w-6xl mx-auto flex flex-col gap-1">
    <UpdateBanner />
    <div class="flex items-center justify-between text-xs text-muted">
      <!-- Left: version -->
    <span>Gullbúr Enclave Core v0.1.0</span>

    <!-- Right: status indicators + actions -->
    <span class="flex items-center gap-4">
      <!-- Console / Debug Report actions -->
      <span class="flex items-center gap-1">
        <button
          class="btn-secondary text-[11px] px-2 py-1"
          onclick={() => showConsole = true}
          title="Live IPC console (JSON-RPC log)"
        >
          ⌨ Console
        </button>
        <button
          class="btn-secondary text-[11px] px-2 py-1"
          onclick={() => showDebugReport = true}
          title="Generate a privacy-safe debug report"
        >
          🛠 Debug
        </button>
      </span>

      <!-- Connection dot + status -->
      <span class="flex items-center gap-2">
        <span class="inline-block w-2.5 h-2.5 rounded-full {dotColor} {dotShadow}"></span>
        <span class="text-secondary">{vault.vaultStatus}</span>
      </span>

      <!-- Tor toggle (opt-in privacy) -->
      <TorToggle size="sm" />

      <!-- Network count -->
      <span>
        Networks: {networkCount()}
      </span>
    </span>
  </div>
  </div>
</footer>

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
