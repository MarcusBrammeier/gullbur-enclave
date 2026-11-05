<script lang="ts">
  /**
   * ApprovalModal — user-consent gate for pending state-changing operations
   * (H4-A).
   *
   * Polls `approval_list` on the Rust vault queue and renders every pending
   * request (method, description, origin, summary key/values) so the user can
   * Approve or Deny each one. Deny is the non-bypassable default posture.
   *
   * In GUI/demo mode the Tauri backend is absent, so polling no-ops and the
   * modal stays hidden.
   */
  import { IS_DEMO } from '../constants';
  import { pushError } from '../toasts.svelte.ts';

  interface ApprovalRequest {
    id: string;
    origin: string;
    method: string;
    description: string;
    summary: Record<string, string>;
    created_at: number;
  }

  const POLL_MS = 1000;

  let pending: ApprovalRequest[] = $state([]);
  let busyId: string | null = $state(null);
  let loaded = $state(false);

  // Imperative poll handle so tests can tear it down deterministically.
  let pollHandle: ReturnType<typeof setInterval> | null = null;

  async function loadPending(): Promise<void> {
    if (IS_DEMO || typeof window === 'undefined') return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const list = (await invoke('approval_list')) as ApprovalRequest[];
      pending = Array.isArray(list) ? list : [];
      loaded = true;
    } catch (e) {
      // Vault not yet reachable — stay quiet; the poll retries on the next tick.
      if (loaded) {
        // Only surface once (avoid toast spam every poll).
        loaded = false;
        pushError(e instanceof Error ? `approval_list: ${e.message}` : String(e));
      }
    }
  }

  async function respond(id: string, approved: boolean): Promise<void> {
    if (IS_DEMO || typeof window === 'undefined') return;
    busyId = id;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('approval_respond', { id, approved });
      // Drop it from the local list immediately; the next poll confirms.
      pending = pending.filter((r) => r.id !== id);
    } catch (e) {
      pushError(e instanceof Error ? `approval_respond: ${e.message}` : String(e));
    } finally {
      busyId = null;
    }
  }

  $effect(() => {
    if (IS_DEMO || typeof window === 'undefined') return;
    loadPending();
    if (pollHandle) clearInterval(pollHandle);
    pollHandle = setInterval(loadPending, POLL_MS);
    return () => {
      if (pollHandle) clearInterval(pollHandle);
      pollHandle = null;
    };
  });
</script>

{#if pending.length > 0}
  <!-- Overlay z-index sits above the header (z-50) and other modals. -->
  <div
    class="fixed inset-0 z-[160] flex items-center justify-center bg-black/70 backdrop-blur-md p-4"
    role="dialog"
    aria-modal="true"
    aria-label="Pending approvals"
    tabindex="-1"
  >
    <div
      class="bg-surface border border-strong rounded-2xl shadow-2xl max-w-xl w-full overflow-hidden flex flex-col max-h-[85vh]"
    >
      <!-- Header -->
      <div class="px-6 py-4 border-b border-strong/40 flex items-center justify-between bg-surface-dim/50">
        <div class="flex items-center gap-2.5">
          <span class="text-xl">🛡️</span>
          <div>
            <h3 class="text-base font-semibold text-primary">Approval Required</h3>
            <p class="text-xs text-secondary">
              {pending.length} pending {pending.length === 1 ? 'operation' : 'operations'} • review before it touches your keys
            </p>
          </div>
        </div>
      </div>

      <!-- List -->
      <div class="p-4 overflow-y-auto space-y-3 flex-1">
        {#each pending as req}
          <div class="card bg-bg-secondary/40 border-strong/30 p-4 space-y-3">
            <div class="flex items-start justify-between gap-3">
              <div class="flex items-center gap-2 min-w-0">
                <span class="text-lg shrink-0">🔐</span>
                <div class="min-w-0">
                  <div class="font-mono text-xs text-muted uppercase tracking-wider truncate">{req.method}</div>
                  <div class="text-sm font-medium text-primary mt-0.5">{req.description}</div>
                </div>
              </div>
              <span class="shrink-0 text-[10px] text-muted bg-surface px-2 py-0.5 rounded border border-strong/20">
                #{req.id}
              </span>
            </div>

            <!-- Origin -->
            <div class="text-xs">
              <span class="text-muted block">Requested by</span>
              <span class="font-mono text-accent break-all">{req.origin}</span>
            </div>

            <!-- Summary k/v -->
            {#if Object.keys(req.summary).length > 0}
              <div class="rounded-lg bg-surface border border-strong/20 p-3 space-y-1.5">
                {#each Object.entries(req.summary) as [key, value]}
                  <div class="flex items-start justify-between gap-3 text-xs">
                    <span class="text-muted shrink-0 uppercase tracking-wider text-[10px] pt-0.5">{key}</span>
                    <span class="font-mono text-primary break-all text-right">{value}</span>
                  </div>
                {/each}
              </div>
            {/if}

            <!-- Actions -->
            <div class="flex items-center justify-end gap-2 pt-1">
              <button
                class="btn-secondary text-sm py-2 px-4"
                disabled={busyId === req.id}
                onclick={() => respond(req.id, false)}
              >
                Deny
              </button>
              <button
                class="btn-primary text-sm py-2 px-6 flex items-center gap-2"
                disabled={busyId === req.id}
                onclick={() => respond(req.id, true)}
              >
                {busyId === req.id ? '…' : '✓'} Approve
              </button>
            </div>
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}