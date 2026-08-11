/**
 * TransactionHistory component tests.
 *
 * Verifies transaction history list with filter tabs, loading states,
 * empty states, and clipboard copy functionality.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';

const { default: TransactionHistory } = await import('./TransactionHistory.svelte');

const TXS = [
  { txid: '0xaaa111bbb222ccccdddd', from: '0x1234', to: '0x5678', amount: '1.5', unit: 'ETH', direction: 'sent' as const, status: 'confirmed' as const, timestamp: 1723075200 },
  { txid: '0xddd333eee444ffffgggg', from: '0x5678', to: '0x1234', amount: '2.5', unit: 'ETH', direction: 'received' as const, status: 'pending' as const, timestamp: 1723161600 },
  { txid: '0xggg555hhh666iiiijjjj', from: '0x9abc', to: '0xdef0', amount: '0.5', unit: 'ETH', direction: 'sent' as const, status: 'failed' as const, timestamp: 1723248000 },
];

/**
 * Tests the component renders correctly with all required elements
 */
describe('TransactionHistory.svelte', () => {
  afterEach(() => {
    cleanup();
  });

  it('renders the section heading and filter tabs', () => {
    const { container } = render(TransactionHistory, { props: { transactions: TXS, loading: false } });
    // Check for heading
    expect(screen.getByText('📋 Transactions')).toBeTruthy();
    // Check all three filter tabs are rendered
    expect(screen.getByText('All')).toBeTruthy();
    expect(screen.getByText('Sent')).toBeTruthy();
    expect(screen.getByText('Received')).toBeTruthy();
    // Check that the card wrapper exists
    expect(container.querySelector('.card')).toBeTruthy();
  });

  it('renders all three transaction rows', () => {
    render(TransactionHistory, { props: { transactions: TXS, loading: false } });
    // Each row contains a button with the truncated txid
    expect(screen.getByText('0xaaa111...ccdddd')).toBeTruthy();
    expect(screen.getByText('0xddd333...ffgggg')).toBeTruthy();
    expect(screen.getByText('0xggg555...iijjjj')).toBeTruthy();
  });

  it('shows sent amounts with minus and received with plus', () => {
    render(TransactionHistory, { props: { transactions: TXS, loading: false } });
    // Check for the minus sign (Unicode minus) and plus sign in the amounts
    expect(screen.getByText('−1.5')).toBeTruthy(); // minus sign
    expect(screen.getByText('+2.5')).toBeTruthy(); // plus sign
  });

  it('shows status badges for each transaction', () => {
    render(TransactionHistory, { props: { transactions: TXS, loading: false } });
    expect(screen.getByText('Confirmed')).toBeTruthy();
    expect(screen.getByText('Pending')).toBeTruthy();
    expect(screen.getByText('Failed')).toBeTruthy();
  });

  it('shows loading skeletons', () => {
    const { container } = render(TransactionHistory, { props: { transactions: [], loading: true } });
    // When loading, expect animated skeleton placeholder divs
    expect(container.querySelectorAll('.animate-pulse').length).toBeGreaterThan(0);
  });

  it('hides loading skeletons when not loading', () => {
    const { container } = render(TransactionHistory, { props: { transactions: TXS, loading: false } });
    expect(container.querySelectorAll('.animate-pulse').length).toBe(0);
  });

  it('shows empty state message', () => {
    render(TransactionHistory, { props: { transactions: [], loading: false } });
    expect(screen.getByText(/No transactions yet/)).toBeTruthy();
  });

  it('shows "No sent transactions" when filtered', async () => {
    render(TransactionHistory, { props: { transactions: [TXS[1]], loading: false } });
    await fireEvent.click(screen.getByText('Sent'));
    expect(screen.getByText(/No sent transactions/)).toBeTruthy();
  });

  it('shows "No received transactions" when filtered', async () => {
    render(TransactionHistory, { props: { transactions: [TXS[0]], loading: false } });
    await fireEvent.click(screen.getByText('Received'));
    expect(screen.getByText(/No received transactions/)).toBeTruthy();
  });

  it('copies txid to clipboard on click', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText: writeTextMock } });
    render(TransactionHistory, { props: { transactions: TXS, loading: false } });
    await new Promise(r => setTimeout(r, 50));
    await screen.getByText('0xaaa111...ccdddd').click();
    await new Promise(r => setTimeout(r, 50));
    expect(writeTextMock).toHaveBeenCalledWith('0xaaa111bbb222ccccdddd');
    expect(screen.getByText(/Copied!/)).toBeTruthy();
  });

  it('formats timestamps correctly', () => {
    render(TransactionHistory, { props: { transactions: TXS, loading: false } });
    const dateStr = new Date(TXS[0].timestamp * 1000).toLocaleString(undefined, {
      month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit',
    });
    expect(screen.getByText(dateStr)).toBeTruthy();
  });

  it('shows block height', () => {
    const txs = TXS.map((tx, i) => ({ ...tx, blockHeight: 1000 + i }));
    render(TransactionHistory, { props: { transactions: txs, loading: false } });
    expect(screen.getByText(/Block #1[,.]000/)).toBeTruthy();
  });
});