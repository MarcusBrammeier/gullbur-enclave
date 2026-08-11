<script lang="ts">
  import { vault, connect, initialize, initializeFromStaged, clearStagedMnemonic, generateMnemonic } from '../vault.svelte.ts';
  import { iconHtml } from '../icons';

  type Step = 'input' | 'backup' | 'confirm' | 'skip_warn' | 'initializing' | 'error';

  let step = $state<Step>('input');
  let seedPhrase = $state('');
  let localError = $state<string | null>(null);
  let generating = $state(false);
  let wordArray = $state<string[]>([]);
  let shuffledWords = $state<string[]>([]);
  let selectedIndexes = $state<number[]>([]);
  let passphrase = $state('');
  let understood = $state(false);

  async function handleGenerate() {
    generating = true;
    localError = null;
    try {
      await connect();
      const mnemonic = await generateMnemonic();
      wordArray = mnemonic.split(' ');
      shuffledWords = [...wordArray].sort();
      selectedIndexes = [];
      step = 'backup';
    } catch (e) {
      localError = e instanceof Error ? e.message : String(e);
    } finally {
      generating = false;
    }
  }

  async function handleRestore() {
    const phrase = seedPhrase.trim();
    if (!phrase) { localError = 'Enter a seed phrase or generate a new one.'; return; }
    localError = null;
    step = 'initializing';
    const timeout = setTimeout(() => {
      step = 'error';
      localError = 'Initialization timed out — is the IPC server running?';
    }, 15_000);
    try {
      await connect();
      await initialize(phrase, passphrase.trim());
      clearTimeout(timeout);
    } catch (e) {
      clearTimeout(timeout);
      step = 'error';
      localError = e instanceof Error ? e.message : String(e);
    }
  }

  function selectWord(_word: string, idx: number) {
    if (selectedIndexes.includes(idx)) {
      selectedIndexes = selectedIndexes.filter((i) => i !== idx);
    } else {
      selectedIndexes = [...selectedIndexes, idx];
    }
  }

  function proceedToConfirm() {
    // Pick verification words: first, last, and 4 random
    const indices = new Set<number>();
    indices.add(0);
    indices.add(wordArray.length - 1);
    while (indices.size < Math.min(6, wordArray.length)) {
      indices.add(Math.floor(Math.random() * wordArray.length));
    }
    selectedIndexes = [];
    step = 'confirm';
  }

  function handleConfirmComplete() {
    if (selectedIndexes.length < wordArray.length) return;
    step = 'initializing';
    const timeout = setTimeout(() => {
      step = 'error';
      localError = 'Initialization timed out — is the IPC server running?';
    }, 15_000);
    connect().then(() => {
      // Initialize from the Rust-staged phrase — the seed is never re-sent
      // from the UI after it was generated.
      initializeFromStaged(passphrase.trim()).then(() => {
        clearTimeout(timeout);
        // App.svelte handles transition to Dashboard via vault.initialized
      }).catch((e) => {
        clearTimeout(timeout);
        step = 'error';
        localError = e instanceof Error ? e.message : String(e);
      });
    }).catch((e) => {
      clearTimeout(timeout);
      step = 'error';
      localError = e instanceof Error ? e.message : String(e);
    });
  }

  function handleSkipWarn() {
    step = 'skip_warn';
  }

  function handleSkipToInit() {
    step = 'initializing';
    const timeout = setTimeout(() => {
      step = 'error';
      localError = 'Initialization timed out — is the IPC server running?';
    }, 15_000);
    connect().then(() => {
      initializeFromStaged(passphrase.trim()).then(() => {
        clearTimeout(timeout);
        // App.svelte handles transition to Dashboard via vault.initialized
      }).catch((e) => {
        clearTimeout(timeout);
        step = 'error';
        localError = e instanceof Error ? e.message : String(e);
      });
    }).catch((e) => {
      clearTimeout(timeout);
      step = 'error';
      localError = e instanceof Error ? e.message : String(e);
    });
  }

  function goBackToInput() {
    clearStagedMnemonic();
    step = 'input';
    seedPhrase = '';
    localError = null;
    wordArray = [];
    shuffledWords = [];
    selectedIndexes = [];
    passphrase = '';
  }

  function resetState() {
    clearStagedMnemonic();
    step = 'input';
    seedPhrase = '';
    localError = null;
    generating = false;
    wordArray = [];
    shuffledWords = [];
    selectedIndexes = [];
    passphrase = '';
  }

  async function openVaultFile() {
    localError = null;
    try {
      // Use Tauri dialog to pick a keystore file
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        title: 'Open Existing Keystore',
        filters: [{ name: 'Keystore', extensions: ['bin', 'dat', '*'] }],
        multiple: false,
      });
      if (!selected) return;
      const { invoke } = await import('@tauri-apps/api/core');
      // On Android, the dialog returns a content:// URI — read it via fetch
      // so we can pass the raw bytes to the Rust backend.
      try {
        const response = await fetch(selected);
        const blob = await response.blob();
        const buffer = await blob.arrayBuffer();
        const data = new Uint8Array(buffer);
        await invoke('open_vault_from_bytes', { data: Array.from(data) });
      } catch {
        // Fallback: try loading by path (desktop) — works on Linux/macOS/Windows
        await invoke('open_vault_from_path', { path: selected });
      }
      await connect();
      // App.svelte reacts to vault.initialized -> shows Dashboard
    } catch (e) {
      localError = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<div class="vault-init-container max-w-lg mx-auto">
  {#if step === 'input'}
    <!-- Step 1: Seed phrase input / Generate -->
    <div class="bg-vault-900/30 border border-default rounded-xl p-6">
      <h2 class="text-lg font-semibold mb-2">Initialize Vault</h2>
      <p class="text-sm text-secondary mb-4">
        Enter your existing seed phrase to restore, or generate a new wallet.
      </p>

      <label class="block text-sm font-medium text-primary mb-1" for="seed-phrase-input">
        Seed Phrase (12 or 24 words)
      </label>
      <textarea
        id="seed-phrase-input"
        bind:value={seedPhrase}
        rows="3"
        placeholder="witch collapse practice feed shame open despair creek road again willow least"
        class="w-full bg-surface border border-strong rounded-lg px-4 py-3 text-primary placeholder-gray-500 focus:outline-none focus:border-vault-500 focus:ring-1 focus:ring-vault-500 font-mono text-sm resize-none"
        disabled={generating}
      ></textarea>

      <div class="mt-3">
        <label class="block text-sm font-medium text-secondary mb-1" for="passphrase-input">
          Passphrase (optional — BIP-39 25th word)
        </label>
        <input
          id="passphrase-input"
          type="text"
          bind:value={passphrase}
          placeholder="Leave empty for standard seed"
          class="w-full bg-surface border border-strong rounded-lg px-4 py-2.5 text-primary placeholder-gray-500 focus:outline-none focus:border-vault-500 focus:ring-1 focus:ring-vault-500 text-sm"
          disabled={generating}
        />
        <p class="text-xs text-muted mt-1">A passphrase creates a completely different wallet from the same seed words</p>
      </div>

      <div class="flex items-center gap-3 mt-4">
        <button class="btn-primary text-sm flex-1" onclick={handleGenerate} disabled={generating}>
          {#if generating}
            <span class="animate-spin inline-block w-4 h-4 border-2 border-strong border-t-transparent rounded-full"></span>
            Generating...
          {:else}
            <span class="text-vault-400">{@html iconHtml('sparkles', 'w-4 h-4 inline-block')}</span>
            Generate New
          {/if}
        </button>
        <button class="btn-secondary text-sm flex-1" onclick={handleRestore} disabled={generating || !seedPhrase.trim()}>
          Restore Wallet
        </button>
      </div>

      <div class="mt-3 pt-3 border-t border-default">
        <button class="btn-secondary text-sm w-full flex items-center justify-center gap-2" onclick={openVaultFile}>
          {@html iconHtml('folder', 'w-4 h-4')} Open Existing Vault File…
        </button>
        <p class="text-xs text-muted mt-1 text-center">Open a previously saved keystore file from anywhere on disk</p>
      </div>

      {#if localError}
        <p class="mt-3 text-sm text-red-400">{localError}</p>
      {/if}
    </div>

  {:else if step === 'backup'}
    <!-- Step 2: Show generated phrase + confirm backup -->
    <div class="bg-vault-900/30 border border-amber-600/30 rounded-xl p-6 pb-safe">
      <div class="flex items-center gap-2 mb-3">
        <span class="text-2xl">{@html iconHtml('lock', 'w-7 h-7')}</span>
        <h2 class="text-lg font-semibold">Back Up Your Seed Phrase</h2>
      </div>
      <p class="text-sm text-amber-300 mb-4">
        Write down these words in order. Never share them with anyone. This is the only way to recover your wallet.
      </p>

      <div class="grid grid-cols-3 gap-2 mb-4">
        {#each wordArray as word, i}
          <div class="bg-surface rounded-lg px-3 py-2 text-sm font-mono text-primary flex items-center gap-2">
            <span class="text-xs text-muted w-5 text-right">{i + 1}.</span>
            <span class="break-all">{word}</span>
          </div>
        {/each}
      </div>

      <div class="space-y-3">
        <label class="flex items-center gap-3 cursor-pointer">
          <input type="checkbox" bind:checked={understood} class="mt-0.5" />
          <span class="text-sm text-primary">I have written down my seed phrase and stored it securely</span>
        </label>
        <button class="w-full py-3 px-6 rounded-xl font-semibold text-sm bg-amber-600 hover:bg-amber-500 text-black transition-colors disabled:opacity-40" disabled={!understood} onclick={proceedToConfirm}>
          Verify My Backup
        </button>
        <button class="w-full py-2.5 px-6 rounded-xl font-semibold text-sm bg-surface-hover hover:bg-surface text-primary transition-colors" onclick={handleSkipWarn}>
          Skip Verification (throwaway wallets only)
        </button>
      </div>

      <button class="text-xs text-muted mt-4 underline" onclick={goBackToInput}>Go back</button>
    </div>

  {:else if step === 'confirm'}
    <!-- Step 3: Verify by selecting words in order -->
    <div class="bg-vault-900/30 border border-vault-700/30 rounded-xl p-6 pb-safe">
      <h2 class="text-lg font-semibold mb-2">Verify Your Backup</h2>
      <p class="text-sm text-secondary mb-4">
        Select the words in the correct order to confirm you've backed up your seed phrase.
      </p>

      <!-- Progress: selected words so far -->
      <div class="flex flex-wrap gap-2 mb-4 min-h-10">
        {#each selectedIndexes as idx, i}
          <span class="bg-vault-800 text-vault-300 text-xs font-mono px-2 py-1 rounded">
            {i + 1}. {shuffledWords[idx]}
          </span>
        {/each}
      </div>

      <!-- Shuffled word buttons -->
      <div class="grid grid-cols-3 gap-2 mb-4">
        {#each shuffledWords, idx}
          <button
            class="px-3 py-2 rounded-lg text-sm font-mono transition-colors {selectedIndexes.includes(idx) ? 'bg-surface-hover text-muted line-through' : 'bg-surface text-primary hover:bg-vault-800 hover:text-vault-300'}"
            onclick={() => selectWord(shuffledWords[idx], idx)}
            disabled={selectedIndexes.includes(idx)}
          >
            {shuffledWords[idx]}
          </button>
        {/each}
      </div>

      <div class="flex gap-3">
        <button class="flex-1 py-3 px-6 rounded-xl font-semibold text-sm bg-surface-hover hover:bg-surface text-primary transition-colors" onclick={() => { selectedIndexes = []; step = 'backup'; }}>
          ← Back
        </button>
        <button class="flex-1 py-3 px-6 rounded-xl font-semibold text-sm bg-vault-600 hover:bg-vault-500 text-white transition-colors disabled:opacity-40" disabled={selectedIndexes.length < wordArray.length || !selectedIndexes.every((_, i) => shuffledWords[selectedIndexes[i]] === wordArray[i])} onclick={handleConfirmComplete}>
          {selectedIndexes.length < wordArray.length ? 'Select All Words' : 'Confirm & Initialize'}
        </button>
      </div>

      {#if localError}
        <p class="mt-3 text-sm text-red-400">{localError}</p>
      {/if}
    </div>

  {:else if step === 'skip_warn'}
    <!-- Skip verification warning -->
    <div class="bg-vault-900/30 border border-red-800 rounded-xl p-6 text-center">
      <div class="text-4xl mb-3">{@html iconHtml('alertTriangle', 'w-10 h-10 text-red-400')}</div>
      <h2 class="text-lg font-semibold text-red-400 mb-2">Skip Seed Backup?</h2>
      <p class="text-sm text-secondary mb-4">
        You won't be able to recover this wallet if you lose access.<br>
        Only do this for throwaway wallets and testing.
      </p>
      <div class="flex gap-3 justify-center">
        <button class="btn-secondary text-sm" onclick={() => { step = 'backup'; }}>
          Go Back
        </button>
        <button class="py-3 px-6 rounded-xl font-semibold text-sm bg-red-700 hover:bg-red-600 text-white transition-colors" onclick={handleSkipToInit}>
          Continue Anyway
        </button>
      </div>
    </div>

  {:else if step === 'initializing'}
    <div class="bg-vault-900/30 border border-default rounded-xl p-8 text-center">
      <div class="animate-spin inline-block w-10 h-10 border-3 border-vault-500 border-t-transparent rounded-full mb-4"></div>
      <h2 class="text-lg font-semibold mb-1">Initializing Vault</h2>
      <p class="text-sm text-secondary">{vault.vaultStatus}</p>
    </div>

  {:else if step === 'error'}
    <div class="bg-vault-900/30 border border-red-800 rounded-xl p-6 text-center">
      <div class="text-4xl mb-3">{@html iconHtml('alertCircle', 'w-10 h-10 text-red-400')}</div>
      <h2 class="text-lg font-semibold text-red-400 mb-1">Initialization Failed</h2>
      <p class="text-sm text-secondary mb-4">{localError || vault.error || 'An unknown error occurred.'}</p>
      <button class="btn-secondary text-sm" onclick={resetState}>Try Again</button>
    </div>
  {/if}
</div>