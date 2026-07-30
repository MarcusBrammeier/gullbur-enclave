<script lang="ts">
  import { vault, networkCount } from '../vault.svelte.ts';
  import TorToggle from './TorToggle.svelte';
  import UpdateBanner from './UpdateBanner.svelte';

  // ── Derived display values ─────────────────────────────────────────────

  const dotColor = $derived(vault.connected ? 'bg-vault-500' : 'bg-red-500');
  const dotShadow = $derived(vault.connected ? 'shadow-[0_0_6px_#22c55e]' : 'shadow-[0_0_6px_#ef4444]');
</script>

<footer class="border-t border-gray-800 px-6 py-3">
  <div class="max-w-6xl mx-auto flex flex-col gap-1">
    <UpdateBanner />
    <div class="flex items-center justify-between text-xs text-gray-500">
      <!-- Left: version -->
    <span>Gullbúr Enclave Core v0.1.0</span>

    <!-- Right: status indicators -->
    <span class="flex items-center gap-4">
      <!-- Connection dot + status -->
      <span class="flex items-center gap-2">
        <span class="inline-block w-2.5 h-2.5 rounded-full {dotColor} {dotShadow}"></span>
        <span class="text-gray-400">{vault.vaultStatus}</span>
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