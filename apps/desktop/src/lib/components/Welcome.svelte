<script lang="ts">
  import { vault } from '../vault.svelte.ts';

  type Step = 'splash' | 'beta' | 'configure' | 'complete';

  let step = $state<Step>(
    typeof localStorage !== 'undefined' && localStorage.getItem('foss_wallet_beta_accepted') === 'true'
      ? 'complete'
      : 'splash'
  );

  let testnetOnly = $state(true);
  let crashConsent = $state(true);
  let enableBiometric = $state(true);
  let autoLockSecs = $state(30);
  let betaAccepted = $state(false);

  /** Detail panels toggled open per config option */
  let detailOpen = $state<Record<string, boolean>>({});

  function toggleDetail(key: string) {
    detailOpen = { ...detailOpen, [key]: !detailOpen[key] };
  }

  function dismissAndContinue() {
    vault.testnetOnly = testnetOnly;
    localStorage.setItem('foss_wallet_crash_consent', String(crashConsent));
    localStorage.setItem('foss_wallet_biometric', String(enableBiometric));
    localStorage.setItem('foss_wallet_autolock', String(autoLockSecs));
    if (betaAccepted) {
      localStorage.setItem('foss_wallet_beta_accepted', 'true');
    }
    step = 'complete';
  }

  function skipToWallet() {
    step = 'complete';
  }

  function handleBetaAccept() {
    betaAccepted = true;
    step = 'configure';
  }
</script>

{#if step === 'splash'}
  <!-- ═══════════════════════════ SPLASH ═══════════════════════════ -->
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black">
    <div class="max-w-lg w-full mx-4 text-center">
      <!-- Icon animation -->
      <div class="text-7xl mb-6 animate-pulse">🔐</div>
      <h1 class="text-4xl font-bold tracking-tight text-white mb-3">
        Gullbúr Enclave Core
      </h1>
      <p class="text-lg text-gray-400 mb-2">Self-Custody. Private. Multi-Chain.</p>
      <p class="text-sm text-gray-600 mb-10 max-w-md mx-auto">
        Bitcoin · Ethereum · Monero — one vault, your keys, your control.
      </p>
      <button
        class="bg-vault-600 hover:bg-vault-500 text-white font-semibold py-3 px-10 rounded-xl text-lg transition-all hover:scale-105 active:scale-95"
        onclick={() => step = 'beta'}
      >
        Get Started
      </button>
      <p class="text-xs text-gray-600 mt-4">
        <a href="https://github.com/sponsors/YOUR_USERNAME" target="_blank" rel="noopener"
           class="text-vault-400 hover:text-vault-300 transition-colors">❤️ Donate</a>
        — support open-source development
      </p>
      <p class="text-xs text-gray-700 mt-2">v0.0.1-internal-beta</p>
    </div>
  </div>

{:else if step === 'beta'}
  <!-- ═══════════════════════════ BETA WARNING ═══════════════════════════ -->
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/95 backdrop-blur-sm">
    <div class="bg-gray-900 border border-amber-600/30 rounded-2xl shadow-2xl max-w-lg w-full mx-4 p-8">
      <div class="text-center mb-6">
        <div class="text-6xl mb-4">🧪</div>
        <h1 class="text-2xl font-bold text-amber-400 mb-3">Beta Software — Test Use Only</h1>
      </div>

      <div class="space-y-4 text-sm text-gray-300 mb-6">
        <div class="bg-amber-900/20 border border-amber-700/30 rounded-xl p-4">
          <p class="font-semibold text-amber-300 mb-2">⚠️ What This Means</p>
          <p>
            This is <strong class="text-amber-300">pre-release beta software</strong> under active development.
            It is intended for <strong class="text-amber-300">testing, evaluation, and educational purposes only</strong>.
          </p>
        </div>

        <div class="bg-gray-800/50 rounded-xl p-4 space-y-2 text-gray-400">
          <p>🔸 Bugs, breaking changes, and data loss are <strong>possible</strong></p>
          <p>🔸 APIs, file formats, and wallet schemas may change without notice</p>
          <p>🔸 Only use testnet funds (BTC testnet, ETH Sepolia, XMR stagenet) <strong>unless you fully understand and accept the risks</strong></p>
          <p>🔸 The developers assume <strong>no liability</strong> for any loss of funds, key mismanagement, or errors</p>
          <p>🔸 Your seed phrase is your <strong>sole responsibility</strong> — there is no recovery mechanism</p>
        </div>

        <p class="text-xs text-gray-600 leading-relaxed">
          By continuing, you acknowledge that this software is provided "as is", without warranty of any kind,
          express or implied. The authors and contributors shall not be held liable for any claim, damages,
          or other liability arising from its use.
        </p>
      </div>

      <div class="flex flex-col gap-3">
        <label class="flex items-center gap-2 text-xs text-gray-500 cursor-pointer">
          <input type="checkbox" bind:checked={betaAccepted} class="accent-amber-500" />
          Don't show this warning again
        </label>
        <button
          class="w-full py-3 px-6 rounded-xl font-semibold text-sm bg-amber-600 hover:bg-amber-500 text-black transition-colors"
          onclick={handleBetaAccept}
        >
          I Understand — Continue
        </button>
        <button
          class="w-full py-2 px-6 rounded-lg text-sm text-gray-500 hover:text-gray-300 transition-colors"
          onclick={() => step = 'splash'}
        >
          ← Go Back
        </button>
      </div>
    </div>
  </div>

{:else if step === 'configure'}
  <!-- ═══════════════════════════ CONFIGURATION ═══════════════════════════ -->
  <div class="fixed inset-0 z-50 flex items-start justify-center bg-black/95 backdrop-blur-sm overflow-y-auto py-8">
    <div class="bg-gray-900 border border-gray-800 rounded-2xl shadow-2xl max-w-lg w-full mx-4 p-8">
      <div class="text-center mb-6">
        <div class="text-4xl mb-3">🛠️</div>
        <h1 class="text-xl font-bold text-gray-100 mb-1">Configure Your Vault</h1>
        <p class="text-sm text-gray-500">These settings can be changed later in Settings.</p>
      </div>

      <div class="space-y-3 mb-6">
        <!-- ── Testnet toggle ── -->
        <div class="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
          <button class="w-full flex items-center justify-between p-4 text-left hover:bg-gray-800/80 transition-colors" onclick={() => toggleDetail('testnet')}>
            <div>
              <p class="text-sm font-medium text-gray-200">🧪 Testnet-Only Mode</p>
              <p class="text-xs text-gray-500 mt-0.5">Restrict operations to test networks</p>
            </div>
            <span class="text-gray-500 text-lg">{detailOpen['testnet'] ? '▾' : '▸'}</span>
          </button>
          {#if detailOpen['testnet']}
            <div class="px-4 pb-4 text-xs text-gray-400 space-y-2">
              <p><strong class="text-gray-300">On (recommended):</strong> Wallet only connects to test networks — BTC testnet, ETH Sepolia, XMR stagenet. All funds are test tokens with no real-world value.</p>
              <p><strong class="text-gray-300">Off:</strong> Wallet connects to mainnet. You can send and receive real assets. Only disable if you understand the risks of beta software with real funds.</p>
              <p class="text-gray-500">You can switch at any time from Settings.</p>
              <div class="mt-3 space-y-1">
                <p class="text-xs text-vault-400 font-medium">🧪 Need test tokens?</p>
                <a href="https://sepoliafaucet.com" target="_blank" rel="noopener" class="block text-xs text-vault-300 hover:text-vault-200">Sepolia ETH — sepoliafaucet.com</a>
                <a href="https://coinfaucet.eu/en/btc-testnet/" target="_blank" rel="noopener" class="block text-xs text-vault-300 hover:text-vault-200">Bitcoin Testnet BTC — coinfaucet.eu</a>
                <a href="https://communitymonero.org/guides/testnet/" target="_blank" rel="noopener" class="block text-xs text-vault-300 hover:text-vault-200">Monero Stagenet XMR — communitymonero.org</a>
              </div>
            </div>
          {/if}
          <div class="px-4 pb-4">
            <button
              class="relative w-11 h-6 rounded-full transition-colors {testnetOnly ? 'bg-amber-600' : 'bg-gray-700'}"
              onclick={() => testnetOnly = !testnetOnly}
              role="switch"
              aria-checked={testnetOnly}
            >
              <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {testnetOnly ? 'translate-x-5' : ''}"></span>
            </button>
            <span class="ml-3 text-sm text-gray-400">{testnetOnly ? 'Testnet (safe)' : 'Mainnet (real funds)'}</span>
          </div>
        </div>

        <!-- ── Crash consent ── -->
        <div class="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
          <button class="w-full flex items-center justify-between p-4 text-left hover:bg-gray-800/80 transition-colors" onclick={() => toggleDetail('crash')}>
            <div>
              <p class="text-sm font-medium text-gray-200">📋 Crash Reporting</p>
              <p class="text-xs text-gray-500 mt-0.5">Help improve stability (privacy-safe)</p>
            </div>
            <span class="text-gray-500 text-lg">{detailOpen['crash'] ? '▾' : '▸'}</span>
          </button>
          {#if detailOpen['crash']}
            <div class="px-4 pb-4 text-xs text-gray-400 space-y-2">
              <p>When the app crashes unexpectedly, a <strong>diagnostic report</strong> is saved to disk containing:</p>
              <ul class="list-disc list-inside space-y-1 text-gray-500">
                <li>App version number</li>
                <li>Where in the code the crash happened (file name and line number)</li>
                <li>The error message from the crash</li>
              </ul>
              <p class="text-vault-400 font-medium">🔒 What is NEVER included:</p>
              <ul class="list-disc list-inside space-y-1 text-gray-500">
                <li>Your seed phrase, private keys, or passwords</li>
                <li>Your wallet addresses or transaction data</li>
                <li>Any personal information or system files</li>
                <li>Network requests or account balances</li>
              </ul>
              <p>These reports stay on your machine and are never sent anywhere automatically.</p>
            </div>
          {/if}
          <div class="px-4 pb-4">
            <button
              class="relative w-11 h-6 rounded-full transition-colors {crashConsent ? 'bg-vault-600' : 'bg-gray-700'}"
              onclick={() => crashConsent = !crashConsent}
              role="switch"
              aria-checked={crashConsent}
            >
              <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {crashConsent ? 'translate-x-5' : ''}"></span>
            </button>
            <span class="ml-3 text-sm text-gray-400">{crashConsent ? 'Enabled' : 'Disabled'}</span>
          </div>
        </div>

        <!-- ── Biometric ── -->
        <div class="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
          <button class="w-full flex items-center justify-between p-4 text-left hover:bg-gray-800/80 transition-colors" onclick={() => toggleDetail('biometric')}>
            <div>
              <p class="text-sm font-medium text-gray-200">🔐 Biometric Unlock</p>
              <p class="text-xs text-gray-500 mt-0.5">Touch ID / Windows Hello / PAM</p>
            </div>
            <span class="text-gray-500 text-lg">{detailOpen['biometric'] ? '▾' : '▸'}</span>
          </button>
          {#if detailOpen['biometric']}
            <div class="px-4 pb-4 text-xs text-gray-400 space-y-2">
              <p>Biometric unlock lets you quickly unlock your vault using your device's built-in fingerprint or face scanner instead of typing a password each time.</p>
              <p><strong class="text-gray-300">On:</strong> After initial wallet setup, you'll be prompted to enroll a biometric for quick access.</p>
              <p><strong class="text-gray-300">Off:</strong> You'll use a software prompt to confirm actions. You can enable biometrics later in Settings.</p>
            </div>
          {/if}
          <div class="px-4 pb-4">
            <button
              class="relative w-11 h-6 rounded-full transition-colors {enableBiometric ? 'bg-vault-600' : 'bg-gray-700'}"
              onclick={() => enableBiometric = !enableBiometric}
              role="switch"
              aria-checked={enableBiometric}
            >
              <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {enableBiometric ? 'translate-x-5' : ''}"></span>
            </button>
            <span class="ml-3 text-sm text-gray-400">{enableBiometric ? 'Enabled' : 'Disabled'}</span>
          </div>
        </div>

        <!-- ── Auto-lock ── -->
        <div class="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
          <button class="w-full flex items-center justify-between p-4 text-left hover:bg-gray-800/80 transition-colors" onclick={() => toggleDetail('autolock')}>
            <div>
              <p class="text-sm font-medium text-gray-200">⏱️ Auto-Lock Timer</p>
              <p class="text-xs text-gray-500 mt-0.5">Lock vault after inactivity</p>
            </div>
            <span class="text-gray-500 text-lg">{detailOpen['autolock'] ? '▾' : '▸'}</span>
          </button>
          {#if detailOpen['autolock']}
            <div class="px-4 pb-4 text-xs text-gray-400 space-y-2">
              <p>The vault automatically locks after a period of inactivity. You'll need to re-authenticate (biometric or software prompt) to sign transactions.</p>
              <p>Default is 30 seconds. Longer times are more convenient but leave your vault unlocked longer. Set to 0 to disable auto-lock entirely (not recommended).</p>
            </div>
          {/if}
          <div class="px-4 pb-4">
            <div class="flex items-center gap-3">
              <input type="range" min="0" max="300" step="5" bind:value={autoLockSecs} class="flex-1 accent-vault-500" />
              <span class="text-sm font-mono text-gray-400 w-20 text-right">
                {autoLockSecs === 0 ? 'Off' : `${autoLockSecs}s`}
              </span>
            </div>
          </div>
        </div>
      </div>

      <div class="flex flex-col gap-3">
        <button
          class="w-full py-3 px-6 rounded-xl font-semibold text-sm bg-vault-600 hover:bg-vault-500 text-white transition-all hover:scale-[1.02] active:scale-95"
          onclick={dismissAndContinue}
        >
          Save &amp; Continue → Create Wallet
        </button>
        <button
          class="w-full py-2 px-6 rounded-lg text-sm text-gray-500 hover:text-gray-300 transition-colors"
          onclick={skipToWallet}
        >
          Skip Setup — Use Defaults
        </button>
      </div>
    </div>
  </div>

{:else if step === 'complete'}
  <!-- ═══════════════════════════ NOP — Welcome done ═══════════════════════════ -->
{/if}