/**
 * ApprovalModal component tests (H4-A approval gating).
 *
 * Pending approval requests are polled from the Rust vault queue via the
 * Tauri `approval_list` command and displayed in a modal; Approve/Deny call
 * `approval_respond`. The `@tauri-apps/api/core` invoke is mocked, as are the
 * `IS_DEMO` constants. In demo mode the modal renders nothing.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

let demoMode = false;

vi.mock('../constants', () => ({
  get IS_DEMO() { return demoMode; },
}));

vi.mock('../vault.svelte.ts', () => ({ vault: mockVault }));

// Proxy invoke so each test can script approval_list / approval_respond.
let invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: any[]) => (invokeMock as any)(...args),
}));

// Keep timers manual so the poll interval is deterministic.
vi.useFakeTimers();

const { default: ApprovalModal } = await import('./ApprovalModal.svelte');

const PENDING = [
  {
    id: 'approval-1',
    origin: 'https://app.uniswap.org',
    method: 'vault.sign_transaction',
    description: 'Sign the requested transaction with your account',
    summary: { network: 'ethereum', tx_hex: '0x02f8...' },
    created_at: 1700000000,
  },
  {
    id: 'approval-2',
    origin: 'gullbur://desktop',
    method: 'vault.create_account',
    description: 'Create a new account',
    summary: { network: 'bitcoin (index 0)' },
    created_at: 1700000001,
  },
];

describe('ApprovalModal.svelte', () => {
  beforeEach(() => {
    demoMode = false;
    resetMockVault();
    invokeMock = vi.fn().mockResolvedValue([]);
  });

  afterEach(() => {
    cleanup();
    invokeMock = vi.fn().mockResolvedValue([]);
    vi.clearAllTimers();
  });

  it('renders nothing when there are no pending approvals', async () => {
    render(ApprovalModal);
    await vi.advanceTimersByTimeAsync(1100); // let the first poll tick
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(invokeMock).toHaveBeenCalledWith('approval_list');
  });

  it('shows pending approvals and their metadata', async () => {
    invokeMock.mockResolvedValue(PENDING);
    render(ApprovalModal);
    await vi.advanceTimersByTimeAsync(1100);

    expect(screen.getAllByText('Approval Required').length).toBeGreaterThan(0);
    expect(screen.getAllByText(/2 pending operations/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/vault.sign_transaction/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/vault.create_account/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Sign the requested transaction with your account/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/https:\/\/app.uniswap.org/i).length).toBeGreaterThan(0);
    // Two Approve / two Deny buttons.
    expect(screen.getAllByText(/Approve/i).length).toBe(2);
    expect(screen.getAllByText('Deny').length).toBe(2);
  });

  it('approves a request via approval_respond and drops it from the list', async () => {
    invokeMock.mockImplementation((cmd: string, _: any) =>
      cmd === 'approval_list' ? PENDING : (cmd === 'approval_respond' ? undefined : Promise.resolve([]))
    );
    render(ApprovalModal);
    await vi.advanceTimersByTimeAsync(1100);

    const approveButtons = screen.getAllByText(/Approve/i);
    await fireEvent.click(approveButtons[0]);
    // After respond, the modal removes it locally and next poll finds only one.
    invokeMock.mockResolvedValue([PENDING[1]]);
    await vi.advanceTimersByTimeAsync(1100);

    expect(invokeMock).toHaveBeenCalledWith('approval_respond', { id: 'approval-1', approved: true });
    expect(screen.queryByText(/vault.sign_transaction/i)).toBeNull();
    expect(screen.getAllByText(/vault.create_account/i).length).toBeGreaterThan(0);
  });

  it('denies a request via approval_respond', async () => {
    invokeMock.mockImplementation((cmd: string, _: any) =>
      cmd === 'approval_list' ? PENDING : (cmd === 'approval_respond' ? undefined : Promise.resolve([]))
    );
    render(ApprovalModal);
    await vi.advanceTimersByTimeAsync(1100);

    const denyButtons = screen.getAllByText('Deny');
    await fireEvent.click(denyButtons[0]);
    invokeMock.mockResolvedValue([PENDING[1]]);
    await vi.advanceTimersByTimeAsync(1100);

    expect(invokeMock).toHaveBeenCalledWith('approval_respond', { id: 'approval-1', approved: false });
    expect(screen.queryByText(/vault.sign_transaction/i)).toBeNull();
  });

  it('renders nothing in demo mode (IS_DEMO true), no IPC calls', async () => {
    demoMode = true;
    render(ApprovalModal);
    await vi.advanceTimersByTimeAsync(3000);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});