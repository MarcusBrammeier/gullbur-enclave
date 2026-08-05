/**
 * Console Bridge — unifies WebView (frontend) logging with the in-app debug
 * console and the native/Rust side.
 *
 * Three sinks:
 *   1. Native console (devtools / WebKit inspector) — keeps normal behaviour.
 *   2. In-app Debug Console panel (`window.__consoleLog`, rendered by
 *      ConsoleLog.svelte) — so GUI-side errors appear in the wallet UI.
 *   3. Rust side (`webview_log` Tauri command) — so WebView errors surface in
 *      the terminal (desktop) / logcat (Android), giving the backend a full
 *      picture even when the web inspector isn't open.
 *
 * Idempotent and best-effort: never throws, safe to call multiple times.
 */

type LogLevel = 'log' | 'info' | 'warn' | 'error' | 'debug';

interface BridgeEntry {
  direction: 'send' | 'receive';
  method?: string;
  payload: string;
  isError: boolean;
}

const MAX_BUFFER = 256;
/** Ring buffer so logs emitted before the panel mounts aren't lost. */
let preMountBuffer: BridgeEntry[] = [];

let installed = false;

/** Route an entry to the in-app ConsoleLog panel (if any). */
function toPanel(payload: string, isError: boolean, method?: string): void {
  const entry: BridgeEntry = { direction: 'receive', method, payload, isError };
  const sink = (window as any).__consoleLog as
    | ((e: Omit<BridgeEntry, 'id' | 'timestamp'>) => void)
    | undefined;
  if (sink) {
    sink(entry);
  } else {
    preMountBuffer.push(entry);
    if (preMountBuffer.length > MAX_BUFFER) preMountBuffer.shift();
  }
}

/** Format console args like the native console does (join with spaces). */
function fmt(args: unknown[]): string {
  return args
    .map((a) => {
      if (typeof a === 'string') return a;
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(' ');
}

/** Forward to Rust logs. Best-effort; swallowed if it fails. */
function toRust(level: LogLevel, message: string): void {
  try {
    // Dynamic import keeps this from breaking browser-only (vite) dev.
    import('@tauri-apps/api/core')
      .then(({ invoke }) =>
        invoke('webview_log', { level, message }).catch(() => {})
      )
      .catch(() => {});
  } catch {
    /* ignore */
  }
}

/** Tail of the pre-mount buffer, drained once the panel is ready. */
export function drainPreMountBuffer(sink: (e: BridgeEntry) => void): void {
  const buffered = preMountBuffer.splice(0, preMountBuffer.length);
  for (const e of buffered) sink(e);
}

export function installConsoleBridge(): void {
  if (installed || typeof window === 'undefined') return;
  installed = true;

  const original = {
    log: console.log,
    info: console.info,
    warn: console.warn,
    error: console.error,
    debug: console.debug,
  };

  const wrap = (level: LogLevel): void => {
    const native = original[level];
    (console as any)[level] = (...args: unknown[]) => {
      const message = fmt(args);
      // Keep native console behaviour (the original fn).
      native.apply(console, args as never[]);
      // In-app panel + Rust bridge.
      const isError = level === 'error' || level === 'warn';
      toPanel(message, isError, `console.${level}`);
      toRust(level, message);
    };
  };

  (['log', 'info', 'warn', 'error', 'debug'] as LogLevel[]).forEach(wrap);

  // Capture uncaught exceptions + unhandled rejections as console.error.
  window.addEventListener('error', (e: ErrorEvent) => {
    const msg = e.error instanceof Error ? e.error.stack ?? e.message : `${e.message}`;
    original.error(`[Uncaught Error] ${msg}`);
    toPanel(`[Uncaught Error] ${e.message}`, true, 'window.error');
    toRust('error', `[Uncaught Error] ${e.message}`);
  });

  window.addEventListener('unhandledrejection', (e: PromiseRejectionEvent) => {
    const reason =
      e.reason instanceof Error ? e.reason.stack ?? e.reason.message : String(e.reason);
    original.error(`[Unhandled Rejection] ${reason}`);
    toPanel(`[Unhandled Rejection] ${reason}`, true, 'unhandledrejection');
    toRust('error', `[Unhandled Rejection] ${reason}`);
  });
}
