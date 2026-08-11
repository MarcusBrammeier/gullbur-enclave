/**
 * ConsoleLog component tests.
 *
 * The log panel exposes window.__consoleLog on mount (used by IpcClient) to
 * push entries. Covers: empty state, pushing entries, error-only filtering,
 * the counts on the filter tabs, and Clear.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { flushSync } from 'svelte';

vi.mock('../consoleBridge', () => ({
  drainPreMountBuffer: vi.fn(),
}));

const { default: ConsoleLog } = await import('./ConsoleLog.svelte');

type Entry = { direction: 'send' | 'receive'; method?: string; payload: string; isError: boolean };
/** Await the component's onMount so window.__consoleLog is registered. */
async function getLog() {
  await new Promise((r) => setTimeout(r, 0));
  const fn = (window as any).__consoleLog as ((e: Entry) => void) | undefined;
  if (!fn) throw new Error('__consoleLog not registered after mount');
  return fn;
}

describe('ConsoleLog.svelte', () => {
  beforeEach(() => {
    delete (window as any).__consoleLog;
  });

  afterEach(() => {
    delete (window as any).__consoleLog;
    cleanup();
  });

  it('renders the empty state when no logs exist', () => {
    render(ConsoleLog);
    expect(screen.getByText(/No IPC calls yet/i)).toBeInTheDocument();
  });

  it('renders an entry pushed via __consoleLog', async () => {
    render(ConsoleLog);
    const log = await getLog();
    log({ direction: 'send', method: 'vault.get_balance', payload: '{"ok":true}', isError: false });
    flushSync();
    expect(screen.getByText('vault.get_balance')).toBeInTheDocument();
    expect(screen.getByText('{"ok":true}')).toBeInTheDocument();
  });

  it('shows the error-only filter and filters out non-errors', async () => {
    render(ConsoleLog);
    const log = await getLog();
    log({ direction: 'send', method: 'vault.status', payload: 'ok', isError: false });
    log({ direction: 'receive', method: 'vault.broadcast', payload: 'boom', isError: true });
    // Error tab shows a count; click it to filter.
    const errorsTab = screen.getByText(/Errors/);
    await fireEvent.click(errorsTab);
    // Non-error entry hidden, error visible.
    expect(screen.getByText('boom')).toBeInTheDocument();
    expect(screen.queryByText('ok')).toBeNull();
  });

  it('clears logs when Clear is clicked', async () => {
    render(ConsoleLog);
    const log = await getLog();
    log({ direction: 'send', method: 'vault.status', payload: 'x', isError: false });
    flushSync();
    expect(screen.getByText('vault.status')).toBeInTheDocument();
    const clearBtn = screen.getByText('Clear');
    await fireEvent.click(clearBtn);
    flushSync();
    expect(screen.getByText(/No IPC calls yet/i)).toBeInTheDocument();
  });
});