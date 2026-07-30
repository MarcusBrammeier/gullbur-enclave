<script lang="ts">
  import type { TxRecord } from '../types';

  let { transactions = [], loading = false }: { transactions: TxRecord[]; loading: boolean } = $props();

  let filter = $state<'all' | 'sent' | 'received'>('all');
  let copiedTxid = $state<string | null>(null);

  const filteredTransactions = $derived(
    filter === 'all'
      ? transactions
      : transactions.filter((tx) => tx.direction === filter)
  );

  const filters = [
    { key: 'all' as const, label: 'All' },
    { key: 'sent' as const, label: 'Sent' },
    { key: 'received' as const, label: 'Received' },
  ];

  function truncateAddress(addr: string): string {
    if (addr.length <= 12) return addr;
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
  }

  function truncateTxid(txid: string): string {
    if (txid.length <= 16) return txid;
    return `${txid.slice(0, 8)}...${txid.slice(-6)}`;
  }

  async function copyToClipboard(text: string, txid: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedTxid = txid;
      setTimeout(() => {
        copiedTxid = null;
      }, 2000);
    } catch {
      // clipboard unavailable
    }
  }

  function formatTimestamp(ts: number): string {
    const date = new Date(ts * 1000);
    return date.toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  function statusLabel(status: string): string {
    switch (status) {
      case 'confirmed': return 'Confirmed';
      case 'pending': return 'Pending';
      case 'failed': return 'Failed';
      default: return status;
    }
  }
</script>

<div class="card">
  <h2 class="text-lg font-semibold mb-4">📋 Transactions</h2>

  <!-- Filter Tabs -->
  <div class="flex gap-1 mb-4 p-1 bg-gray-800/50 rounded-lg" role="tablist">
    {#each filters as { key, label }}
      <button
        class="flex-1 px-3 py-1.5 text-sm font-medium rounded-md transition-colors
          {filter === key
            ? 'bg-gray-700 text-gray-100'
            : 'text-gray-400 hover:text-gray-200'}"
        onclick={() => (filter = key)}
        role="tab"
        aria-selected={filter === key}
      >
        {label}
      </button>
    {/each}
  </div>

  <!-- Loading State -->
  {#if loading}
    <div class="space-y-3">
      {#each Array(4) as _}
        <div class="flex items-center gap-4 p-3 rounded-lg bg-gray-800/30 animate-pulse">
          <div class="flex-1 space-y-2">
            <div class="h-3 bg-gray-700 rounded w-28"></div>
            <div class="h-2.5 bg-gray-700/70 rounded w-44"></div>
          </div>
          <div class="text-right space-y-2">
            <div class="h-3 bg-gray-700 rounded w-16 ml-auto"></div>
            <div class="h-2.5 bg-gray-700/70 rounded w-20 ml-auto"></div>
          </div>
          <div class="h-5 bg-gray-700 rounded w-16"></div>
        </div>
      {/each}
    </div>
  {:else if filteredTransactions.length === 0}
    <!-- Empty State -->
    <div class="text-center py-10">
      <span class="text-4xl block mb-3">📭</span>
      <p class="text-gray-500 text-sm">
        {transactions.length === 0 ? 'No transactions yet' : `No ${filter} transactions`}
      </p>
    </div>
  {:else}
    <!-- Transaction List -->
    <div class="space-y-2">
      {#each filteredTransactions as tx (tx.txid)}
        <div class="flex items-center gap-4 p-3 rounded-lg bg-gray-800/20 hover:bg-gray-800/40 transition-colors">
          <!-- Txid -->
          <div class="flex-1 min-w-0">
            <button
              class="font-mono text-xs text-vault-400 hover:text-vault-300 transition-colors cursor-pointer truncate block max-w-[180px]"
              onclick={() => copyToClipboard(tx.txid, tx.txid)}
              title="Click to copy full txid"
            >
              {copiedTxid === tx.txid ? '✅ Copied!' : truncateTxid(tx.txid)}
            </button>
            <div class="text-xs text-gray-500 mt-0.5 truncate max-w-[300px]">
              <span class="text-gray-600">From</span> {truncateAddress(tx.from)}
              <span class="mx-1.5">→</span>
              <span class="text-gray-600">To</span> {truncateAddress(tx.to)}
            </div>
          </div>

          <!-- Amount -->
          <div class="text-right shrink-0">
            <span class="text-sm font-mono font-medium {tx.direction === 'sent' ? 'text-red-400' : 'text-green-400'}">
              {tx.direction === 'sent' ? '−' : '+'}{tx.amount}
            </span>
            <span class="text-xs text-gray-500 ml-1">{tx.unit}</span>
            {#if tx.timestamp}
              <div class="text-xs text-gray-600 mt-0.5">{formatTimestamp(tx.timestamp)}</div>
            {/if}
          </div>

          <!-- Status + Block Height -->
          <div class="shrink-0 flex flex-col items-end gap-1">
            <span
              class="inline-block text-xs font-medium px-2 py-0.5 rounded-full border
                {tx.status === 'pending'
                  ? 'text-yellow-400 bg-yellow-400/10 border-yellow-400/30'
                  : tx.status === 'confirmed'
                    ? 'text-green-400 bg-green-400/10 border-green-400/30'
                    : 'text-red-400 bg-red-400/10 border-red-400/30'}"
            >
              {statusLabel(tx.status)}
            </span>
            {#if tx.blockHeight}
              <span class="text-xs text-gray-600 font-mono">Block #{tx.blockHeight.toLocaleString()}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>