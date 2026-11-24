/* tslint:disable */
/* eslint-disable */

/**
 * Generate a FALCON keypair.
 *
 * `logn` — security level:
 *   9  → FN-DSA-512  (NIST Level I,   pub 897B,  priv 1281B)
 *   10 → FN-DSA-1024 (NIST Level V,   pub 1793B, priv 2305B)
 *
 * Returns `{ public_key: Uint8Array, private_key: Uint8Array }`.
 */
export function generate_keypair(logn: number): object;

export function init(): void;

/**
 * Sign a message with a FALCON private key.
 *
 * `private_key` — raw private key bytes (1281 for FN-DSA-512, 2305 for FN-DSA-1024)
 * `message`     — the bytes to sign
 * Returns the signature as `Uint8Array` (666 bytes for FN-DSA-512).
 */
export function sign(private_key: Uint8Array, message: Uint8Array): Uint8Array;

/**
 * Verify a FALCON signature.
 *
 * `public_key` — raw public key bytes (897 for FN-DSA-512, 1793 for FN-DSA-1024)
 * `message`    — the original signed bytes
 * `signature`  — the signature bytes (666 for FN-DSA-512)
 * Returns `true` if the signature is valid.
 */
export function verify(public_key: Uint8Array, message: Uint8Array, signature: Uint8Array): boolean;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly generate_keypair: (a: number, b: number) => void;
    readonly init: () => void;
    readonly sign: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly verify: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_start: () => void;
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
