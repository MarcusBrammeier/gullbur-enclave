/**
 * Dashboard component tests.
 *
 * Verifies the two regression-prone behaviors headlessly:
 *  1. Account rows render keyed on unique account.id (the each_key_duplicate fix).
 *  2. "Create Account" calls createAccount(network, nextIndex) with a valid index.
 *
 * The vault store (backed by a reactive $state mock) + IPC functions are mocked,
 * and the heavy child components (Send/Receive/Portfolio) are stubbed.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

// ── Mock the vault store module → reactive $state mock ───────────────
const createAccountMock = vi.fn();

vi.mock('../vault.svelte.ts', () => ({
  vault: mockVault,
  createAccount: (...args: any[]) => createAccountMock(...args),
  refreshBalances: vi.fn().mockResolvedValue(undefined),
  refreshNetworkBalance: vi.fn().mockResolvedValue(undefined),
  setSelectedNetwork: vi.fn(),
  getAccountLabel: () => null,
  setAccountLabel: vi.fn(),
  getNetworkUnit: (_n: string) => 'LTC',
}));

// ── Stub the heavy child components ──────────────────────────────────
vi.mock('./Send.svelte', () => ({
  default: () => '<div data-testid="mock-send" />',
}));
vi.mock('./Receive.svelte', () => ({
  default: () => '<div data-testid="mock-receive" />',
}));
vi.mock('./Portfolio.svelte', () => ({
  default: () => '<div data-testid="mock-portfolio" />',
}));

// Re-import after mocks are registered.
const { default: Dashboard } = await import('./Dashboard.svelte');

describe('Dashboard.svelte', () => {
  beforeEach(() => {
    createAccountMock.mockReset();
    resetMockVault();
  });

  it('shows the empty-state and Create Account button when there are no accounts', () => {
    render(Dashboard);
    expect(screen.getByText(/No accounts yet/i)).toBeInTheDocument();
    // There are two Create Account buttons (empty-state + Quick Actions).
    expect(screen.getAllByRole('button', { name: /Create Account/i })).toHaveLength(2);
  });

  it('renders multiple accounts without duplicate-key error, keyed on unique id', () => {
    mockVault.accounts = [
      { id: 'ltc-litecoin-testnet-0', network: 'litecoin-testnet', address: 'tltc1qaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0', index: 0, balance: { confirmed: '1.2', unconfirmed: '0' } },
      { id: 'ltc-litecoin-testnet-1', network: 'litecoin-testnet', address: 'tltc1qbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb1', index: 1, balance: { confirmed: '2.5', unconfirmed: '0' } },
    ];
    flushSync();

    render(Dashboard);

    // Both accounts render (truncated addresses) — no duplicate-key crash.
    // The address may appear in more than one place (row + other surface), so
    // assert presence rather than uniqueness.
    expect(screen.getAllByText('tltc1q...aaa0').length).toBeGreaterThan(0);
    expect(screen.getAllByText('tltc1q...bbb1').length).toBeGreaterThan(0);
    // Account count indicator reflects both.
    expect(screen.getAllByText('2 accounts').length).toBeGreaterThan(0);
  });

  it('calls createAccount with the selected network and next index', async () => {
    createAccountMock.mockResolvedValue({
      id: 'ltc-litecoin-testnet-1',
      network: 'litecoin-testnet',
      address: 'tltcBBBB',
      index: 1,
      balance: null,
    });

    const { unmount } = render(Dashboard);
    // Seed one account (index 0) → nextIndex should recompute to 1.
    mockVault.accounts = [
      { id: 'ltc-litecoin-testnet-0', network: 'litecoin-testnet', address: 'tltc1qaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0', index: 0, balance: null },
    ];
    flushSync();

    const btns = screen.getAllByRole('button', { name: /Create Account/i });
    await fireEvent.click(btns[0]);

    expect(createAccountMock).toHaveBeenCalledWith('litecoin-testnet', 1);
    unmount();
  });
});
