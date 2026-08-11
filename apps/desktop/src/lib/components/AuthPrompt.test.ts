/**
 * AuthPrompt component tests.
 *
 * The overlay is gated on vault.authStatus === 'hardware_required'. Covers:
 * visibility, confirm-success path, confirm-error cooldown, and the hardware
 * lockout screen. The Tauri invoke is mocked; the reactive $state mockVault is
 * the source of truth.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

vi.mock('../vault.svelte.ts', () => ({ vault: mockVault }));

// Mock the dynamic @tauri-apps/api/core import used inside handlers.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: any[]) => (globalThis as any).__mockInvoke?.(...args),
}));

const { default: AuthPrompt } = await import('./AuthPrompt.svelte');

function setHardwareRequired() {
  mockVault.authStatus = 'hardware_required';
  mockVault.authTimeout = 30;
}

describe('AuthPrompt.svelte', () => {
  beforeEach(() => {
    resetMockVault();
    mockVault.authTimeout = 30;
    (globalThis as any).__mockInvoke = undefined;
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('renders nothing when auth is not hardware_required', () => {
    mockVault.authStatus = 'biometric_unlocked';
    render(AuthPrompt);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('renders the hardware prompt when auth is hardware_required', () => {
    setHardwareRequired();
    render(AuthPrompt);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toBeInTheDocument();
    expect(screen.getAllByText(/Hardware Authentication Required/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Confirm/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Cancel/i).length).toBeGreaterThan(0);
  });

  it('confirm success unlocks to biometric_unlocked', async () => {
    setHardwareRequired();
    (globalThis as any).__mockInvoke = vi.fn().mockResolvedValue(undefined);
    render(AuthPrompt);
    const confirmBtn = screen.getAllByText('Confirm')[0];
    await fireEvent.click(confirmBtn);
    flushSync();
    await vi.waitFor(() => {
      expect(mockVault.authStatus).toBe('biometric_unlocked');
    });
    // Overlay hides once unlocked.
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('confirm failure shows an error and enters cooldown (retry disabled)', async () => {
    setHardwareRequired();
    (globalThis as any).__mockInvoke = vi.fn().mockRejectedValue(new Error('Device busy'));
    render(AuthPrompt);
    const confirmBtn = screen.getAllByText('Confirm')[0];
    await fireEvent.click(confirmBtn);
    flushSync();
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Device busy/i).length).toBeGreaterThan(0);
    });
    // Cooldown: Confirm button shows "Retry in 2s…" and is disabled.
    expect(screen.getAllByText(/Retry in 2s/i).length).toBeGreaterThan(0);
    const disabledBtn = screen.getByText(/Retry in 2s/i).closest('button');
    expect((disabledBtn as HTMLButtonElement).disabled).toBe(true);
  });

  it('shows the Security Lockout screen on a lockout message', async () => {
    setHardwareRequired();
    (globalThis as any).__mockInvoke = vi
      .fn()
      .mockRejectedValue(new Error('Hardware lockout: too many attempts'));
    render(AuthPrompt);
    const confirmBtn = screen.getAllByText('Confirm')[0];
    await fireEvent.click(confirmBtn);
    flushSync();
    await vi.waitFor(() => {
      expect(screen.getAllByText(/Security Lockout/i).length).toBeGreaterThan(0);
    });
    expect(screen.getAllByText(/manual intervention/i).length).toBeGreaterThan(0);
  });

  it('cancel button locks the vault to unauthenticated', async () => {
    setHardwareRequired();
    (globalThis as any).__mockInvoke = vi.fn().mockResolvedValue(undefined);
    render(AuthPrompt);
    const cancelBtn = screen.getAllByText('Cancel')[0];
    await fireEvent.click(cancelBtn);
    flushSync();
    await vi.waitFor(() => {
      expect(mockVault.authStatus).toBe('unauthenticated');
    });
  });
});