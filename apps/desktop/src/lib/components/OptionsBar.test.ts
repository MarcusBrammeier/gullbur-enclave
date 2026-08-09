/**
 * OptionsBar component tests.
 *
 * Verifies theme selection and the testnet-only toggle.
 * The vault store is mocked with the reactive $state mock from mockVault.svelte.ts.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

const setThemeMock = vi.fn();

vi.mock('../vault.svelte.ts', () => ({
  vault: mockVault,
  setTheme: (...args: any[]) => setThemeMock(...args),
}));

const { default: OptionsBar } = await import('./OptionsBar.svelte');

describe('OptionsBar.svelte', () => {
  beforeEach(() => {
    setThemeMock.mockReset();
    resetMockVault();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders theme options for obsidian, dark-slate, and light-studio', () => {
    render(OptionsBar);
    expect(screen.getAllByText('🪨').length).toBeGreaterThan(0);
    expect(screen.getAllByText('🌙').length).toBeGreaterThan(0);
    expect(screen.getAllByText('☀️').length).toBeGreaterThan(0);
  });

  it('renders accent color dot buttons', () => {
    render(OptionsBar);
    // There should be 5 accent dots
    const accentBtns = screen.getAllByTitle(/Accent:/i);
    expect(accentBtns.length).toBe(5);
  });

  it('renders the testnet-only toggle switch', () => {
    render(OptionsBar);
    const switches = screen.getAllByRole('switch');
    expect(switches.length).toBeGreaterThan(0);
  });

  it('shows testnet-only badge when testnetOnly is true', () => {
    mockVault.testnetOnly = true;
    flushSync();
    render(OptionsBar);
    expect(screen.getAllByText(/Testnet Only/i).length).toBeGreaterThan(0);
  });

  it('does not show testnet-only badge when testnetOnly is false', () => {
    mockVault.testnetOnly = false;
    flushSync();
    render(OptionsBar);
    expect(screen.queryByText(/Testnet Only/i)).toBeNull();
  });

  it('shows beta warning modal when turning off testnet-only', async () => {
    mockVault.testnetOnly = true;
    flushSync();
    render(OptionsBar);
    const switches = screen.getAllByRole('switch');
    await fireEvent.click(switches[0]);
    expect(mockVault.showBetaWarning).toBe(true);
    expect(screen.getAllByText(/Mainnet is in Beta/i).length).toBeGreaterThan(0);
  });

  it('confirming mainnet sets testnetOnly to false and hides warning', async () => {
    mockVault.testnetOnly = true;
    mockVault.showBetaWarning = true;
    flushSync();
    render(OptionsBar);
    const continueBtns = screen.getAllByText('Continue');
    await fireEvent.click(continueBtns[0]);
    expect(mockVault.testnetOnly).toBe(false);
    expect(mockVault.showBetaWarning).toBe(false);
  });

  it('cancelling mainnet hides warning without changing testnetOnly', async () => {
    mockVault.testnetOnly = true;
    mockVault.showBetaWarning = true;
    flushSync();
    render(OptionsBar);
    const cancelBtns = screen.getAllByText('Cancel');
    await fireEvent.click(cancelBtns[0]);
    expect(mockVault.testnetOnly).toBe(true);
    expect(mockVault.showBetaWarning).toBe(false);
  });
});