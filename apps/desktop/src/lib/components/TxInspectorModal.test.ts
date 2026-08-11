/**
 * TxInspectorModal component tests.
 *
 * Zero-trust transaction inspector: tabs (summary/technical/raw), the
 * unlimited-allowance warning, fee display, and confirm/cancel wiring. It takes
 * plain props (no vault/IPC), so no mocking is needed beyond a test account.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';

const { default: TxInspectorModal } = await import('./TxInspectorModal.svelte');

const ACCOUNT = {
  id: 'btc-bitcoin-0',
  network: 'bitcoin',
  address: 'bc1qaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
  index: 0,
  balance: { confirmed: '1.0', unconfirmed: '0' },
  path: "m/84'/0'/0'/0/0",
};
const FEE = { level: 'medium' as const, fee: 0.0005, estimatedTime: '~30 min' };

function renderModal(overrides: Record<string, unknown> = {}) {
  return render(TxInspectorModal, {
    account: ACCOUNT,
    recipient: 'bc1qrecipientaddress',
    amount: '0.5',
    fee: FEE,
    networkUnit: 'BTC',
    onconfirm: vi.fn(),
    oncancel: vi.fn(),
    ...overrides,
  });
}

describe('TxInspectorModal.svelte', () => {
  afterEach(() => cleanup());

  it('renders the summary tab by default with asset movements', () => {
    renderModal();
    expect(screen.getAllByText(/Zero-Trust Transaction Inspector/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Simulated Asset Movements/i).length).toBeGreaterThan(0);
    // The sender shows a minus delta, recipient a plus.
    expect(screen.getAllByText(/-0\.5 BTC/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/\+0\.5 BTC/i).length).toBeGreaterThan(0);
  });

  it('shows the estimated network fee from the fee prop', () => {
    renderModal();
    expect(screen.getAllByText(/Estimated Network Fee/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/0\.0005 BTC \(medium\)/i).length).toBeGreaterThan(0);
  });

  it('shows the unlimited-allowance security warning for huge amounts', () => {
    renderModal({ amount: 'unlimited' });
    expect(screen.getAllByText(/Unlimited Token Allowance/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/unlimited control over your tokens/i).length).toBeGreaterThan(0);
  });

  it('does not show the unlimited warning for a normal amount', () => {
    renderModal({ amount: '0.5' });
    expect(screen.queryByText(/Unlimited Token Allowance/i)).toBeNull();
  });

  it('switches to the raw payload tab and shows the JSON body', async () => {
    renderModal();
    const rawTab = screen.getAllByText(/Raw Payload/i)[0];
    await fireEvent.click(rawTab);
    expect(screen.getAllByText(/bc1qrecipientaddress/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/"network": "bitcoin"/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/"amount": "0\.5"/i).length).toBeGreaterThan(0);
  });

  it('calls onconfirm when Sign & Broadcast is clicked', async () => {
    const onconfirm = vi.fn();
    renderModal({ onconfirm });
    const signBtn = screen.getAllByText(/Sign & Broadcast/i)[0];
    await fireEvent.click(signBtn);
    expect(onconfirm).toHaveBeenCalled();
  });

  it('calls oncancel when Cancel is clicked', async () => {
    const oncancel = vi.fn();
    renderModal({ oncancel });
    const cancelBtn = screen.getAllByText('Cancel')[0];
    await fireEvent.click(cancelBtn);
    expect(oncancel).toHaveBeenCalled();
  });

  it('renders the CLSAG ring privacy tab for monero accounts', async () => {
    renderModal({
      account: { ...ACCOUNT, id: 'xmr-monero-0', network: 'monero', address: '4XMRaddr' },
      networkUnit: 'XMR',
    });
    // The technical tab button is dynamically labelled for monero.
    const techTab = screen.getAllByText(/CLSAG Ring Privacy/i)[0];
    await fireEvent.click(techTab);
    // The body shows the RingCT privacy verification.
    expect(screen.getAllByText(/RingCT Privacy Verified/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/11 \/ 11 Ring Members/i).length).toBeGreaterThan(0);
  });
});