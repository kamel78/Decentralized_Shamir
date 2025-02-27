/* tslint:disable */
/* eslint-disable */
export class PubKeyAdder {
  free(): void;
  constructor();
  add(new_point: string): void;
  get_pubkey(): string;
}
export class WShamirUser {
  free(): void;
  constructor(js_users_list: any, username: string, threshold: number);
  static new_from_serialized(json_string: string): WShamirUser;
  serialize(): string;
  update_share(in_user: string, in_share_part: string): void;
  get_share(): string;
  get_secret_part_for_user(in_user: string): string;
  generate_secret(): void;
  get_partial_pubkey(): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_wshamiruser_free: (a: number, b: number) => void;
  readonly wshamiruser_new: (a: any, b: number, c: number, d: number) => number;
  readonly wshamiruser_new_from_serialized: (a: number, b: number) => number;
  readonly wshamiruser_serialize: (a: number) => [number, number];
  readonly wshamiruser_update_share: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly wshamiruser_get_share: (a: number) => [number, number];
  readonly wshamiruser_get_secret_part_for_user: (a: number, b: number, c: number) => [number, number];
  readonly wshamiruser_generate_secret: (a: number) => void;
  readonly wshamiruser_get_partial_pubkey: (a: number) => [number, number];
  readonly __wbg_pubkeyadder_free: (a: number, b: number) => void;
  readonly pubkeyadder_new: () => number;
  readonly pubkeyadder_add: (a: number, b: number, c: number) => void;
  readonly pubkeyadder_get_pubkey: (a: number) => [number, number];
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_export_2: WebAssembly.Table;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
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
