<script lang="ts">
  import { vault, setTheme } from '../vault.svelte.ts';

  function handleTestnetToggle() {
    if (vault.testnetOnly) {
      // Turning testnet-only OFF — show beta warning
      vault.showBetaWarning = true;
    } else {
      // Turning it back ON — no warning needed
      vault.testnetOnly = true;
    }
  }

  function confirmMainnet() {
    vault.testnetOnly = false;
    vault.showBetaWarning = false;
  }

  function cancelMainnet() {
    vault.showBetaWarning = false;
  }
</script>

<div class="flex items-center gap-3">
  <!-- Testnet-Only Toggle -->
  <div class="flex items-center gap-1.5">
    <button
      class="relative w-9 h-5 rounded-full transition-colors {vault.testnetOnly ? 'bg-amber-600' : 'bg-surface-hover'}"
      onclick={handleTestnetToggle}
      role="switch"
      aria-checked={vault.testnetOnly}
      title={vault.testnetOnly ? 'Testnet-only mode — click to disable' : 'Enable testnet-only mode'}
    >
      <span
        class="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform {vault.testnetOnly ? 'translate-x-4' : ''}"
      ></span>
    </button>
    {#if vault.testnetOnly}
      <span class="text-[10px] uppercase tracking-wider font-semibold text-amber-400 bg-amber-400/10 border border-amber-400/30 rounded px-1.5 py-0.5 leading-none">
        Testnet Only
      </span>
    {/if}
  </div>

  <!-- Theme Selector -->
  <div class="flex rounded-lg overflow-hidden border border-strong/50">
    {#each ['dark', 'light', 'system'] as t}
      <button
        class="px-3 py-1.5 text-xs font-medium transition-all
          {vault.theme === t
            ? 'bg-vault-600 text-white shadow-sm'
            : 'bg-surface text-secondary hover:bg-surface-hover hover:text-primary'}"
        onclick={() => setTheme(t as 'light' | 'dark' | 'system')}
      >
        {t === 'dark' ? '🌙' : t === 'light' ? '☀️' : '💻'}
      </button>
    {/each}
  </div>
</div>

<!-- Beta Warning Modal -->
{#if vault.showBetaWarning}
<!-- svelte-ignore a11y_click_events_have_key_events a11y_interactive_supports_focus -->
<div
  class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50"
  onclick={cancelMainnet}
  onkeydown={(e) => { if (e.key === 'Escape') cancelMainnet(); }}
  role="dialog"
  aria-modal="true"
  aria-label="Mainnet beta warning"
  tabindex="-1"
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="bg-vault-900 border border-strong rounded-xl shadow-2xl max-w-sm w-full mx-4 p-6"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="flex items-start gap-3 mb-4">
      <span class="text-2xl">⚠️</span>
      <div>
        <h3 class="text-base font-semibold text-primary">Mainnet is in Beta</h3>
        <p class="text-sm text-secondary mt-1 leading-relaxed">
          Real assets are at risk. Are you sure?
        </p>
      </div>
    </div>
    <div class="flex gap-3 justify-end">
      <button
        class="px-4 py-2 rounded-lg text-sm font-medium bg-surface text-primary hover:bg-surface-hover border border-strong/50 transition-colors"
        onclick={cancelMainnet}
      >
        Cancel
      </button>
      <button
        class="px-4 py-2 rounded-lg text-sm font-medium bg-red-700 text-white hover:bg-red-600 transition-colors"
        onclick={confirmMainnet}
      >
        Continue
      </button>
    </div>
  </div>
</div>
{/if}
