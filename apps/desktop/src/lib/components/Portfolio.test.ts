/**
 * Portfolio component tests.
 *
 * Verifies accounts are grouped by network, empty-state renders,
 * and balance error state shows warning instead of silent zero.
 *
 * getNetworkSpec and getNetworkUnit in vault.svelte.ts read vault.networks
 * internally, so we populate vault.networks rather than mocking those fns.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

const refreshBalancesMock = vi.fn().mockResolvedValue(undefined);
const getTransactionHistoryMock = vi.fn().mockResolvedValue([]);

vi.mock('../vault.svelte.ts', () => ({
  vault: mockVault,
  refreshBalances: (...args: any[]) => refreshBalancesMock(...args),
  getTransactionHistory: (...args: any[]) => getTransactionHistoryMock(...args),
  // These are re-exported from vault.svelte.ts — they check vault.networks internally
  // so we don't need to mock them as long as vault.networks is populated.
  getNetworkSpec: (id: string) => (mockVault as any).networks.find((n: any) => n.id === id),
  getNetworkUnit: (id: string) => {
    const net = (mockVault as any).networks.find((n: any) => n.id === id);
    return net?.unit ?? net?.symbol ?? id.toUpperCase();
  },
}));

vi.mock('./TransactionHistory.svelte', () => ({
  default: () => '<div data-testid="mock-tx-history" />',
}));

const { default: Portfolio } = await import('./Portfolio.svelte');

const TEST_NETWORKS = [
  { id: 'litecoin-testnet', name: 'Litecoin Testnet', symbol: 'LTC', unit: 'LTC', decimals: 8, active: true, is_testnet: true },
];

describe('Portfolio.svelte', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    refreshBalancesMock.mockResolvedValue(undefined);
    getTransactionHistoryMock.mockResolvedValue([]);
    resetMockVault();
    mockVault.networks = JSON.parse(JSON.stringify(TEST_NETWORKS));
  });

  afterEach(() => {
    cleanup();
  });

  it('shows the empty-state messages when there are no accounts', () => {
    render(Portfolio);
    expect(screen.getAllByText(/No accounts yet/i).length).toBeGreaterThan(0);
  });

  it('renders the Portfolio heading and 0-account count', () => {
    render(Portfolio);
    expect(screen.getAllByText(/📊 Portfolio/).length).toBeGreaterThan(0);
    expect(screen.getAllByText('0 accounts').length).toBeGreaterThan(0);
  });

  it('renders accounts grouped by network card', () => {
    mockVault.accounts = [
      { id: 'ltc-0', network: 'litecoin-testnet', address: 'tltc1qaaa', index: 0, balance: { confirmed: '1.2345', unconfirmed: '0' } },
      { id: 'ltc-1', network: 'litecoin-testnet', address: 'tltc1qbbb', index: 1, balance: { confirmed: '2.5', unconfirmed: '0' } },
    ];
    flushSync();
    render(Portfolio);
    expect(screen.getAllByText(/Litecoin Testnet/).length).toBeGreaterThan(0);
    // Total balance displayed: 1.2345 + 2.5 = 3.7345
    expect(screen.getAllByText(/3\.7345/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/2 accounts/).length).toBeGreaterThan(0);
  });

  it('renders per-account rows in the All Accounts section', () => {
    mockVault.accounts = [
      { id: 'ltc-0', network: 'litecoin-testnet', address: 'tltc1qaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0', index: 0, balance: { confirmed: '1.2', unconfirmed: '0' } },
    ];
    flushSync();
    render(Portfolio);
    expect(screen.getAllByText(/🔑 All Accounts/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/🔄 Refresh/).length).toBeGreaterThan(0);
  });

  it('shows balance error warning instead of silent zero', () => {
    mockVault.accounts = [
      { id: 'ltc-0', network: 'litecoin-testnet', address: 'tltc1qaaa', index: 0, balance: null, balanceError: 'RPC timeout' },
    ];
    flushSync();
    render(Portfolio);
    expect(screen.getAllByText('⚠').length).toBeGreaterThan(0);
    expect(screen.getAllByText('balance error').length).toBeGreaterThan(0);
  });

  it('triggers refresh when Refresh button is clicked', async () => {
    mockVault.accounts = [
      { id: 'ltc-0', network: 'litecoin-testnet', address: 'tltc1qaaa', index: 0, balance: { confirmed: '1.0', unconfirmed: '0' } },
    ];
    flushSync();
    render(Portfolio);
    const refreshBtns = screen.getAllByText('🔄 Refresh');
    await fireEvent.click(refreshBtns[0]);
    expect(refreshBalancesMock).toHaveBeenCalled();
  });

  it('selecting an account shows transaction history', async () => {
    getTransactionHistoryMock.mockResolvedValue([
      { txid: '0xabc', from: 'tltc1qaaa', to: 'tltc1qbbb', amount: '0.5', unit: 'LTC', direction: 'sent', status: 'confirmed' },
    ]);
    mockVault.accounts = [
      { id: 'ltc-0', network: 'litecoin-testnet', address: 'tltc1qaaa', index: 0, balance: { confirmed: '1.0', unconfirmed: '0' } },
    ];
    flushSync();
    render(Portfolio);
    const accountRows = screen.getAllByText(/tltc1qaaa/);
    await fireEvent.click(accountRows[0]);
    expect(screen.getAllByText(/📋 Transaction History/).length).toBeGreaterThan(0);
    // The stubbed TransactionHistory renders (as text content from the string mock)
    // and the close button shows
    expect(screen.getAllByText(/✕ Close/).length).toBeGreaterThan(0);
  });

  it('deselecting an account hides transaction history', async () => {
    mockVault.accounts = [
      { id: 'ltc-0', network: 'litecoin-testnet', address: 'tltc1qaaa', index: 0, balance: { confirmed: '1.0', unconfirmed: '0' } },
    ];
    flushSync();
    render(Portfolio);
    const accountRows = screen.getAllByText(/tltc1qaaa/);
    await fireEvent.click(accountRows[0]);
    expect(screen.getAllByText(/📋 Transaction History/).length).toBeGreaterThan(0);
    await fireEvent.click(accountRows[0]);
    expect(screen.queryByText(/📋 Transaction History/)).toBeNull();
  });
});