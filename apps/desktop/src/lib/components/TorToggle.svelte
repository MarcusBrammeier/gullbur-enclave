<script lang="ts">
  import { vault } from '../vault.svelte.ts';

  // ── Props ──────────────────────────────────────────────────────────────────

  let { showLabel = true, size = 'sm' as 'sm' | 'md' }: {
    showLabel?: boolean;
    size?: 'sm' | 'md';
  } = $props();

  // ── Local state ────────────────────────────────────────────────────────────

  let switching = $state(false);
  let errorMsg = $state<string | null>(null);

  // ── Derived ────────────────────────────────────────────────────────────────

  const isOn = $derived(vault.torEnabled);
  // dotColor unused — TorToggle uses SVG+text classes directly
  // dotShadow unused — kept for potential future glow effect
  const labelText = $derived(isOn ? 'Tor: On' : 'Tor: Off');

  // ── Handlers ───────────────────────────────────────────────────────────────

  async function toggleTor() {
    if (switching) return;
    switching = true;
    errorMsg = null;

    try {
      // Call the Tauri backend to start/stop the arti daemon
      if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('toggle_tor', { enabled: !isOn });
      } else {
        // No Tauri runtime (browser extension context) — toggle local state only
        vault.torEnabled = !isOn;
      }
      vault.torEnabled = !isOn;
    } catch (e) {
      console.error('[tor] Toggle failed:', e);
      errorMsg = e instanceof Error ? e.message : 'Tor toggle failed';
      // Auto-clear error after 5s
      setTimeout(() => { errorMsg = null; }, 5000);
      // Do NOT flip the UI — backend didn't actually change
    } finally {
      switching = false;
    }
  }
</script>

<button
  class="flex items-center gap-1.5 {size === 'sm' ? 'text-xs' : 'text-sm'} text-muted hover:text-primary transition-colors"
  onclick={toggleTor}
  disabled={switching}
  title={isOn ? 'Disable Tor routing' : 'Enable Tor routing'}
>
  <!-- Tor onion icon -->
  <svg
    class="w-3.5 h-3.5 {isOn ? 'text-purple-400' : 'text-muted'}"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <circle cx="12" cy="12" r="10" />
    <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10A15.3 15.3 0 0 1 12 2z" />
    <circle cx="12" cy="12" r="4" />
  </svg>

  {#if showLabel}
    <span class={isOn ? 'text-purple-400' : ''}>{labelText}</span>
  {/if}

  {#if switching}
    <span class="animate-spin inline-block w-2.5 h-2.5 border-2 border-strong border-t-transparent rounded-full"></span>
  {/if}
</button>

{#if errorMsg}
  <div class="text-xs text-red-400 mt-1 max-w-48" title={errorMsg}>⚠️ Tor unavailable</div>
{/if}