/**
 * Receive component tests.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

const getAccountLabelMock = vi.fn().mockReturnValue(null);

vi.mock('../vault.svelte.ts', () => ({
  vault: mockVault,
  getAccountLabel: (...args: any[]) => getAccountLabelMock(...args),
}));

const oncloseMock = vi.fn();
const { default: Receive } = await import('./Receive.svelte');

const ACCOUNT1 = { id: 'ethereum-0', network: 'ethereum', address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', index: 0, balance: null };

const TEST_NETWORKS = [
  { id: 'ethereum', name: 'Ethereum', symbol: 'ETH', decimals: 18, is_testnet: false, active: true, unit: 'ETH' },
  { id: 'bitcoin', name: 'Bitcoin', symbol: 'BTC', decimals: 8, is_testnet: false, active: true, unit: 'BTC' },
];

describe('Receive.svelte', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetMockVault();
    oncloseMock.mockReset();
    getAccountLabelMock.mockReset();
    getAccountLabelMock.mockReturnValue(null);
    mockVault.selectedNetwork = 'ethereum';
    mockVault.networks = JSON.parse(JSON.stringify(TEST_NETWORKS));
    mockVault.accounts = JSON.parse(JSON.stringify([ACCOUNT1]));
  });

  afterEach(() => {
    cleanup();
  });

  it('renders the receive modal', () => {
    render(Receive, { onclose: oncloseMock });
    expect(screen.getByText('Receive')).toBeTruthy();
  });

  it('has a network selector', () => {
    render(Receive, { onclose: oncloseMock });
    expect(document.getElementById('receive-network-select')).toBeTruthy();
  });

  it('shows Loading QR text on initial render', () => {
    render(Receive, { onclose: oncloseMock });
    expect(screen.getByText(/Loading QR/)).toBeTruthy();
  });

  it('displays the first account address', () => {
    render(Receive, { onclose: oncloseMock });
    expect(screen.getByText(ACCOUNT1.address)).toBeTruthy();
  });

  it('has a Copy Address button', () => {
    render(Receive, { onclose: oncloseMock });
    expect(screen.getByText(/Copy Address/)).toBeTruthy();
  });

  it('copies address to clipboard', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText: writeTextMock } });
    render(Receive, { onclose: oncloseMock });
    await fireEvent.click(screen.getByText(/Copy Address/));
    expect(writeTextMock).toHaveBeenCalledWith(ACCOUNT1.address);
    expect(screen.getByText(/Copied!/)).toBeTruthy();
  });

  it('calls onclose on Escape key', async () => {
    render(Receive, { onclose: oncloseMock });
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(oncloseMock).toHaveBeenCalled();
  });

  it('calls onclose on backdrop click', async () => {
    render(Receive, { onclose: oncloseMock });
    const backdrop = document.querySelector('[role="dialog"]')!;
    await fireEvent.click(backdrop);
    expect(oncloseMock).toHaveBeenCalled();
  });

  it('shows create account prompt when no accounts', () => {
    mockVault.accounts = [];
    render(Receive, { onclose: oncloseMock });
    expect(screen.getByText(/Create an account/)).toBeTruthy();
  });

  it('handles empty selectedNetwork', () => {
    mockVault.selectedNetwork = '';
    render(Receive, { onclose: oncloseMock });
    expect(document.getElementById('receive-network-select')).toBeTruthy();
  });

  it('calls onclose on close button click', async () => {
    render(Receive, { onclose: oncloseMock });
    const closeBtn = screen.getByText('\u00D7');
    await fireEvent.click(closeBtn);
    expect(oncloseMock).toHaveBeenCalled();
  });
});