/**
 * Settings component tests.
 *
 * Verifies the settings modal renders, the testnet-only toggle works, and the
 * mainnet-access warning flow. Heavy deps (themeEngine, toasts, DebugReport,
 * ConsoleLog, @tauri-apps/api) are stubbed; the vault mock drives testnet state.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

vi.mock('../vault.svelte.ts', () => ({ vault: mockVault }));

vi.mock('../themeEngine.svelte.ts', () => ({
  themeEngine: {
    getAvailableThemes: () => ['dark-slate', 'light-slate'],
    currentThemeId: 'dark-slate',
  },
}));
vi.mock('../toasts.svelte.ts', () => ({
  pushError: vi.fn(),
  pushInfo: vi.fn(),
  pushWarning: vi.fn(),
}));
vi.mock('./DebugReport.svelte', () => ({ default: () => '<div data-testid="mock-debug" />' }));
vi.mock('./ConsoleLog.svelte', () => ({ default: () => '<div data-testid="mock-console-log" />' }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const oncloseMock = vi.fn();
const { default: Settings } = await import('./Settings.svelte');

function renderSettings() {
  return render(Settings, { onclose: oncloseMock });
}

describe('Settings.svelte', () => {
  beforeEach(() => {
    resetMockVault();
    mockVault.connected = true;
    mockVault.initialized = true;
    mockVault.testnetOnly = false;
    oncloseMock.mockReset();
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders the settings dialog with version', () => {
    renderSettings();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getAllByText(/Settings/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/v0\.1\.0-beta/i).length).toBeGreaterThan(0);
  });

  it('close button invokes onclose', () => {
    renderSettings();
    const closeBtn = screen.getByRole('button', { name: '\u00d7' });
    fireEvent.click(closeBtn);
    expect(oncloseMock).toHaveBeenCalled();
  });

  it('enabling testnet-only mode sets vault.testnetOnly', () => {
    renderSettings();
    const testnetSwitch = screen.getByRole('switch', { name: /testnet-only mode/i });
    fireEvent.click(testnetSwitch);
    flushSync();
    expect(mockVault.testnetOnly).toBe(true);
  });

  it('disabling testnet-only shows the mainnet-access beta warning', () => {
    mockVault.testnetOnly = true;
    renderSettings();
    const testnetSwitch = screen.getByRole('switch', { name: /testnet-only mode/i });
    fireEvent.click(testnetSwitch); // turning OFF → warning modal
    flushSync();
    expect(screen.getAllByText(/Mainnet Access/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/beta software/i).length).toBeGreaterThan(0);
  });

  it('continuing past the mainnet warning actually disables testnet-only', () => {
    mockVault.testnetOnly = true;
    renderSettings();
    const testnetSwitch = screen.getByRole('switch', { name: /testnet-only mode/i });
    fireEvent.click(testnetSwitch);
    flushSync();
    // Confirma the warning and continue → vault.testnetOnly becomes false.
    const continueBtn = screen.getAllByText(/I Understand — Continue/i)[0];
    fireEvent.click(continueBtn);
    flushSync();
    expect(mockVault.testnetOnly).toBe(false);
    expect(screen.queryByText(/Mainnet Access/i)).toBeNull();
  });

  it('shows the seed confirm gate before revealing (not leaking seed)', () => {
    renderSettings();
    // Seed Recovery area: "Show Seed Phrase" requires explicit confirm first.
    const showSeedBtn = screen.getAllByText(/Show Seed Phrase/i)[0];
    expect(showSeedBtn).toBeTruthy();
    fireEvent.click(showSeedBtn);
    flushSync();
    expect(screen.getAllByText(/full access to your wallet/i).length).toBeGreaterThan(0);
  });
});