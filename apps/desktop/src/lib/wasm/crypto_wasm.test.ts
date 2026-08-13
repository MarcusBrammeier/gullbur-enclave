/**
 * WASM crypto round-trip test — verifies the REAL packaged crypto_wasm_bg.wasm.
 *
 * This is the guard against the documented WASM-trap failure mode ("Unreachable
 * code should not be executed" — ABI mismatch, stale blob, or getrandom backend
 * breakage). It loads the exact byte blob the frontend ships via IpcClient.ts →
 * src/lib/wasm/crypto_wasm.js, so a stale/regenerated wasm that no longer
 * encrypt/decrypt round-trips is caught here — something nothing else tests.
 *
 * Uses initSync() with the actual .wasm bytes (read via fs) so it exercises the
 * real artifact rather than a stubbed fetch/URL path.
 */
// @vitest-environment node
// Runs outside jsdom because it reads the .wasm bytes from disk and needs the
// node WebAssembly + crypto.getRandomValues runtime, not a DOM.
import { describe, it, expect, beforeAll } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import init, {
  initSync,
  generate_key,
  encrypt,
  decrypt,
} from "./crypto_wasm.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WASM_PATH = join(__dirname, "crypto_wasm_bg.wasm");

describe("crypto_wasm (packaged blob round-trip)", () => {
  beforeAll(async () => {
    // Load the real shipped wasm bytes synchronously — bypasses the URL/fetch
    // path entirely so the test fails loudly if the blob itself is broken.
    const bytes = readFileSync(WASM_PATH);
    initSync({ module: bytes });
  });

  it("generate_key returns a 64-char hex (32 raw bytes)", () => {
    const key = generate_key();
    expect(key).toMatch(/^[0-9a-f]{64}$/i);
    // Two calls must differ (randomness actually wired to getRandomValues)
    expect(generate_key()).not.toBe(key);
  });

  it("encrypt → decrypt round-trips a JSON object", () => {
    const key = generate_key();
    const doc = { jsonrpc: "2.0", method: "get_balance", params: { account: 3 }, id: 7 };
    const ct = encrypt(key, JSON.stringify(doc));
    expect(ct).toContain('"iv_hex"');
    expect(ct).toContain('"data_b64"');

    const pt = decrypt(key, ct);
    expect(JSON.parse(pt)).toEqual(doc);
  });

  it("round-trips arbitrary unicode and empty objects", () => {
    const key = generate_key();
    const doc = { note: "héllo 世界 wörld", xs: [], nested: { ok: true } };
    const pt = decrypt(key, encrypt(key, JSON.stringify(doc)));
    expect(JSON.parse(pt)).toEqual(doc);

    const emptyPt = decrypt(key, encrypt(key, "{}"));
    expect(JSON.parse(emptyPt)).toEqual({});
  });

  it("the same plaintext produces different ciphertext each time (fresh IV)", () => {
    const key = generate_key();
    const doc = JSON.stringify({ field: "constant-value" });
    const a = encrypt(key, doc);
    const b = encrypt(key, doc);
    expect(a).not.toBe(b);
    expect(decrypt(key, a)).toBe(doc);
    expect(decrypt(key, b)).toBe(doc);
  });

  it("decrypt fails on the wrong key", () => {
    const keyA = generate_key();
    const keyB = generate_key();
    const ct = encrypt(keyA, JSON.stringify({ secret: 1 }));
    expect(() => decrypt(keyB, ct)).toThrow();
  });

  it("rejects a non-32-byte key and malformed JSON", () => {
    expect(() => encrypt("abcd", "{}")).toThrow();
    expect(() => encrypt(generate_key(), "not-json{{")).toThrow();
  });

  it("the wasm default init() also loads (fetch path used at runtime)", async () => {
    // Ensure the async init() path the app actually calls works too.
    await init();
  });
});