<script lang="ts">
  import { onMount } from 'svelte';

  interface LogEntry {
    id: number;
    timestamp: string;
    direction: 'send' | 'receive';
    method?: string;
    payload: string;
    isError: boolean;
  }

  let logs = $state<LogEntry[]>([]);
  let maxLogs = 200;
  let autoScroll = $state(true);
  let filterMode = $state<'all' | 'errors'>('all');
  let logContainer: HTMLDivElement | undefined = $state(undefined);

  let nextId = 0;

  // Expose log function globally so IpcClient can call it
  onMount(() => {
    (window as any).__consoleLog = (entry: Omit<LogEntry, 'id' | 'timestamp'>) => {
      const now = new Date();
      const ts = now.toTimeString().slice(0, 8) + '.' + String(now.getMilliseconds()).padStart(3, '0');
      logs = [{ id: nextId++, timestamp: ts, ...entry }, ...logs].slice(0, maxLogs);
    };
    return () => { delete (window as any).__consoleLog; };
  });

  $effect(() => {
    if (autoScroll && logContainer) {
      logContainer.scrollTop = 0;
    }
  });

  function copyAll() {
    const text = logs
      .filter(e => filterMode === 'errors' ? e.isError : true)
      .map(e => `[${e.timestamp}] ${e.direction === 'send' ? '→' : '←'} ${e.method ?? ''} ${e.isError ? '❌' : '✅'} ${e.payload}`)
      .join('\n');
    navigator.clipboard.writeText(text).catch(() => {});
  }

  let filteredLogs = $derived(
    filterMode === 'errors' ? logs.filter(e => e.isError) : logs
  );
</script>

<div class="flex flex-col h-full">
  <!-- Toolbar -->
  <div class="flex items-center justify-between mb-3 shrink-0">
    <div class="flex gap-2">
      <button
        class="text-xs px-3 py-1.5 rounded-lg transition-colors {filterMode === 'all' ? 'bg-vault-600 text-white' : 'bg-surface text-secondary hover:text-primary'}"
        onclick={() => filterMode = 'all'}
      >All ({logs.length})</button>
      <button
        class="text-xs px-3 py-1.5 rounded-lg transition-colors {filterMode === 'errors' ? 'bg-red-700 text-white' : 'bg-surface text-secondary hover:text-primary'}"
        onclick={() => filterMode = 'errors'}
      >Errors ({logs.filter(e => e.isError).length})</button>
    </div>
    <div class="flex gap-2">
      <button class="text-xs text-muted hover:text-primary" onclick={() => { logs = []; }}>Clear</button>
      <button class="text-xs text-vault-400 hover:text-vault-300" onclick={copyAll}>📋 Copy All</button>
      <label class="flex items-center gap-1 text-xs text-muted cursor-pointer">
        <input type="checkbox" bind:checked={autoScroll} class="w-3 h-3" />
        Auto-scroll
      </label>
    </div>
  </div>

  <!-- Log list -->
  <div
    class="flex-1 overflow-y-auto bg-surface-elevated border border-default rounded-lg font-mono text-xs space-y-0.5 p-2"
    bind:this={logContainer}
  >
    {#if filteredLogs.length === 0}
      <div class="text-muted text-center py-8 text-xs">
        {filterMode === 'errors' ? 'No errors logged' : 'No IPC calls yet'}
      </div>
    {:else}
      {#each filteredLogs as entry (entry.id)}
        <div
          class="flex gap-2 px-2 py-1 rounded hover:bg-surface-dim/50 {entry.isError ? 'bg-red-950/30 border-l-2 border-red-500' : ''}"
        >
          <span class="text-muted shrink-0 w-16">{entry.timestamp}</span>
          <span class="shrink-0 w-4 {entry.direction === 'send' ? 'text-vault-500' : 'text-blue-400'}">{entry.direction === 'send' ? '→' : '←'}</span>
          {#if entry.method}
            <span class="text-primary shrink-0 max-w-32 truncate">{entry.method}</span>
          {/if}
          <span class="{entry.isError ? 'text-red-400' : 'text-secondary'} truncate">{entry.payload}</span>
        </div>
      {/each}
    {/if}
  </div>
</div>