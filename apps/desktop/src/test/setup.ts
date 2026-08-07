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
