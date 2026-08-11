/**
 * DebugReport component tests.
 *
 * The privacy-safe debug report modal: generates a report via invoke on click,
 * renders redactable account rows, and wires close. invoke + clipboard are mocked.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/svelte';
import { flushSync } from 'svelte';

const { invokeMock, oncloseMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  oncloseMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: any[]) => invokeMock(...a) }));

const REPORT = {
  version: '0.1.0-beta.2',
  os: 'linux',
  arch: 'x86_64',
  build_date: '2026-08-10',
  plugins: [{ id: 'btc', name: 'Bitcoin', networks: ['bitcoin'], capabilities: ['psbt'] }],
  accounts: [
    { network: 'bitcoin', address: 'bc1qtestaddress', path: "m/84'/0'/0'/0/0" },
    { network: 'ethereum', address: '0xethaddress', path: "m/44'/60'/0'/0/0" },
  ],
  env_config: { testnet_only: false, tor_enabled: true, auto_lock_seconds: 30 },
  recent_crashes: [],
};

const { default: DebugReport } = await import('./DebugReport.svelte');

function renderModal() {
  return render(DebugReport, { onclose: oncloseMock });
}

describe('DebugReport.svelte', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    oncloseMock.mockReset();
  });

  afterEach(() => cleanup());

  it('renders the modal and generate button', () => {
    renderModal();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getAllByText(/Generate Debug Report/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/safe for sharing/i).length).toBeGreaterThan(0);
  });

  it('generates and shows the report with redactable accounts', async () => {
    invokeMock.mockResolvedValue(REPORT);
    renderModal();
    const genBtn = screen.getAllByText(/Generate Debug Report/i)[0];
    await fireEvent.click(genBtn);
    flushSync();
    await waitFor(() => {
      expect(screen.getAllByText(/bc1qtestaddress/i).length).toBeGreaterThan(0);
      expect(screen.getAllByText(/0xethaddress/i).length).toBeGreaterThan(0);
    });
    expect(invokeMock).toHaveBeenCalledWith('generate_debug_report');
  });

  it('shows an error when generate fails', async () => {
    invokeMock.mockRejectedValue(new Error('report generation failed'));
    renderModal();
    const genBtn = screen.getAllByText(/Generate Debug Report/i)[0];
    await fireEvent.click(genBtn);
    flushSync();
    await waitFor(() => {
      expect(screen.getAllByText(/report generation failed/i).length).toBeGreaterThan(0);
    });
  });

  it('renders no crash message when there are no crashes', async () => {
    invokeMock.mockResolvedValue(REPORT);
    renderModal();
    await fireEvent.click(screen.getAllByText(/Generate Debug Report/i)[0]);
    await waitFor(() => {
      expect(screen.getAllByText(/No crash reports found/i).length).toBeGreaterThan(0);
    });
  });

  it('calls onclose when the close button is clicked', async () => {
    renderModal();
    const closeBtn = screen.getByRole('button', { name: '\u00d7' });
    await fireEvent.click(closeBtn);
    expect(oncloseMock).toHaveBeenCalled();
  });
});