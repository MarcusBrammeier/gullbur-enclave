/**
 * Toast store unit tests — exercises the queue/cycling behavior with fake timers.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  pushToast,
  pushError,
  currentToast,
  hasToast,
  dismissToast,
  clearToasts,
} from './toasts.svelte.ts';

describe('toasts.svelte.ts', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    clearToasts();
  });
  afterEach(() => {
    vi.useRealTimers();
    clearToasts();
  });

  it('starts empty', () => {
    expect(hasToast()).toBe(false);
    expect(currentToast()).toBeNull();
  });

  it('shows the current toast after a push', () => {
    pushError('boom');
    expect(hasToast()).toBe(true);
    expect(currentToast()?.message).toBe('boom');
    expect(currentToast()?.level).toBe('error');
  });

  it('auto-advances to the next queued toast after 3s', () => {
    pushToast('error', 'first');
    pushToast('error', 'second');

    // First toast visible initially.
    expect(currentToast()?.message).toBe('first');

    // After 3s the first expires → second shows.
    vi.advanceTimersByTime(3_000);
    expect(currentToast()?.message).toBe('second');

    // After another 3s the queue drains.
    vi.advanceTimersByTime(3_000);
    expect(currentToast()).toBeNull();
    expect(hasToast()).toBe(false);
  });

  it('does not show the second toast until the first expires (FIFO, no overlap)', () => {
    pushToast('error', 'first');
    pushToast('error', 'second');

    expect(currentToast()?.message).toBe('first');

    // Mid-cycle: second should NOT be visible yet.
    vi.advanceTimersByTime(1_500);
    expect(currentToast()?.message).toBe('first');
  });

  it('drops exact consecutive duplicate messages (error storm suppression)', () => {
    pushToast('error', 'flood');
    pushToast('error', 'flood'); // dup — ignored
    pushToast('error', 'flood'); // dup — ignored

    // Only one reported, then drains.
    expect(currentToast()?.message).toBe('flood');
    vi.advanceTimersByTime(3_000);
    expect(currentToast()).toBeNull();
  });

  it('dismissToast clears the current toast immediately and advances', () => {
    pushToast('error', 'first');
    pushToast('error', 'second');

    dismissToast();
    expect(currentToast()?.message).toBe('second');
  });
});
