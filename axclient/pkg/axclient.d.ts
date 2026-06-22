/* tslint:disable */
/* eslint-disable */
export class LoginHandler {
  free(): void;
  constructor(password: string);
  finish_login(response: string): LoginResult;
  request(): string;
}
export class LoginResult {
  private constructor();
  free(): void;
  readonly export_key: string;
  readonly session_key: string;
  readonly serevr_pub_key: string;
  readonly finish_login_request: string;
}
export class RegistrationHandler {
  free(): void;
  constructor(password: string);
  finish_registration(response: string): RegistrationResult;
  password(): string;
  request(): string;
}
export class RegistrationResult {
  private constructor();
  free(): void;
  readonly export_key: string;
  readonly server_public_key: string;
  readonly record: string;
}
export class ResetPasswordHandler {
  free(): void;
  constructor(password: string, reset_code: string);
  finish_registration(response: string): RegistrationResult;
  password(): string;
  code(): string;
  request(): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_registrationhandler_free: (a: number, b: number) => void;
  readonly __wbg_registrationresult_free: (a: number, b: number) => void;
  readonly registrationresult_export_key: (a: number) => [number, number];
  readonly registrationresult_server_public_key: (a: number) => [number, number];
  readonly registrationresult_record: (a: number) => [number, number];
  readonly registrationhandler_start: (a: number, b: number) => [number, number, number];
  readonly registrationhandler_finish_registration: (a: number, b: number, c: number) => [number, number, number];
  readonly registrationhandler_password: (a: number) => [number, number];
  readonly registrationhandler_request: (a: number) => [number, number];
  readonly __wbg_loginhandler_free: (a: number, b: number) => void;
  readonly __wbg_loginresult_free: (a: number, b: number) => void;
  readonly loginhandler_start: (a: number, b: number) => [number, number, number];
  readonly loginhandler_finish_login: (a: number, b: number, c: number) => [number, number, number];
  readonly loginhandler_request: (a: number) => [number, number];
  readonly loginresult_export_key: (a: number) => [number, number];
  readonly loginresult_session_key: (a: number) => [number, number];
  readonly loginresult_serevr_pub_key: (a: number) => [number, number];
  readonly loginresult_finish_login_request: (a: number) => [number, number];
  readonly __wbg_resetpasswordhandler_free: (a: number, b: number) => void;
  readonly resetpasswordhandler_start: (a: number, b: number, c: number, d: number) => [number, number, number];
  readonly resetpasswordhandler_finish_registration: (a: number, b: number, c: number) => [number, number, number];
  readonly resetpasswordhandler_password: (a: number) => [number, number];
  readonly resetpasswordhandler_code: (a: number) => [number, number];
  readonly resetpasswordhandler_request: (a: number) => [number, number];
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_export_2: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
