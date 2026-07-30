/* tslint:disable */
/* eslint-disable */

/**
 * Decrypt an AES-256-GCM encrypted payload.
 *
 * - `key_hex`: 64-char hex string (32 bytes raw)
 * - `payload_json`: JSON string `{"iv_hex": "...", "data_b64": "..."}`
 *
 * Returns the original JSON string.
 */
export function decrypt(key_hex: string, payload_json: string): string;

/**
 * Encrypt a JSON string with AES-256-GCM.
 *
 * - `key_hex`: 64-char hex string (32 bytes raw)
 * - `json_data`: any JSON-serializable string
 *
 * Returns a JSON string: `{"iv_hex": "...", "data_b64": "..."}`
 */
export function encrypt(key_hex: string, json_data: string): string;

/**
 * Generate a random AES-256-GCM key.
 * Returns a 64-char hex string.
 */
export function generate_key(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly decrypt: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly encrypt: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly generate_key: (a: number) => void;
    readonly __wbindgen_export: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export2: (a: number, b: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
