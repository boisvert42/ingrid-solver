/* tslint:disable */
/* eslint-disable */

export class WasmSolver {
    free(): void;
    [Symbol.dispose](): void;
    get_cell_names(): string;
    get_prefills(): string;
    get_slot_configs(): string;
    load_dictionary(dict_contents: string): void;
    constructor(slots_string: string);
    run_ac3(fill_json: string): string;
    set_min_score(min_score: number): void;
    validate_candidate(slot_id: number, word: string, fill_json: string): boolean;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmsolver_free: (a: number, b: number) => void;
    readonly wasmsolver_get_cell_names: (a: number) => [number, number];
    readonly wasmsolver_get_prefills: (a: number) => [number, number];
    readonly wasmsolver_get_slot_configs: (a: number) => [number, number];
    readonly wasmsolver_load_dictionary: (a: number, b: number, c: number) => [number, number];
    readonly wasmsolver_new: (a: number, b: number) => [number, number, number];
    readonly wasmsolver_run_ac3: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmsolver_set_min_score: (a: number, b: number) => void;
    readonly wasmsolver_validate_candidate: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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
