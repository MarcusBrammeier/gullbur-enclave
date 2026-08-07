<script lang="ts">
  import { currentToast, dismissToast } from '../toasts.svelte.ts';

  // Toast shown is a getter function (Svelte 5 forbids exporting $derived).
  let ct = $derived(currentToast());

  // Tailwind-style classes per level; no pure white / no pure black per theme.
  const styles: Record<string, string> = {
    error: 'bg-red-900/90 text-red-50 border-red-500',
    warning: 'bg-amber-900/90 text-amber-50 border-amber-500',
    info: 'bg-surface-dim text-primary border-vault-500',
  };

  const icons: Record<string, string> = {
    error: '⛔',
    warning: '⚠️',
    info: 'ℹ️',
  };
</script>

<!-- One toast at a time, fixed top-center, auto-advances after ~3s -->
{#if ct}
  <div class="fixed top-4 left-1/2 -translate-x-1/2 z-[100] w-full max-w-md px-4" role="status" aria-live="assertive">
    <div class="flex items-start gap-3 border rounded-lg shadow-2xl px-4 py-3 {styles[ct.level] ?? styles.info}">
      <span class="text-lg leading-none mt-0.5">{icons[ct.level] ?? 'ℹ️'}</span>
      <span class="flex-1 text-sm font-medium break-words">{ct.message}</span>
      <button
        class="shrink-0 text-current/70 hover:text-current transition-colors px-1"
        aria-label="Dismiss notification"
        onclick={dismissToast}
      >✕</button>
    </div>
  </div>
{/if}
