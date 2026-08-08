/**
 * VaultInit component tests.
 *
 * Verifies the seed entry / generate UI, and that the correct vault functions
 * are invoked with the right arguments during initialization flows.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

const connectMock = vi.fn().mockResolvedValue(undefined);
const initializeMock = vi.fn().mockResolvedValue('test-mnemonic');
const initializeFromStagedMock = vi.fn().mockResolvedValue('test-mnemonic');
const clearStagedMock = vi.fn().mockResolvedValue(undefined);
const generateMnemonicMock = vi.fn().mockResolvedValue('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about');

vi.mock('../vault.svelte.ts', () => ({
  vault: mockVault,
  connect: (...args: any[]) => connectMock(...args),
  initialize: (...args: any[]) => initializeMock(...args),
  initializeFromStaged: (...args: any[]) => initializeFromStagedMock(...args),
  clearStagedMnemonic: (...args: any[]) => clearStagedMock(...args),
  generateMnemonic: (...args: any[]) => generateMnemonicMock(...args),
}));

const { default: VaultInit } = await import('./VaultInit.svelte');

describe('VaultInit.svelte', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    connectMock.mockResolvedValue(undefined);
    initializeMock.mockResolvedValue('test-mnemonic');
    initializeFromStagedMock.mockResolvedValue('test-mnemonic');
    clearStagedMock.mockResolvedValue(undefined);
    generateMnemonicMock.mockResolvedValue('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about');
    resetMockVault();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders the generate-or-restore UI at the input step', () => {
    render(VaultInit);
    expect(screen.getAllByText(/Initialize Vault/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Generate New/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Restore Wallet/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Seed Phrase/i).length).toBeGreaterThan(0);
  });

  it('renders the optional passphrase input', () => {
    render(VaultInit);
    expect(screen.getAllByText(/Passphrase \(optional/i).length).toBeGreaterThan(0);
  });

  it('renders the Open Existing Vault File button', () => {
    render(VaultInit);
    expect(screen.getAllByText(/Open Existing Vault File/i).length).toBeGreaterThan(0);
  });

  it('Restore Wallet is disabled when seed phrase is empty', () => {
    render(VaultInit);
    const restoreBtns = screen.getAllByText(/Restore Wallet/i);
    expect(restoreBtns[0]).toBeDisabled();
  });

  it('calls connect and generateMnemonic when Generate New is clicked', async () => {
    render(VaultInit);
    const generateBtns = screen.getAllByText('Generate New');
    await fireEvent.click(generateBtns[0]);
    expect(connectMock).toHaveBeenCalled();
    expect(generateMnemonicMock).toHaveBeenCalled();
  });

  it('shows backup step after successful mnemonic generation', async () => {
    render(VaultInit);
    const generateBtns = screen.getAllByText('Generate New');
    await fireEvent.click(generateBtns[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Back Up Your Seed Phrase/i).length).toBeGreaterThan(0);
    });
  });

  it('shows error state when generateMnemonic fails', async () => {
    generateMnemonicMock.mockRejectedValue(new Error('IPC connection failed'));
    render(VaultInit);
    const generateBtns = screen.getAllByText('Generate New');
    await fireEvent.click(generateBtns[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText('IPC connection failed').length).toBeGreaterThan(0);
    });
  });

  it('calls connect and initialize with seed phrase when Restore Wallet is clicked', async () => {
    render(VaultInit);
    const textareas = screen.getAllByPlaceholderText(/witch collapse practice/i);
    await fireEvent.input(textareas[0], { target: { value: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about' } });
    const restoreBtns = screen.getAllByText('Restore Wallet');
    expect(restoreBtns[0]).not.toBeDisabled();
    await fireEvent.click(restoreBtns[0]);
    expect(connectMock).toHaveBeenCalled();
    expect(initializeMock).toHaveBeenCalledWith('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about', '');
  });

  it('calls initialize with passphrase when provided', async () => {
    render(VaultInit);
    const textareas = screen.getAllByPlaceholderText(/witch collapse practice/i);
    await fireEvent.input(textareas[0], { target: { value: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about' } });
    const passphraseInputs = screen.getAllByPlaceholderText(/Leave empty for standard seed/i);
    await fireEvent.input(passphraseInputs[0], { target: { value: 'mysecret' } });
    const restoreBtns = screen.getAllByText('Restore Wallet');
    await fireEvent.click(restoreBtns[0]);
    expect(initializeMock).toHaveBeenCalledWith('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about', 'mysecret');
  });

  it('transitions to initializing step when restore succeeds', async () => {
    render(VaultInit);
    const textareas = screen.getAllByPlaceholderText(/witch collapse practice/i);
    await fireEvent.input(textareas[0], { target: { value: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about' } });
    const restoreBtns = screen.getAllByText('Restore Wallet');
    await fireEvent.click(restoreBtns[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Initializing Vault/i).length).toBeGreaterThan(0);
    });
  });

  it('shows error state when initialize fails on restore', async () => {
    initializeMock.mockRejectedValue(new Error('Invalid seed phrase'));
    render(VaultInit);
    const textareas = screen.getAllByPlaceholderText(/witch collapse practice/i);
    await fireEvent.input(textareas[0], { target: { value: 'bad seed phrase' } });
    const restoreBtns = screen.getAllByText('Restore Wallet');
    await fireEvent.click(restoreBtns[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Initialization Failed/i).length).toBeGreaterThan(0);
    });
  });

  it('shows backup skip warning and allows continuing anyway', async () => {
    render(VaultInit);
    // Generate first
    const generateBtns = screen.getAllByText('Generate New');
    await fireEvent.click(generateBtns[0]);
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Back Up Your Seed Phrase/i).length).toBeGreaterThan(0);
    });
    // Click skip verification
    const skipBtns = screen.getAllByText(/Skip Verification/i);
    await fireEvent.click(skipBtns[0]);
    expect(screen.getAllByText(/Skip Seed Backup/i).length).toBeGreaterThan(0);
    // Click Continue Anyway
    const continueBtns = screen.getAllByText('Continue Anyway');
    await fireEvent.click(continueBtns[0]);
    // Should call connect + initialize-from-staged (seed never re-sent)
    expect(connectMock).toHaveBeenCalled();
    expect(initializeFromStagedMock).toHaveBeenCalled();
  });
});