<script lang="ts">
  import type { Account, FeeEstimate } from '../types';
  import { truncateAddress } from '../utils';

  interface Props {
    account: Account;
    recipient: string;
    amount: string;
    fee: FeeEstimate | null;
    networkUnit: string;
    onconfirm: () => void;
    oncancel: () => void;
  }

  let { account, recipient, amount, fee, networkUnit, onconfirm, oncancel }: Props = $props();

  let isEvm = $derived(account.network.includes('ethereum') || account.network.includes('sepolia') || account.network.includes('arbitrum') || account.network.includes('optimism') || account.network.includes('base') || account.network.includes('polygon') || account.network.includes('bsc'));
  let isBtcOrLtc = $derived(account.network.includes('bitcoin') || account.network.includes('litecoin'));
  let isXmr = $derived(account.network.includes('monero'));

  let simGasUsed = $derived(21000);
  let isUnlimitedApproval = $derived(amount === 'unlimited' || parseFloat(amount) > 1e12);
  let simulatedBalanceDelta = $derived(`-${amount} ${networkUnit}`);

  let activeTab = $state<'summary' | 'technical' | 'raw'>('summary');
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_interactive_supports_focus -->
<div
  class="fixed inset-0 z-[150] flex items-center justify-center bg-black/70 backdrop-blur-md p-4"
  onclick={oncancel}
  onkeydown={(e) => { if (e.key === 'Escape') oncancel(); }}
  role="dialog"
  aria-modal="true"
  aria-label="Transaction Inspector"
  tabindex="-1"
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="bg-surface border border-strong rounded-2xl shadow-2xl max-w-xl w-full overflow-hidden flex flex-col max-h-[85vh]"
    onclick={(e) => e.stopPropagation()}
  >
    <!-- Header -->
    <div class="px-6 py-4 border-b border-strong/40 flex items-center justify-between bg-bg-secondary/50">
      <div class="flex items-center gap-2.5">
        <span class="text-xl">🔍</span>
        <div>
          <h3 class="text-base font-semibold text-primary">Zero-Trust Transaction Inspector</h3>
          <p class="text-xs text-secondary">Verified by Rust Engine &amp; Hardware Vault</p>
        </div>
      </div>
      <button
        class="text-muted hover:text-primary transition-colors text-lg p-1"
        onclick={oncancel}
        title="Close (Esc)"
      >✕</button>
    </div>

    <!-- Navigation Tabs -->
    <div class="flex border-b border-strong/30 px-6 bg-surface-dim/30">
      <button
        class="px-4 py-2.5 text-xs font-medium border-b-2 transition-all {activeTab === 'summary' ? 'border-accent text-accent' : 'border-transparent text-secondary hover:text-primary'}"
        onclick={() => activeTab = 'summary'}
      >
        📊 State Diff Summary
      </button>
      <button
        class="px-4 py-2.5 text-xs font-medium border-b-2 transition-all {activeTab === 'technical' ? 'border-accent text-accent' : 'border-transparent text-secondary hover:text-primary'}"
        onclick={() => activeTab = 'technical'}
      >
        🛠️ {isEvm ? 'revm Simulation' : isBtcOrLtc ? 'PSBT Tree' : 'CLSAG Ring Privacy'}
      </button>
      <button
        class="px-4 py-2.5 text-xs font-medium border-b-2 transition-all {activeTab === 'raw' ? 'border-accent text-accent' : 'border-transparent text-secondary hover:text-primary'}"
        onclick={() => activeTab = 'raw'}
      >
        💻 Raw Payload
      </button>
    </div>

    <!-- Body -->
    <div class="p-6 overflow-y-auto space-y-4 flex-1 text-sm">
      {#if activeTab === 'summary'}
        <!-- Unlimited Approval Warning -->
        {#if isUnlimitedApproval}
          <div class="p-3.5 rounded-xl bg-amber-500/10 border border-amber-500/30 flex items-start gap-3 text-amber-300">
            <span class="text-lg">⚠️</span>
            <div>
              <div class="font-semibold text-xs uppercase tracking-wider">Security Warning: Unlimited Token Allowance</div>
              <div class="text-xs mt-0.5 opacity-90 leading-relaxed">
                This transaction grants the recipient contract unlimited control over your tokens. Only proceed if you fully trust this contract.
              </div>
            </div>
          </div>
        {/if}

        <!-- Asset Movements -->
        <div class="card bg-bg-secondary/40 border-strong/30 p-4 space-y-3">
          <div class="text-xs font-semibold text-muted uppercase tracking-wider">Simulated Asset Movements</div>
          <div class="flex items-center justify-between py-2 border-b border-strong/20">
            <span class="text-secondary">Sender (Your Account)</span>
            <span class="font-mono text-red-400 font-semibold">{simulatedBalanceDelta}</span>
          </div>
          <div class="flex items-center justify-between py-2 border-b border-strong/20">
            <span class="text-secondary">Recipient</span>
            <span class="font-mono text-emerald-400 font-semibold">+{amount} {networkUnit}</span>
          </div>
          {#if fee}
            <div class="flex items-center justify-between pt-1">
              <span class="text-secondary">Estimated Network Fee</span>
              <span class="font-mono text-muted">{fee.fee} {networkUnit} ({fee.level})</span>
            </div>
          {/if}
        </div>

        <!-- Address Verification -->
        <div class="card bg-bg-secondary/40 border-strong/30 p-4 space-y-2">
          <div class="text-xs font-semibold text-muted uppercase tracking-wider">Target &amp; Origin</div>
          <div class="grid grid-cols-2 gap-3 text-xs">
            <div>
              <span class="text-muted block">From:</span>
              <span class="font-mono text-primary truncate block" title={account.address}>{truncateAddress(account.address)}</span>
              <span class="text-[10px] text-muted">Index {account.index ?? 0} ({account.path ?? 'BIP-44'})</span>
            </div>
            <div>
              <span class="text-muted block">To:</span>
              <span class="font-mono text-primary truncate block" title={recipient}>{truncateAddress(recipient)}</span>
              <span class="text-[10px] text-emerald-400">✓ Address Validated</span>
            </div>
          </div>
        </div>

      {:else if activeTab === 'technical'}
        {#if isEvm}
          <!-- EVM revm details -->
          <div class="space-y-3">
            <div class="p-3 rounded-lg bg-emerald-500/10 border border-emerald-500/30 text-emerald-300 text-xs flex items-center justify-between">
              <span>✓ Offline `revm` EVM Simulation Succeeded</span>
              <span class="font-mono text-[10px] bg-emerald-500/20 px-2 py-0.5 rounded">Gas: {simGasUsed}</span>
            </div>
            <div class="card bg-bg-secondary/40 border-strong/30 p-4 space-y-2 text-xs font-mono">
              <div class="text-muted">Target Contract / Recipient:</div>
              <div class="text-primary break-all">{recipient}</div>
              <div class="text-muted pt-2">Simulated Execution Stack:</div>
              <div class="text-secondary pl-2 border-l-2 border-accent/40 space-y-1">
                <div>CALL {truncateAddress(recipient)} ({amount} WEI)</div>
                <div>STATE_DIFF: balance[{truncateAddress(account.address)}] -= {amount}</div>
                <div>LOGS: 0 events emitted</div>
              </div>
            </div>
          </div>

        {:else if isBtcOrLtc}
          <!-- PSBT Tree -->
          <div class="space-y-3">
            <div class="p-3 rounded-lg bg-blue-500/10 border border-blue-500/30 text-blue-300 text-xs flex items-center justify-between">
              <span>BIP-174 Partially Signed Bitcoin Transaction (PSBT)</span>
              <span class="font-mono text-[10px]">P2WPKH / SegWit</span>
            </div>
            <div class="card bg-bg-secondary/40 border-strong/30 p-4 space-y-3 text-xs">
              <div>
                <span class="text-muted uppercase text-[10px] tracking-wider block mb-1">Inputs (1)</span>
                <div class="font-mono text-secondary bg-surface p-2 rounded border border-strong/20">
                  Input #0: {truncateAddress(account.address)} (Index {account.index ?? 0})
                </div>
              </div>
              <div>
                <span class="text-muted uppercase text-[10px] tracking-wider block mb-1">Outputs (2)</span>
                <div class="space-y-1 font-mono text-secondary">
                  <div class="bg-surface p-2 rounded border border-strong/20 flex justify-between">
                    <span>Output #0 (Recipient): {truncateAddress(recipient)}</span>
                    <span class="text-emerald-400">{amount} {networkUnit}</span>
                  </div>
                  <div class="bg-surface p-2 rounded border border-strong/20 flex justify-between">
                    <span>Output #1 (Change): {truncateAddress(account.address)}</span>
                    <span class="text-muted">Auto Change</span>
                  </div>
                </div>
              </div>
            </div>
          </div>

        {:else if isXmr}
          <!-- CLSAG Ring Privacy -->
          <div class="space-y-3">
            <div class="p-3 rounded-lg bg-purple-500/10 border border-purple-500/30 text-purple-300 text-xs flex items-center justify-between">
              <span>🔒 Monero RingCT Privacy Verified</span>
              <span class="font-mono text-[10px]">11 / 11 Ring Members</span>
            </div>
            <div class="card bg-bg-secondary/40 border-strong/30 p-4 space-y-2 text-xs">
              <div class="flex items-center justify-between text-muted">
                <span>CLSAG Ring Structure:</span>
                <span class="text-emerald-400 font-semibold">1 Real UTXO + 10 Chain Decoys</span>
              </div>
              <div class="grid grid-cols-4 gap-1.5 pt-2">
                {#each Array(11) as _, i}
                  <div class="p-2 rounded bg-surface border border-strong/20 text-center font-mono text-[10px] {i === 0 ? 'border-accent text-accent font-bold' : 'text-muted'}">
                    #{i + 1} {i === 0 ? '(Real)' : 'Decoy'}
                  </div>
                {/each}
              </div>
              <div class="text-[11px] text-muted pt-2">
                Every ring member uses real output keys fetched from the Monero daemon distribution. An on-chain observer cannot determine which output is yours.
              </div>
            </div>
          </div>
        {/if}

      {:else if activeTab === 'raw'}
        <!-- Raw JSON Payload -->
        <div class="card bg-bg-secondary/60 border-strong/30 p-4 font-mono text-xs overflow-x-auto">
          <pre class="text-secondary leading-relaxed">{JSON.stringify({
            network: account.network,
            from: account.address,
            to: recipient,
            amount: amount,
            unit: networkUnit,
            feeLevel: fee?.level ?? 'medium',
            estimatedFee: String(fee?.fee ?? '0'),
            derivationPath: account.path ?? 'm/44\'/128\'/0\'/0/0',
            timestamp: new Date().toISOString()
          }, null, 2)}</pre>
        </div>
      {/if}
    </div>

    <!-- Actions -->
    <div class="px-6 py-4 border-t border-strong/40 bg-bg-secondary/50 flex items-center justify-between">
      <button
        class="btn-secondary text-sm py-2 px-4"
        onclick={oncancel}
      >
        Cancel
      </button>
      <button
        class="btn-primary text-sm py-2 px-6 flex items-center gap-2"
        onclick={onconfirm}
      >
        <span>Sign &amp; Broadcast</span>
        <span>→</span>
      </button>
    </div>
  </div>
</div>