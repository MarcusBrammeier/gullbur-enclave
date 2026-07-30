<script lang="ts">
  import type { Account, FeeEstimate, Balance } from '../types';
  import { vault, validateAddress, estimateFee, signTransaction, broadcastTransaction, simulateTransfer, getAccountLabel } from '../vault.svelte.ts';

  interface Props {
    account: Account;
    onclose: () => void;
  }

  let { account, onclose }: Props = $props();

  type Step = 'address' | 'amount' | 'fee' | 'review' | 'signing' | 'result';

  let step = $state<Step>('address');
  let recipientAddress = $state('');
  let amount = $state('');
  let selectedFee = $state<'fast' | 'medium' | 'slow'>('medium');
  let feeEstimates = $state<FeeEstimate[]>([]);
  let feeLoading = $state(false);
  let addressValid = $state<boolean | null>(null);
  let addressValidating = $state(false);
  let addressError = $state('');
  let amountError = $state('');
  let txid = $state('');
  let resultError = $state('');
  let signing = $state(false);
  let simulating = $state(false);
  let simResult = $state<{ success: boolean; gasUsed: number; revertReason: string | null } | null>(null);

  let networkUnit = $derived.by(() => {
    const map: Record<string, string> = {
      bitcoin: 'BTC',
      ethereum: 'ETH',
      monero: 'XMR',
    };
    return map[account.network] ?? account.network.toUpperCase();
  });

  let currentFee = $derived(
    feeEstimates.find((f) => f.level === selectedFee) ?? null
  );

  let canProceedFromAddress = $derived(
    recipientAddress.length > 0 && addressValid === true
  );

  let canProceedFromAmount = $derived.by(() => {
    const val = parseFloat(amount);
    return !isNaN(val) && val > 0 && amountError === '';
  });

  function formatBalance(balance: Balance | null): string {
    if (!balance) return '0';
    return parseFloat(balance.confirmed).toLocaleString(undefined, { maximumFractionDigits: 8 });
  }

  function getBalanceFloat(balance: Balance | null): number {
    if (!balance) return 0;
    const val = parseFloat(balance.confirmed);
    return isNaN(val) ? 0 : val;
  }

  async function handleValidateAddress() {
    if (!recipientAddress.trim()) {
      addressValid = null;
      addressError = '';
      return;
    }

    addressValidating = true;
    addressError = '';
    try {
      const valid = await validateAddress(recipientAddress.trim(), account.network);
      addressValid = valid;
      if (!valid) {
        addressError = 'Invalid address for this network';
      }
    } catch (err) {
      addressValid = false;
      addressError = err instanceof Error ? err.message : 'Validation failed';
    } finally {
      addressValidating = false;
    }
  }

  function handleAmountInput(e: Event) {
    const input = e.target as HTMLInputElement;
    amount = input.value;
    amountError = '';

    const val = parseFloat(amount);
    if (amount !== '' && (isNaN(val) || val <= 0)) {
      amountError = 'Enter a valid positive amount';
    } else {
      const bal = getBalanceFloat(account.balance);
      if (val > bal) {
        amountError = 'Insufficient balance';
      }
    }
  }

  async function loadFeeEstimates() {
    feeLoading = true;
    try {
      const estimates = await estimateFee(
        account.network,
        recipientAddress.trim(),
        amount,
      );
      feeEstimates = estimates ?? [];
    } catch (err) {
      console.error('Failed to load fee estimates:', err);
      feeEstimates = [];
    } finally {
      feeLoading = false;
    }
  }

  async function goToFeeStep() {
    step = 'fee';
    await loadFeeEstimates();
  }

  async function handleSimulate() {
    simulating = true;
    simResult = null;
    try {
      const result = await simulateTransfer(
        account.network,
        account.address,
        recipientAddress.trim(),
        amount,
      );
      simResult = result;
    } catch (err) {
      simResult = { success: false, gasUsed: 0, revertReason: err instanceof Error ? err.message : 'Simulation failed' };
    } finally {
      simulating = false;
    }
  }

  async function handleSignAndBroadcast() {
    step = 'signing';
    signing = true;
    resultError = '';

    try {
      const signedTx = await signTransaction({
        from: account.address,
        to: recipientAddress.trim(),
        amount: amount,
        network: account.network,
        feeLevel: selectedFee,
      });

      const result = (await broadcastTransaction(signedTx)) as { txid: string };
      txid = result?.txid ?? 'Unknown';
      step = 'result';
    } catch (err) {
      resultError = err instanceof Error ? err.message : 'Transaction failed';
      step = 'result';
    } finally {
      signing = false;
    }
  }

  function getNetworkUnit(networkId: string): string {
    const map: Record<string, string> = { bitcoin: 'BTC', ethereum: 'ETH', monero: 'XMR' };
    return map[networkId] ?? networkId.toUpperCase();
  }

  function explorerTxUrl(networkId: string, txid: string): string | null {
    const urls: Record<string, string> = {
      bitcoin: 'https://mempool.space/tx/',
      'bitcoin-testnet': 'https://mempool.space/testnet/tx/',
      ethereum: 'https://etherscan.io/tx/',
      sepolia: 'https://sepolia.etherscan.io/tx/',
      monero: 'https://www.exploremonero.com/transaction/',
      'monero-stagenet': 'https://stagenet.exploremonero.com/transaction/',
      polygon: 'https://polygonscan.com/tx/',
      arbitrum: 'https://arbiscan.io/tx/',
      base: 'https://basescan.org/tx/',
      bnb: 'https://bscscan.com/tx/',
      optimism: 'https://optimistic.etherscan.io/tx/',
    };
    return urls[networkId] ? urls[networkId] + txid : null;
  }

  let txExplorerUrl = $derived(txid ? explorerTxUrl(account.network, txid) : null);

  function resetAndClose() {
    step = 'address';
    recipientAddress = '';
    amount = '';
    selectedFee = 'medium';
    feeEstimates = [];
    addressValid = null;
    addressError = '';
    amountError = '';
    txid = '';
    resultError = '';
    onclose();
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      resetAndClose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      resetAndClose();
    }
  }

  function feeLabelClass(fee: FeeEstimate, isSelected: boolean): string {
    let base = 'flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors ';
    if (isSelected) {
      base += 'border-vault-500 bg-vault-950/30 ';
    } else {
      base += 'border-gray-700 hover:border-gray-600 ';
    }
    return base;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Modal Backdrop -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
  onclick={handleBackdropClick}
  onkeydown={handleKeydown}
  role="dialog"
  aria-modal="true"
  aria-label="Send transaction"
  tabindex="-1"
>
  <!-- Modal Card -->
  <div class="card w-full max-w-md mx-4 max-h-[90vh] overflow-y-auto shadow-2xl border-gray-700">
    <!-- Header -->
    <div class="flex items-center justify-between mb-5">
      <h2 class="text-lg font-semibold">💸 Send {networkUnit}</h2>
      <button
        class="text-gray-500 hover:text-gray-300 transition-colors text-xl leading-none"
        onclick={resetAndClose}
        aria-label="Close"
      >
        ✕
      </button>
    </div>

    <!-- From account info -->
    <div class="bg-gray-800/50 border border-gray-700 rounded-lg p-3 mb-5">
      <span class="text-xs text-gray-500 block mb-1">From</span>
      <span class="font-mono text-sm text-gray-300">
        {getAccountLabel(account.address) ?? (account.address.length > 12
          ? `${account.address.slice(0, 8)}...${account.address.slice(-6)}`
          : account.address)}
      </span>
    </div>

    <!-- Step 1: Recipient Address -->
    {#if step === 'address'}
      <div class="space-y-4">
        <div>
          <label class="block text-sm text-gray-400 mb-1.5" for="recipient">
            Recipient Address
          </label>
          <input
            id="recipient"
            type="text"
            class="input-field w-full font-mono text-sm"
            class:border-red-500={addressValid === false}
            class:border-vault-500={addressValid === true}
            placeholder={`Enter ${networkUnit} address...`}
            bind:value={recipientAddress}
            onblur={handleValidateAddress}
          />
          {#if addressValidating}
            <p class="text-xs text-gray-500 mt-1">Validating...</p>
          {:else if addressValid === true}
            <p class="text-xs text-vault-400 mt-1">✓ Valid address</p>
          {:else if addressError}
            <p class="text-xs text-red-400 mt-1">{addressError}</p>
          {/if}
        </div>

        <button
          class="btn-primary w-full"
          disabled={!canProceedFromAddress}
          onclick={() => (step = 'amount')}
        >
          Continue
        </button>
      </div>

    <!-- Step 2: Amount -->
    {:else if step === 'amount'}
      <div class="space-y-4">
        <div>
          <label class="block text-sm text-gray-400 mb-1.5" for="amount">
            Amount ({networkUnit})
          </label>
          <div class="relative">
            <input
              id="amount"
              type="number"
              step="any"
              min="0"
              class="input-field w-full pr-16 font-mono"
              placeholder="0.00"
              value={amount}
              oninput={handleAmountInput}
            />
            <span class="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-gray-500 font-medium">
              {networkUnit}
            </span>
          </div>
          {#if amountError}
            <p class="text-xs text-red-400 mt-1">{amountError}</p>
          {/if}
          <p class="text-xs text-gray-500 mt-1">
            Available: {formatBalance(account.balance)} {networkUnit}
          </p>
        </div>

        <div class="flex gap-3">
          <button
            class="btn-secondary flex-1"
            onclick={() => (step = 'address')}
          >
            Back
          </button>
          <button
            class="btn-primary flex-1"
            disabled={!canProceedFromAmount}
            onclick={goToFeeStep}
          >
            Continue
          </button>
        </div>
      </div>

    <!-- Step 3: Fee Selection -->
    {:else if step === 'fee'}
      <div class="space-y-4">
        <div>
          <p class="block text-sm text-gray-400 mb-3">
            Transaction Fee
          </p>

          {#if feeLoading}
            <div class="text-center py-4 text-gray-500 text-sm">
              Loading fee estimates...
            </div>
          {:else if feeEstimates.length === 0}
            <div class="text-center py-4">
              <p class="text-gray-500 text-sm mb-2">No fee estimates available</p>
              <p class="text-xs text-gray-600">Using default fees</p>
            </div>
          {:else}
            <div class="space-y-2">
              {#each feeEstimates as fee (fee.level)}
                {@const isSelected = selectedFee === fee.level}
                <label
                  class={feeLabelClass(fee, isSelected)}
                >
                  <input
                    type="radio"
                    name="fee"
                    value={fee.level}
                    checked={isSelected}
                    onchange={() => (selectedFee = fee.level as 'fast' | 'medium' | 'slow')}
                    class="accent-vault-500"
                  />
                  <div class="flex-1">
                    <span class="text-sm font-medium capitalize text-gray-200">
                      {fee.level}
                    </span>
                    <span class="text-xs text-gray-500 ml-2">
                      {fee.estimatedTime ?? ''}
                    </span>
                  </div>
                  <span class="font-mono text-sm text-gray-300">
                    {typeof fee.fee === 'number'
                      ? fee.fee.toLocaleString(undefined, { maximumFractionDigits: 8 })
                      : fee.fee}
                  </span>
                </label>
              {/each}
            </div>
          {/if}
        </div>

        <div class="flex gap-3">
          <button
            class="btn-secondary flex-1"
            onclick={() => (step = 'amount')}
          >
            Back
          </button>
          <button
            class="btn-primary flex-1"
            onclick={() => (step = 'review')}
          >
            Review
          </button>
        </div>
      </div>

    <!-- Step 4: Review -->
    {:else if step === 'review'}
      <div class="space-y-4">
        <h3 class="text-sm font-semibold text-gray-300">Review Transaction</h3>

        <div class="bg-gray-800/50 border border-gray-700 rounded-lg divide-y divide-gray-700">
          <div class="flex justify-between px-4 py-3">
            <span class="text-sm text-gray-400">From</span>
            <span class="text-sm font-mono text-gray-300 max-w-[180px] truncate">
              {account.address.length > 12
                ? `${account.address.slice(0, 8)}...${account.address.slice(-6)}`
                : account.address}
            </span>
          </div>
          <div class="flex justify-between px-4 py-3">
            <span class="text-sm text-gray-400">To</span>
            <span class="text-sm font-mono text-gray-300 max-w-[180px] truncate">
              {recipientAddress.length > 12
                ? `${recipientAddress.slice(0, 8)}...${recipientAddress.slice(-6)}`
                : recipientAddress}
            </span>
          </div>
          <div class="flex justify-between px-4 py-3">
            <span class="text-sm text-gray-400">Amount</span>
            <span class="text-sm font-mono text-vault-400">
              {parseFloat(amount).toLocaleString(undefined, { maximumFractionDigits: 8 })} {networkUnit}
            </span>
          </div>
          <div class="flex justify-between px-4 py-3">
            <span class="text-sm text-gray-400">Fee</span>
            <span class="text-sm font-mono text-gray-300 capitalize">
              {selectedFee}
              {#if currentFee}
                <span class="text-gray-500 ml-1">
                  ({typeof currentFee.fee === 'number'
                    ? currentFee.fee.toLocaleString(undefined, { maximumFractionDigits: 8 })
                    : currentFee.fee})
                </span>
              {/if}
            </span>
          </div>
          <div class="flex justify-between px-4 py-3">
            <span class="text-sm text-gray-400">Network</span>
            <span class="text-sm font-mono text-gray-300">{networkUnit}</span>
          </div>
        </div>

        <div class="flex flex-col gap-3">
          {#if simResult}
            <div class="bg-gray-800/50 border rounded-lg p-3"
              class:border-vault-500={simResult.success}
              class:border-red-500={!simResult.success}>
              {#if simulating}
                <div class="flex items-center gap-2 text-sm text-gray-400">
                  <span class="animate-spin inline-block w-3 h-3 border-2 border-vault-500 border-t-transparent rounded-full"></span>
                  Simulating...
                </div>
              {:else if simResult.success}
                <div class="flex items-center justify-between text-sm">
                  <span class="text-vault-400">✅ Simulation OK</span>
                  <span class="text-gray-400">~{simResult.gasUsed.toLocaleString()} gas</span>
                </div>
              {:else}
                <div class="text-sm text-red-400">
                  ⚠️ {simResult.revertReason ?? 'Simulation failed'}
                </div>
              {/if}
            </div>
          {/if}
          <div class="flex gap-3">
            <button class="btn-secondary flex-1" onclick={() => (step = 'fee')}>
              Back
            </button>
            <button class="btn-secondary flex-1" disabled={simulating} onclick={handleSimulate}>
              {simulating ? 'Simulating…' : '🔬 Simulate'}
            </button>
            <button class="btn-primary flex-1" disabled={!vault.connected} onclick={handleSignAndBroadcast}>
              Sign &amp; Send
            </button>
          </div>
        </div>
      </div>

    <!-- Step 5: Signing (loading) -->
    {:else if step === 'signing'}
      <div class="text-center py-8">
        <div class="animate-spin inline-block w-8 h-8 border-2 border-vault-500 border-t-transparent rounded-full mb-4"></div>
        <p class="text-gray-300 text-sm">Signing and broadcasting...</p>
        <p class="text-gray-500 text-xs mt-2">This may take a moment</p>
      </div>

    <!-- Step 6: Result -->
    {:else if step === 'result'}
      <div class="text-center py-4">
        {#if resultError}
          <div class="text-4xl mb-3">❌</div>
          <h3 class="text-lg font-semibold text-red-400 mb-2">Transaction Failed</h3>
          <p class="text-sm text-red-300 mb-6">{resultError}</p>
          <div class="flex gap-3">
            <button
              class="btn-secondary flex-1"
              onclick={() => {
                resultError = '';
                step = 'review';
              }}
            >
              Try Again
            </button>
            <button
              class="btn-primary flex-1"
              onclick={resetAndClose}
            >
              Close
            </button>
          </div>
        {:else}
          <div class="text-4xl mb-3">✅</div>
          <h3 class="text-lg font-semibold text-vault-400 mb-2">Transaction Sent</h3>
          <div class="bg-gray-800/50 border border-gray-700 rounded-lg p-3 mb-4">
            <span class="text-xs text-gray-500 block mb-1">Transaction ID</span>
            <span class="font-mono text-xs text-gray-300 break-all">{txid}</span>
          </div>
          {#if txExplorerUrl}
            <a href={txExplorerUrl} target="_blank" rel="noopener noreferrer"
              class="inline-flex items-center gap-1 text-xs text-vault-400 hover:text-vault-300 mb-4 transition-colors">
              🔗 View on explorer
            </a>
          {/if}
          <button
            class="btn-primary w-full"
            onclick={resetAndClose}
          >
            Done
          </button>
        {/if}
      </div>
    {/if}

    <!-- Step indicator -->
    {#if step !== 'signing' && step !== 'result'}
      <div class="flex items-center justify-center gap-1.5 mt-6 pt-4 border-t border-gray-800">
        {#each ['address', 'amount', 'fee', 'review'] as s}
          {@const idx = ['address', 'amount', 'fee', 'review'].indexOf(step)}
          {@const currIdx = ['address', 'amount', 'fee', 'review'].indexOf(s)}
          <div
            class="w-2 h-2 rounded-full transition-colors"
            class:bg-vault-500={currIdx <= idx}
            class:bg-gray-700={currIdx > idx}
          ></div>
        {/each}
      </div>
    {/if}
  </div>
</div>