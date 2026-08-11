/**
 * UpdateBanner component tests.
 *
 * Fetches update info via invoke('check_for_updates') on mount (unless demo
 * mode), and shows a dismissible banner when an update is available. Covers:
 * show-when-outdated, hidden-when-current, and dismiss.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/svelte';

vi.mock('../constants', () => ({ IS_DEMO: false }));

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: any[]) => invokeMock(...a) }));

const { default: UpdateBanner } = await import('./UpdateBanner.svelte');

const OUTDATED = {
  local_version: '0.1.0-beta.2',
  latest_version: '0.1.1',
  up_to_date: false,
  release_url: 'https://github.com/MarcusBrammeier/gullbur-enclave/releases',
  release_notes: null,
  prerelease: false,
  error: null,
};

describe('UpdateBanner.svelte', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => cleanup());

  it('shows the update banner when an update is available', async () => {
    invokeMock.mockResolvedValue(OUTDATED);
    render(UpdateBanner);
    await waitFor(() => {
      expect(screen.getAllByText(/v0\.1\.1/i).length).toBeGreaterThan(0);
    });
    expect(screen.getAllByText(/available/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Download/i).length).toBeGreaterThan(0);
  });

  it('shows no banner when already up to date', async () => {
    invokeMock.mockResolvedValue({ ...OUTDATED, up_to_date: true });
    render(UpdateBanner);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalled();
    });
    expect(screen.queryByText(/available/i)).toBeNull();
  });

  it('dismisses the banner when the close button is clicked', async () => {
    invokeMock.mockResolvedValue(OUTDATED);
    render(UpdateBanner);
    await waitFor(() => {
      expect(screen.getAllByText(/v0\.1\.1/i).length).toBeGreaterThan(0);
    });
    // Dismiss is the icon-only secondary button (no accessible text).
    const dismissBtn = screen.getAllByRole('button')[1];
    await fireEvent.click(dismissBtn);
    expect(screen.queryByText(/available/i)).toBeNull();
  });

  it('shows the prerelease badge when the update is a prerelease', async () => {
    invokeMock.mockResolvedValue({ ...OUTDATED, prerelease: true });
    render(UpdateBanner);
    await waitFor(() => {
      expect(screen.getAllByText(/pre-release/i).length).toBeGreaterThan(0);
    });
  });
});