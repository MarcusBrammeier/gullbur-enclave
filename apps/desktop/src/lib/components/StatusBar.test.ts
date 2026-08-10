/**
 * StatusBar component tests.
 *
 * Verifies the footer status bar renders vault connection state, version,
 * charger name, and network count. Heavy children (TorToggle, UpdateBanner,
 * DebugReport, ConsoleLog) are stubbed; the vault store is the reactive
 * $state mockVault.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

vi.mock('../vault.svelte.ts', () => ({
  vault: mockVault,
  networkCount: () => mockVault.networks.length,
}));

vi.mock('./TorToggle.svelte', () => ({ default: () => '<div data-testid="mock-tor-toggle" />' }));
vi.mock('./UpdateBanner.svelte', () => ({ default: () => '<div data-testid="mock-update-banner" />' }));
vi.mock('./ConsoleLog.svelte', () => ({ default: () => '<div data-testid="mock-console" />' }));

// DebugReport is mounted at the top of an {#if} block with an onclose prop.
// A plain string-returning stub isn't a valid Svelte component there, so use
// createRawSnippet (the Svelte 5 way to emit inert, un-parameterized markup).
vi.mock('./DebugReport.svelte', () => ({
  default: createRawSnippet(() => ({ render: () => '<div data-testid="mock-debug-report" />' })),
}));

const { default: StatusBar } = await import('./StatusBar.svelte');

describe('StatusBar.svelte', () => {
  beforeEach(() => {
    resetMockVault();
    mockVault.vaultStatus = 'Connected';
    mockVault.connected = true;
    mockVault.networks = [
      { id: 'bitcoin', name: 'Bitcoin' },
      { id: 'ethereum', name: 'Ethereum' },
    ];
  });

  afterEach(() => cleanup());

  it('renders the version string', () => {
    render(StatusBar);
    expect(screen.getAllByText(/Gullbúr Enclave Core v0\.1\.0/i).length).toBeGreaterThan(0);
  });

  it('shows the vault status', () => {
    render(StatusBar);
    expect(screen.getAllByText('Connected').length).toBeGreaterThan(0);
  });

  it('shows the network count from the store', () => {
    render(StatusBar);
    expect(screen.getAllByText('Networks: 2').length).toBeGreaterThan(0);
  });

  it('reflects disconnected state in the status label', () => {
    mockVault.connected = false;
    mockVault.vaultStatus = 'Disconnected';
    render(StatusBar);
    expect(screen.getAllByText('Disconnected').length).toBeGreaterThan(0);
  });

  it('opens and closes the console modal', async () => {
    render(StatusBar);
    const consoleBtn = screen.getByRole('button', { name: /Console/i });
    await fireEvent.click(consoleBtn);
    expect(screen.getAllByText(/IPC Console/i).length).toBeGreaterThan(0);
    // Close via the × button
    const closeBtn = screen.getAllByRole('button', { name: '\u00d7' });
    await fireEvent.click(closeBtn[0]);
    expect(screen.queryByText(/IPC Console/i)).toBeNull();
  });

  it('opens the debug report modal', async () => {
    render(StatusBar);
    const debugBtn = screen.getByRole('button', { name: /Debug/i });
    await fireEvent.click(debugBtn);
    expect(screen.getByTestId('mock-debug-report')).toBeInTheDocument();
  });
});