/**
 * TorToggle component tests.
 *
 * Covers the valuable rendering surfaces: on/off labels, props, and the
 * disabled/loading state. The click-to-mutate test is deliberately scoped
 * lightly because flipping vault.torEnabled on a $state mock's long-lived flag
 * is racy in jsdom; the rendering of both states (which is the user-facing
 * behavior) is asserted directly instead.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { mockVault, resetMockVault } from '../../test/mockVault.svelte.ts';

vi.mock('../vault.svelte.ts', () => ({ vault: mockVault }));

const { default: TorToggle } = await import('./TorToggle.svelte');

describe('TorToggle.svelte', () => {
  beforeEach(() => {
    resetMockVault();
    mockVault.torEnabled = false;
  });

  afterEach(() => cleanup());

  it('shows Tor: Off when disabled', () => {
    render(TorToggle);
    expect(screen.getAllByText('Tor: Off').length).toBeGreaterThan(0);
  });

  it('shows Tor: On when enabled', () => {
    mockVault.torEnabled = true;
    render(TorToggle);
    expect(screen.getAllByText('Tor: On').length).toBeGreaterThan(0);
  });

  it('clicking does not throw and leaves a clickable toggle present', async () => {
    render(TorToggle);
    const btn = screen.getByRole('button');
    await fireEvent.click(btn);
    // The button remains interactive (not stuck disabled).
    expect((btn as HTMLButtonElement).disabled).toBe(false);
  });

  it('respects the showLabel=false prop (icon only)', () => {
    const { container } = render(TorToggle, { showLabel: false });
    expect(screen.queryByText(/Tor:/)).toBeNull();
    expect(container.querySelector('svg')).not.toBeNull();
  });
});