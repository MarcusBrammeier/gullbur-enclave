// Vitest setup — extend expect with jest-dom DOM matchers
// and polyfill anything the Svelte components need under jsdom.
import '@testing-library/jest-dom/vitest';

// jsdom lacks requestAnimationFrame in some configs — some Svelte transitions
// touch it; providing a no-op keeps component render deterministic.
if (typeof globalThis.requestAnimationFrame !== 'function') {
  globalThis.requestAnimationFrame = (cb: FrameRequestCallback) =>
    setTimeout(() => cb(performance.now()), 16) as unknown as number;
}
if (typeof globalThis.cancelAnimationFrame !== 'function') {
  globalThis.cancelAnimationFrame = (id: number) => clearTimeout(id);
}

// Some jsdom/vitest combinations expose `window` without `localStorage`
// (opaque origin). Components that read the address book throw on mount, which
// made 21 Send tests red regardless of the code under test. Provide a real
// in-memory Storage implementation when one is missing.
if (typeof globalThis.localStorage === 'undefined') {
  const store = new Map<string, string>();
  const storage: Storage = {
    get length() {
      return store.size;
    },
    clear: () => store.clear(),
    getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
    key: (i: number) => Array.from(store.keys())[i] ?? null,
    removeItem: (k: string) => {
      store.delete(k);
    },
    setItem: (k: string, v: string) => {
      store.set(k, String(v));
    },
  };
  Object.defineProperty(globalThis, 'localStorage', {
    value: storage,
    writable: true,
    configurable: true,
  });
  if (typeof window !== 'undefined' && !window.localStorage) {
    Object.defineProperty(window, 'localStorage', {
      value: storage,
      writable: true,
      configurable: true,
    });
  }
}

// jsdom does not implement Element.prototype.animate — Svelte 5 transitions
// (fade, scale, etc.) call it natively; polyfill as a no-op tick so tests
// don't throw "element.animate is not a function".
if (typeof Element !== 'undefined' && !Element.prototype.animate) {
  Element.prototype.animate = function () {
    return {
      play() {},
      pause() {},
      finish() {},
      cancel() {},
      addEventListener() {},
      removeEventListener() {},
      onfinish: null,
      finished: Promise.resolve(),
      currentTime: 0,
      playbackRate: 1,
      startTime: 0,
      timeline: null,
      playState: 'finished',
      effect: null,
      persist() {},
      reverse() {},
      updatePlaybackRate() {},
    } as unknown as Animation;
  } as never;
}
