<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface DebugReport {
    version: string;
    os: string;
    arch: string;
    build_date: string;
    plugins: { id: string; name: string; networks: string[]; capabilities: string[] }[];
    accounts: { network: string; address: string; path: string | null }[];
    env_config: { testnet_only: boolean; tor_enabled: boolean; auto_lock_seconds: number };
    recent_crashes: Record<string, unknown>[];
  }

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let report = $state<DebugReport | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  // Editable review state — users can redact lines before export
  let redactedItems = $state<Set<number>>(new Set());
  let userComments = $state('');
  let showReport = $state(false);

  let copied = $state(false);
  let uploadStatus = $state<'idle' | 'uploading' | 'done' | 'error'>('idle');

  async function generate() {
    loading = true;
    error = null;
    try {
      const result = await invoke('generate_debug_report') as DebugReport;
      report = result;
      showReport = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function toggleRedact(idx: number) {
    const next = new Set(redactedItems);
    if (next.has(idx)) {
      next.delete(idx);
    } else {
      next.add(idx);
    }
    redactedItems = next;
  }

  function formatReportAsText(includeComments: boolean): string {
    if (!report) return '';
    const lines: string[] = [];
    let idx = 0;

    lines.push('# Gullbúr Enclave Core — Debug Report');
    lines.push(`Generated: ${new Date().toISOString()}`);
    lines.push('');

    lines.push('## Version Info');
    if (!redactedItems.has(idx++)) lines.push(`- Version: ${report.version}`);
    if (!redactedItems.has(idx++)) lines.push(`- OS: ${report.os} / ${report.arch}`);
    if (!redactedItems.has(idx++)) lines.push(`- Build date: ${report.build_date}`);
    lines.push('');

    lines.push('## Plugins');
    for (const p of report.plugins) {
      if (!redactedItems.has(idx++)) {
        lines.push(`- **${p.name}** (\`${p.id}\`)`);
        lines.push(`  - Networks: ${p.networks.join(', ')}`);
        lines.push(`  - Capabilities: ${p.capabilities.join(', ')}`);
      }
    }
    lines.push('');

    lines.push('## Accounts');
    if (report.accounts.length === 0) {
      lines.push('*No accounts created yet*');
      if (!redactedItems.has(idx++)) idx++;
    } else {
      for (const a of report.accounts) {
        if (!redactedItems.has(idx++)) {
          lines.push(`- ${a.network}: \`${a.address}\``);
        }
      }
    }

    // Always show config (non-sensitive)
    lines.push('');
    lines.push('## Config');
    lines.push(`- Testnet-only: ${report.env_config.testnet_only}`);
    lines.push(`- Tor enabled: ${report.env_config.tor_enabled}`);
    lines.push(`- Auto-lock: ${report.env_config.auto_lock_seconds}s`);

    lines.push('');
    lines.push('## Recent Crashes');
    if (report.recent_crashes.length === 0) {
      lines.push('*No crash reports found*');
    } else {
      for (const crash of report.recent_crashes) {
        if (!redactedItems.has(idx++)) {
          lines.push('```json');
          lines.push(JSON.stringify(crash, null, 2));
          lines.push('```');
        }
      }
    }

    if (includeComments && userComments.trim()) {
      lines.push('');
      lines.push('## User Comments');
      lines.push(userComments);
    }

    return lines.join('\n');
  }

  async function copyReport() {
    try {
      const text = formatReportAsText(true);
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => copied = false, 2000);
    } catch { /* fallback */ }
  }

  async function uploadToGitHub() {
    if (!report) return;
    uploadStatus = 'uploading';
    try {
      const body = formatReportAsText(true);
      const title = `Debug Report: ${report.version} / ${report.os}`;

      // GitHub issue URL with pre-filled body
      // REPLACE_ME: Change MarcusBrammeier and gullbur-enclave to your actual GitHub org and repo name
      const repo = 'MarcusBrammeier/gullbur-enclave';
      const baseUrl = `https://github.com/${repo}/issues/new`;
      const params = new URLSearchParams({
        title: title,
        labels: 'debug-report',
        body: body,
      });
      window.open(`${baseUrl}?${params.toString()}`, '_blank');
      uploadStatus = 'done';
    } catch {
      uploadStatus = 'error';
    }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
  role="dialog"
  aria-modal="true"
  aria-label="Debug Report"
  tabindex="-1"
  onclick={handleBackdropClick}
  onkeydown={(e) => { if (e.key === 'Escape') onclose(); }}
>
  <div class="bg-surface-dim border border-strong rounded-2xl shadow-2xl max-w-2xl w-full mx-4 p-6 max-h-[85vh] flex flex-col" role="document">
    <div class="flex items-center justify-between mb-4 shrink-0">
      <h2 class="text-lg font-semibold">🔍 Debug Report</h2>
      <button class="text-muted hover:text-primary text-xl leading-none" onclick={onclose}>&times;</button>
    </div>

    {#if !showReport}
      <!-- Pre-generation info -->
      <div class="flex-1 overflow-y-auto space-y-4">
        <div class="bg-blue-900/20 border border-blue-700/30 rounded-xl p-4 text-sm text-blue-300 space-y-2">
          <p>⚠️ <strong>This report is designed to be safe for sharing.</strong></p>
          <p>It includes your wallet addresses, plugin config, and crash data — but <strong>never</strong> your seed phrase, private keys, or balances.</p>
          <p class="text-secondary">Review the report below before sharing. You can redact any line you're not comfortable with.</p>
        </div>

        <button
          class="btn-primary w-full"
          disabled={loading}
          onclick={generate}
        >
          {loading ? '⏳ Generating...' : '🔍 Generate Debug Report'}
        </button>

        {#if error}
          <div class="bg-red-900/30 border border-red-800 rounded-lg px-4 py-3 text-sm text-red-300">❌ {error}</div>
        {/if}
      </div>
    {:else if report}
      <!-- Report view -->
      <div class="flex-1 overflow-y-auto space-y-4">
        <div class="flex items-center gap-2 text-xs text-muted">
          <span class="px-2 py-0.5 bg-surface rounded">v{report.version}</span>
          <span>{report.os}/{report.arch}</span>
        </div>

        <!-- Redactable items -->
        <div class="space-y-1">
          {#each report.accounts as acct, idx}
            <label class="flex items-start gap-2 py-1 px-2 rounded hover:bg-surface/50 cursor-pointer text-sm">
              <input
                type="checkbox"
                class="mt-0.5 accent-vault-500"
                checked={redactedItems.has(idx)}
                onchange={() => toggleRedact(idx)}
              />
              <span class:line-through={redactedItems.has(idx)} class:opacity-40={redactedItems.has(idx)}>
                <span class="text-secondary">{acct.network}:</span> <span class="font-mono text-primary">{acct.address}</span>
              </span>
            </label>
          {/each}
        </div>

        <!-- Crashes -->
        {#if report.recent_crashes.length > 0}
          <div class="bg-amber-900/10 border border-amber-700/20 rounded-lg p-3 text-xs">
            <p class="font-medium text-amber-400 mb-2">📋 {report.recent_crashes.length} crash report(s) found</p>
            {#each report.recent_crashes as crash}
              <pre class="text-secondary mt-1 overflow-x-auto">{JSON.stringify(crash, null, 2)}</pre>
            {/each}
          </div>
        {:else}
          <p class="text-xs text-muted">No crash reports found</p>
        {/if}

        <!-- User comments -->
        <div>
          <label for="debug-comments" class="block text-xs text-secondary mb-1">Add comments for the developer:</label>
          <textarea
            id="debug-comments"
            class="input-field w-full h-20 text-sm resize-none"
            placeholder="e.g. I was trying to send LTC on testnet when..."
            bind:value={userComments}
          ></textarea>
        </div>
      </div>

      <!-- Preview -->
      {#if report.accounts.length > 0 || userComments}
        <details class="mt-3">
          <summary class="text-xs text-muted cursor-pointer hover:text-primary">📄 Preview report text</summary>
          <pre class="mt-2 bg-surface-elevated border border-default rounded-lg p-3 text-xs text-secondary max-h-40 overflow-y-auto">{formatReportAsText(true)}</pre>
        </details>
      {/if}

      <!-- Action buttons -->
      <div class="flex gap-2 mt-4 shrink-0">
        <button class="btn-secondary text-sm flex-1" onclick={copyReport}>
          {copied ? '✅ Copied!' : '📋 Copy Report'}
        </button>
        <button
          class="btn-primary text-sm flex-1"
          disabled={uploadStatus === 'uploading'}
          onclick={uploadToGitHub}
        >
          {uploadStatus === 'uploading' ? '⏳ Opening...'
            : uploadStatus === 'done' ? '✅ Opened'
            : uploadStatus === 'error' ? '❌ Failed'
            : '⬆️ Open GitHub Issue'}
        </button>
      </div>
    {/if}
  </div>
</div>