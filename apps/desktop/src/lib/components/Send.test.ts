/**
 * Send component tests.
 *
 * Verifies the multi-step send wizard: address validation, amount validation,
 * fee estimation, simulation, and sign & broadcast flows.
 *
 * Follows the established pattern: reactive $state mock via mockVault,
 * heavy children stubbed, flushSync before DOM queries after state mutations.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

// ── Mock vault functions ────────────────────────────────────────────────────

const validateAddressMock = vi.fn();
const estimateFeeMock = vi.fn().mockResolvedValue([]);
const signTransactionMock = vi.fn();
const broadcastTransactionMock = vi.fn();
const simulateTransferMock = vi.fn();

vi.mock('../vault.svelte.ts', () => ({
  vault: mockVault,
  validateAddress: (...args: any[]) => validateAddressMock(...args),
  estimateFee: (...args: any[]) => estimateFeeMock(...args),
  signTransaction: (...args: any[]) => signTransactionMock(...args),
  broadcastTransaction: (...args: any[]) => broadcastTransactionMock(...args),
  simulateTransfer: (...args: any[]) => simulateTransferMock(...args),
  getAccountLabel: () => null,
}));

const oncloseMock = vi.fn();
const { default: Send } = await import('./Send.svelte');

// ── Helpers ─────────────────────────────────────────────────────────────────

const TEST_ACCOUNT = {
  id: 'ltc-litecoin-testnet-0',
  network: 'litecoin-testnet',
  address: 'tltc1qaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0',
  index: 0,
  balance: { confirmed: '10.00000000', unconfirmed: '0' },
};

function renderSend() {
  return render(Send, { account: TEST_ACCOUNT, onclose: oncloseMock });
}

/**
 * Navigate from step 1 (address) to step 2 (amount).
 * Sets the recipient, validates (tick), and clicks Continue.
 */
async function goToAmountStep(recipient = 'tltc1qvalidadr') {
  validateAddressMock.mockResolvedValue(true);
  const recipientInputs = screen.getAllByPlaceholderText(/Enter LITECOIN-TESTNET address/i);
  await fireEvent.input(recipientInputs[0], { target: { value: recipient } });
  await fireEvent.blur(recipientInputs[0]);
  const continues = screen.getAllByText('Continue');
  const firstEnabled = continues.find((btn) => !(btn as HTMLButtonElement).disabled);
  if (firstEnabled) {
    await fireEvent.click(firstEnabled);
  }
}

/**
 * Navigate from step 2 (amount) to step 3 (fee).
 */
async function goToFeeStep(recipient = 'tltc1qvalidadr', amountVal = '1.0') {
  await goToAmountStep(recipient);
  const amountInputs = screen.getAllByPlaceholderText('0.00');
  await fireEvent.input(amountInputs[0], { target: { value: amountVal } });
  const continues = screen.getAllByText('Continue');
  const firstEnabled = continues.find((btn) => !(btn as HTMLButtonElement).disabled);
  if (firstEnabled) {
    await fireEvent.click(firstEnabled);
  }
}

/**
 * Navigate from step 3 (fee) to step 4 (review).
 */
async function goToReviewStep(recipient = 'tltc1qvalidadr', amountVal = '1.0') {
  estimateFeeMock.mockResolvedValue([
    { level: 'slow', fee: 0.0001, estimatedTime: '~1 hour' },
    { level: 'medium', fee: 0.0005, estimatedTime: '~30 min' },
    { level: 'fast', fee: 0.001, estimatedTime: '~10 min' },
  ]);
  await goToFeeStep(recipient, amountVal);
  await vi.waitFor(() => {
    const review = screen.getAllByText('Review');
    if (review.length > 0) expect(review[0]).toBeInTheDocument();
  });
  const review = screen.getAllByText('Review');
  await fireEvent.click(review[0]);
}

describe('Send.svelte', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetMockVault();
    oncloseMock.mockReset();
    validateAddressMock.mockReset();
    estimateFeeMock.mockReset().mockResolvedValue([]);
    signTransactionMock.mockReset();
    broadcastTransactionMock.mockReset();
    simulateTransferMock.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  // ── Basic rendering ─────────────────────────────────────────────────

  it('renders the send modal with the address step first', () => {
    renderSend();
    expect(screen.getAllByRole('dialog', { name: /Send transaction/i }).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Recipient Address/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Continue/i).length).toBeGreaterThan(0);
  });

  it('shows the from account info', () => {
    renderSend();
    expect(screen.getAllByText(/tltc1qaa\.\.\.aaaaa0/).length).toBeGreaterThan(0);
  });

  it('shows a close button', () => {
    renderSend();
    const closeBtns = screen.getAllByRole('button', { name: /Close/i });
    expect(closeBtns.length).toBeGreaterThan(0);
  });

  // ── Amount validation ───────────────────────────────────────────────

  it('shows error for negative amount in handleAmountInput', async () => {
    renderSend();
    await goToAmountStep();
    const amountInputs = screen.getAllByPlaceholderText('0.00');
    await fireEvent.input(amountInputs[0], { target: { value: '-5' } });
    expect(screen.getAllByText(/Enter a valid positive amount/i).length).toBeGreaterThan(0);
  });

  it('shows error for zero amount in handleAmountInput', async () => {
    renderSend();
    await goToAmountStep();
    const amountInputs = screen.getAllByPlaceholderText('0.00');
    await fireEvent.input(amountInputs[0], { target: { value: '0' } });
    expect(screen.getAllByText(/Enter a valid positive amount/i).length).toBeGreaterThan(0);
  });

  it('shows insufficient balance when amount exceeds balance', async () => {
    renderSend();
    await goToAmountStep();
    const amountInputs = screen.getAllByPlaceholderText('0.00');
    await fireEvent.input(amountInputs[0], { target: { value: '100' } });
    expect(screen.getAllByText(/Insufficient balance/i).length).toBeGreaterThan(0);
  });

  it('shows no amount error for a valid amount within balance', async () => {
    renderSend();
    await goToAmountStep();
    const amountInputs = screen.getAllByPlaceholderText('0.00');
    await fireEvent.input(amountInputs[0], { target: { value: '5.0' } });
    expect(screen.queryByText(/Enter a valid positive amount/i)).toBeNull();
    expect(screen.queryByText(/Insufficient balance/i)).toBeNull();
  });

  // ── Address validation ──────────────────────────────────────────────

  it('shows valid address indicator when address validation succeeds', async () => {
    validateAddressMock.mockResolvedValue(true);
    renderSend();
    const recipientInputs = screen.getAllByPlaceholderText(/Enter LITECOIN-TESTNET address/i);
    await fireEvent.input(recipientInputs[0], { target: { value: 'tltc1qvalidaddress' } });
    await fireEvent.blur(recipientInputs[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/✓ Valid address/i).length).toBeGreaterThan(0);
    });
  });

  it('shows invalid address error when validation fails', async () => {
    validateAddressMock.mockResolvedValue(false);
    renderSend();
    const recipientInputs = screen.getAllByPlaceholderText(/Enter LITECOIN-TESTNET address/i);
    await fireEvent.input(recipientInputs[0], { target: { value: 'badaddress' } });
    await fireEvent.blur(recipientInputs[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Invalid address for this network/i).length).toBeGreaterThan(0);
    });
  });

  it('shows validation error message when validateAddress throws', async () => {
    validateAddressMock.mockRejectedValue(new Error('Network error'));
    renderSend();
    const recipientInputs = screen.getAllByPlaceholderText(/Enter LITECOIN-TESTNET address/i);
    await fireEvent.input(recipientInputs[0], { target: { value: 'tltc1qtest123' } });
    await fireEvent.blur(recipientInputs[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText('Network error').length).toBeGreaterThan(0);
    });
  });

  // ── Navigation flow ─────────────────────────────────────────────────

  it('can proceed from address step to amount step with valid address', async () => {
    validateAddressMock.mockResolvedValue(true);
    renderSend();
    const recipientInputs = screen.getAllByPlaceholderText(/Enter LITECOIN-TESTNET address/i);
    await fireEvent.input(recipientInputs[0], { target: { value: 'tltc1qvalidaddress' } });
    await fireEvent.blur(recipientInputs[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/✓ Valid address/i).length).toBeGreaterThan(0);
    });
    const continues = screen.getAllByText('Continue');
    const firstEnabled = continues.find((btn) => !(btn as HTMLButtonElement).disabled);
    expect(firstEnabled).toBeDefined();
    await fireEvent.click(firstEnabled!);
    expect(screen.getAllByText(/Amount \(LITECOIN-TESTNET\)/i).length).toBeGreaterThan(0);
  });

  it('cannot proceed from address step with invalid address', async () => {
    validateAddressMock.mockResolvedValue(false);
    renderSend();
    const recipientInputs = screen.getAllByPlaceholderText(/Enter LITECOIN-TESTNET address/i);
    await fireEvent.input(recipientInputs[0], { target: { value: 'bad' } });
    await fireEvent.blur(recipientInputs[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Invalid address for this network/i).length).toBeGreaterThan(0);
    });
    const continues = screen.getAllByText('Continue');
    expect(continues.every((btn) => (btn as HTMLButtonElement).disabled)).toBe(true);
  });

  it('can go back from amount step to address step', async () => {
    renderSend();
    await goToAmountStep();
    expect(screen.getAllByText(/Amount \(LITECOIN-TESTNET\)/i).length).toBeGreaterThan(0);
    const backs = screen.getAllByText('Back');
    await fireEvent.click(backs[0]);
    expect(screen.getAllByText(/Recipient Address/i).length).toBeGreaterThan(0);
  });

  // ── Fee step flow ───────────────────────────────────────────────────

  it('can proceed from amount step to fee step', async () => {
    estimateFeeMock.mockResolvedValue([
      { level: 'slow', fee: 0.0001, estimatedTime: '~1 hour' },
      { level: 'medium', fee: 0.0005, estimatedTime: '~30 min' },
      { level: 'fast', fee: 0.001, estimatedTime: '~10 min' },
    ]);
    renderSend();
    await goToAmountStep();
    const amountInputs = screen.getAllByPlaceholderText('0.00');
    await fireEvent.input(amountInputs[0], { target: { value: '1.0' } });
    const continues = screen.getAllByText('Continue');
    const firstEnabled = continues.find((btn) => !(btn as HTMLButtonElement).disabled);
    await fireEvent.click(firstEnabled!);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Transaction Fee/i).length).toBeGreaterThan(0);
    });
    expect(estimateFeeMock).toHaveBeenCalledWith('litecoin-testnet', 'tltc1qvalidadr', '1.0');
    expect(screen.getAllByText('slow').length).toBeGreaterThan(0);
    expect(screen.getAllByText('medium').length).toBeGreaterThan(0);
    expect(screen.getAllByText('fast').length).toBeGreaterThan(0);
  });

  it('shows no fee estimates message when estimateFee returns empty', async () => {
    validateAddressMock.mockResolvedValue(true);
    estimateFeeMock.mockResolvedValue([]);
    renderSend();
    await goToFeeStep();
    await vi.waitFor(() => {
      expect(screen.getAllByText(/No fee estimates available/i).length).toBeGreaterThan(0);
    });
  });

  // ── Review step ─────────────────────────────────────────────────────

  it('can reach review step from fee step', async () => {
    renderSend();
    await goToReviewStep();
    expect(screen.getAllByText(/Review Transaction/i).length).toBeGreaterThan(0);
  });

  // ── Simulate ────────────────────────────────────────────────────────

  it('calls handleSimulate when Simulate button is clicked on review step', async () => {
    simulateTransferMock.mockResolvedValue({ success: true, gasUsed: 21000, revertReason: null });
    renderSend();
    await goToReviewStep();
    const simBtns = screen.getAllByText('🔬 Simulate');
    await fireEvent.click(simBtns[0]);
    expect(simulateTransferMock).toHaveBeenCalledWith(
      'litecoin-testnet',
      TEST_ACCOUNT.address,
      'tltc1qvalidadr',
      '1.0',
    );
    // The simulate call is async — wait for the result to render
    await vi.waitFor(() => {
      expect(screen.getAllByText(/✅ Simulation OK/i).length).toBeGreaterThan(0);
    });
  });

  // ── Sign and Broadcast (with mock connected) ────────────────────────

  it('calls signTransaction and broadcastTransaction when Sign & Send is clicked', async () => {
    signTransactionMock.mockResolvedValue({ signed_tx: '0xsigned' });
    broadcastTransactionMock.mockResolvedValue({ txid: '0xtxid123' });
    renderSend();
    await goToReviewStep();
    const signBtns = screen.getAllByText(/Sign & Send/i);
    await fireEvent.click(signBtns[0]);
    expect(signTransactionMock).toHaveBeenCalledWith({
      from: TEST_ACCOUNT.address,
      to: 'tltc1qvalidadr',
      amount: '1.0',
      network: 'litecoin-testnet',
      feeLevel: 'medium',
    });
    expect(broadcastTransactionMock).toHaveBeenCalledWith({ signed_tx: '0xsigned' });
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Transaction Sent/i).length).toBeGreaterThan(0);
    });
  });

  // ── Result state ────────────────────────────────────────────────────

  it('shows transaction failed when signTransaction throws', async () => {
    signTransactionMock.mockRejectedValue(new Error('Insufficient funds'));
    renderSend();
    await goToReviewStep();
    const signBtns = screen.getAllByText(/Sign & Send/i);
    await fireEvent.click(signBtns[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Transaction Failed/i).length).toBeGreaterThan(0);
      expect(screen.getAllByText(/Insufficient funds/i).length).toBeGreaterThan(0);
    });
  });

  it('shows success with txid on successful send', async () => {
    signTransactionMock.mockResolvedValue({ signed_tx: '0xsigned' });
    broadcastTransactionMock.mockResolvedValue({ txid: '0xabc123def456' });
    renderSend();
    await goToReviewStep();
    const signBtns = screen.getAllByText(/Sign & Send/i);
    await fireEvent.click(signBtns[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Transaction Sent/i).length).toBeGreaterThan(0);
      expect(screen.getAllByText('0xabc123def456').length).toBeGreaterThan(0);
    });
  });

  // ── Closing behavior ────────────────────────────────────────────────

  it('calls onclose when Close button is clicked', async () => {
    renderSend();
    const closeBtns = screen.getAllByRole('button', { name: /Close/i });
    await fireEvent.click(closeBtns[0]);
    expect(oncloseMock).toHaveBeenCalled();
  });
});