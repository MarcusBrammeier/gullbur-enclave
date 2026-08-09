<script lang="ts">
  import { vault } from '../vault.svelte.ts';
  import { iconHtml } from '../icons';

  // ── Local state ──────────────────────────────────────────────────────────

  let secondsLeft = $state(30);
  let confirmError = $state<string | null>(null);
  let cooldown = $state(false);
  let locked = $state(false);

  // ── Derived ──────────────────────────────────────────────────────────────

  const isVisible = $derived(vault.authStatus === 'hardware_required');
  const progressPercent = $derived((secondsLeft / vault.authTimeout) * 100);
  const barColor = $derived(secondsLeft > 10 ? 'bg-amber-500' : secondsLeft > 5 ? 'bg-orange-500' : 'bg-red-500');

  // ── Timer — reactive to authStatus changes ───────────────────────────────

  $effect(() => {
    if (vault.authStatus === 'hardware_required') {
      // Reset timer state
      secondsLeft = vault.authTimeout;
      confirmError = null;
      cooldown = false;
      locked = false;
      vault.authStartedAt = Date.now();

      const interval = setInterval(() => {
        const elapsed = (Date.now() - vault.authStartedAt) / 1000;
        secondsLeft = Math.max(0, vault.authTimeout - Math.floor(elapsed));

        if (secondsLeft <= 0) {
          clearInterval(interval);
          lockVault();
        }
      }, 200);

      return () => {
        clearInterval(interval);
      };
    }
  });

  // ── Handlers ─────────────────────────────────────────────────────────────

  async function confirmHardware() {
    if (cooldown || locked) return;
    confirmError = null;

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('confirm_hardware');
      vault.authStatus = 'biometric_unlocked';
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      confirmError = msg;
      cooldown = true;

      // Check for hardware lockout
      if (msg.toLowerCase().includes('lockout') || msg.toLowerCase().includes('brute')) {
        locked = true;
        return;
      }

      // 2-second visual cooldown
      setTimeout(() => {
        cooldown = false;
      }, 2000);
    }
  }

  async function lockVault() {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('lock_vault');
    } catch {
      // Fallback: set local state
    }
    vault.authStatus = 'unauthenticated';
    vault.authStartedAt = 0;
  }

  function cancel() {
    void lockVault();
  }
</script>

{#if isVisible}
  <!-- Overlay backdrop -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
    role="dialog"
    aria-modal="true"
    aria-label="Hardware authentication required"
  >
    <div class="bg-surface-dim border border-strong rounded-xl p-8 max-w-md w-full mx-4 shadow-2xl">
      {#if locked}
        <!-- Lockout state -->
        <div class="text-center">
          <div class="inline-flex items-center justify-center mb-4">{@html iconHtml('shield', 'w-8 h-8')}</div>
          <h2 class="text-lg font-semibold text-red-400 mb-2">Security Lockout</h2>
          <p class="text-sm text-secondary mb-4">
            Hardware authentication has been locked due to repeated failures.
            Manual intervention is required.
          </p>
          <p class="text-xs font-mono text-muted mb-4">{confirmError}</p>
          <button class="btn-secondary text-sm" onclick={cancel}>
            Close
          </button>
        </div>
      {:else}
        <!-- Normal auth prompt -->
        <div class="text-center mb-6">
          <div class="inline-flex items-center justify-center mb-3">{@html iconHtml('lock', 'w-8 h-8')}</div>
          <h2 class="text-lg font-semibold text-amber-400 mb-1">Hardware Authentication Required</h2>
          <p class="text-sm text-secondary">
            Touch your YubiKey or use biometrics to authorize this operation.
          </p>
        </div>

        <!-- Progress bar -->
        <div class="mb-6">
          <div class="flex justify-between text-xs text-muted mb-1.5">
            <span>Auto-lock in</span>
            <span class="font-mono {secondsLeft <= 10 ? 'text-red-400' : ''}">{secondsLeft}s</span>
          </div>
          <div class="w-full h-2 bg-surface-hover rounded-full overflow-hidden">
            <div
              class="h-full rounded-full transition-all duration-200 ease-linear {barColor}"
              style="width: {progressPercent}%"
            ></div>
          </div>
        </div>

        <!-- Error message -->
        {#if confirmError}
          <div
            class="mb-4 p-3 bg-red-900/30 border border-red-800 rounded-lg {cooldown ? 'animate-pulse' : ''}"
          >
            <p class="text-xs font-mono text-red-400">{confirmError}</p>
          </div>
        {/if}

        <!-- Actions -->
        <div class="flex items-center gap-3">
          <button
            class="flex-1 px-4 py-2.5 rounded-lg text-sm font-medium transition-all
              {cooldown
                ? 'bg-surface-hover text-muted cursor-not-allowed'
                : 'bg-amber-600 hover:bg-amber-500 text-white'}"
            onclick={confirmHardware}
            disabled={cooldown}
          >
            {#if cooldown}
              Retry in 2s…
            {:else}
              Confirm
            {/if}
          </button>
          <button
            class="px-4 py-2.5 rounded-lg text-sm font-medium bg-surface hover:bg-surface-hover text-primary transition-colors"
            onclick={cancel}
          >
            Cancel
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}