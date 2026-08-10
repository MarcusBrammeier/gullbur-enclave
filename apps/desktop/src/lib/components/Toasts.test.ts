/**
 * Toasts component tests — verifies the toast UI renders and dismisses.
 *
 * The store logic (queue/FIFO/dedupe) is covered in toasts.test.ts; here we
 * mount the actual component against the real store to prove the DOM renders
 * the current toast and the dismiss button wires to dismissToast().
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { pushError, pushWarning, pushInfo, clearToasts } from '../toasts.svelte.ts';

const { default: Toasts } = await import('./Toasts.svelte');

describe('Toasts.svelte', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    clearToasts();
  });

  afterEach(() => {
    vi.useRealTimers();
    clearToasts();
    cleanup();
  });

  it('renders nothing when there is no toast', () => {
    render(Toasts);
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('renders the toast message when one is pushed', () => {
    render(Toasts);
    pushError('funds below minimum');
    flushSync(); // flush $state store -> component $derived -> DOM
    const status = screen.getByRole('status');
    expect(status).toBeInTheDocument();
    expect(status.textContent).toContain('funds below minimum');
  });

  it('renders the correct icon per level', () => {
    render(Toasts);
    pushInfo('note');
    flushSync();
    expect(screen.getByRole('status').textContent).toContain('ℹ️');
    // Reset the singleton queue so the second render shows the warning cleanly.
    clearToasts();
    cleanup();
    render(Toasts);
    pushWarning('careful');
    flushSync();
    expect(screen.getByRole('status').textContent).toContain('⚠️');
  });

  it('dismiss button calls dismissToast and clears the current toast', async () => {
    render(Toasts);
    pushError('dismiss me');
    flushSync();
    expect(screen.getByRole('status')).toBeInTheDocument();

    const dismissBtn = screen.getByRole('button', { name: /Dismiss notification/i });
    await fireEvent.click(dismissBtn);
    flushSync();
    // After dismiss with no queue, currentToast is null → nothing renders.
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('is aria-live assertive for screen readers', () => {
    render(Toasts);
    pushError('accessible');
    flushSync();
    const status = screen.getByRole('status');
    expect(status.getAttribute('aria-live')).toBe('assertive');
  });
});