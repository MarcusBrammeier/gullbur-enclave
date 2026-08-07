/**
 * Global toast store — a FIFO queue of short-lived notices (errors/warnings).
 *
 * Displays one toast at a time for `TOAST_MS` (3s), then advances to the next
 * queued item. New toasts are appended; if one is showing, the next appears
 * after the current one expires (no stacking / no overlap).
 *
 * Reactive via Svelte 5 `$state` so components re-render automatically.
 */

export interface Toast {
  id: number;
  level: 'error' | 'warning' | 'info';
  message: string;
}

const TOAST_MS = 3_000;

// Internal reactive state.
const toasts = $state<Toast[]>([]);
let nextId = 1;
let timer: ReturnType<typeof setTimeout> | null = null;

/**
 * The toast currently being displayed, or null.
 * Svelte 5 forbids exporting `$derived` from a module, so expose getters.
 */
export function currentToast(): Toast | null {
  return toasts[0] ?? null;
}

/** True while a toast is showing. */
export function hasToast(): boolean {
  return toasts.length > 0;
}

function advance(): void {
  // Pop the front item, then show the next (if any) for its own duration.
  toasts.shift();
  scheduleNext();
}

function scheduleNext(): void {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  if (toasts.length === 0) return;
  timer = setTimeout(advance, TOAST_MS);
}

/** Enqueue a notice. Dedupes consecutive identical messages. */
export function pushToast(level: Toast['level'], message: string): void {
  const prev = toasts[toasts.length - 1];
  if (prev && prev.level === level && prev.message === message) {
    return; // skip exact consecutive dupes (error storm suppression)
  }
  toasts.push({ id: nextId++, level, message });
  scheduleNext();
}

export function pushError(message: string): void {
  pushToast('error', message);
}

export function pushWarning(message: string): void {
  pushToast('warning', message);
}

export function pushInfo(message: string): void {
  pushToast('info', message);
}

/** Manually dismiss the current toast immediately. */
export function dismissToast(): void {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  advance();
}

/** Clear all queued toasts (e.g. on app teardown). */
export function clearToasts(): void {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  toasts.length = 0;
}
