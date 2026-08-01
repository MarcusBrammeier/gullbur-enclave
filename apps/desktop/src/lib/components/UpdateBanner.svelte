<script lang="ts">
  import { onMount } from 'svelte';
  import { IS_DEMO } from '../constants';

  interface UpdateInfo {
    local_version: string;
    latest_version: string;
    up_to_date: boolean;
    release_url: string | null;
    release_notes: string | null;
    prerelease: boolean;
    error: string | null;
  }

  let checking = $state(true);
  let info = $state<UpdateInfo | null>(null);
  let dismissed = $state(false);

  onMount(async () => {
    try {
      if (!IS_DEMO) {
        const { invoke } = await import('@tauri-apps/api/core');
        const result = await invoke('check_for_updates') as UpdateInfo;
        info = result;
      }
    } catch (e) {
      // Silent — update check is non-critical
      console.debug('[UpdateCheck] failed:', e);
    } finally {
      checking = false;
    }
  });

  function openRelease() {
    if (info?.release_url) {
      window.open(info.release_url, '_blank');
    }
  }

  function dismiss() {
    dismissed = true;
  }
</script>

{#if !checking && info && !info.up_to_date && !dismissed}
  <div class="update-banner">
    <span class="update-text">
      🚀 <strong>v{info.latest_version}</strong> available
      {#if info.prerelease}
        <span class="prerelease-badge">pre-release</span>
      {/if}
      — <button class="link-btn" onclick={openRelease}>Download</button>
    </span>
    <button class="dismiss-btn" onclick={dismiss}>✕</button>
  </div>
{/if}

<style>
  .update-banner {
    background: linear-gradient(135deg, #1e3a5f, #0d2744);
    border: 1px solid #2d5a8a;
    border-radius: 8px;
    padding: 6px 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    font-size: 0.8rem;
    color: #b0d4f1;
    animation: slideDown 0.3s ease-out;
  }

  @keyframes slideDown {
    from { opacity: 0; transform: translateY(-8px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  .update-text {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .prerelease-badge {
    font-size: 0.65rem;
    background: #f59e0b;
    color: #000;
    padding: 1px 6px;
    border-radius: 4px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .link-btn {
    background: none;
    border: none;
    color: #60a5fa;
    cursor: pointer;
    text-decoration: underline;
    font-size: inherit;
    padding: 0;
  }

  .link-btn:hover {
    color: #93c5fd;
  }

  .dismiss-btn {
    background: none;
    border: none;
    color: #6b8fba;
    cursor: pointer;
    font-size: 0.85rem;
    padding: 0 4px;
    line-height: 1;
  }

  .dismiss-btn:hover {
    color: #b0d4f1;
  }
</style>