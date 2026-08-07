/**
 * Reactive mock vault store for component tests.
 *
 * Lives in a .svelte.ts file so the `$state` rune is compiled — a plain .ts
 * mock object is NOT reactive, so Svelte `$derived` values (nextIndex,
 * filteredAccounts) would go stale and tests would read wrong state.
 */
export const mockVault = $state({
  accounts: [] as any[],
  networks: [] as any[],
  selectedNetwork: 'litecoin-testnet',
  connected: true,
  testnetOnly: false,
  error: null as string | null,
  vaultStatus: 'Connected',
  theme: 'dark',
  showBetaWarning: false,
});

export function resetMockVault(): void {
  mockVault.accounts = [];
  mockVault.networks = [];
  mockVault.selectedNetwork = 'litecoin-testnet';
  mockVault.connected = true;
  mockVault.testnetOnly = false;
  mockVault.error = null;
  mockVault.vaultStatus = 'Connected';
  mockVault.theme = 'dark';
  mockVault.showBetaWarning = false;
}
