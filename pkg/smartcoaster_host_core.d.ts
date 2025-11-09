/* tslint:disable */
/* eslint-disable */
export function init_logging(): void;
export enum WasmSessionError {
  FramingError = 0,
  RxBufferNotEnoughSpace = 1,
  UnexpectedMessage = 2,
  IncorrectDeviceMode = 3,
  SessionEnded = 4,
  ChunkRequestOutOfBounds = 5,
}
export class WasmFirmwareLoader {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get current download progress
   */
  get_progress(): WasmProgress | undefined;
  /**
   * Initialize the firmware loader session
   */
  init_session(): void;
  /**
   * Check if session has ended
   */
  is_session_ended(): boolean;
  /**
   * Get bytes that need to be sent to the device
   */
  get_bytes_to_send(): Uint8Array | undefined;
  /**
   * Get firmware size in bytes
   */
  get_firmware_size(): number;
  /**
   * Process incoming bytes from the device and advance session state
   */
  handle_incoming_bytes(incoming_bytes: Uint8Array): void;
  /**
   * Create a new firmware loader with the provided firmware bytes
   */
  constructor(firmware_bytes: Uint8Array);
}
export class WasmProgress {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  max_chunks: number;
  current_chunk: number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_get_wasmprogress_current_chunk: (a: number) => number;
  readonly __wbg_get_wasmprogress_max_chunks: (a: number) => number;
  readonly __wbg_set_wasmprogress_current_chunk: (a: number, b: number) => void;
  readonly __wbg_set_wasmprogress_max_chunks: (a: number, b: number) => void;
  readonly __wbg_wasmfirmwareloader_free: (a: number, b: number) => void;
  readonly __wbg_wasmprogress_free: (a: number, b: number) => void;
  readonly init_logging: () => void;
  readonly wasmfirmwareloader_get_bytes_to_send: (a: number) => [number, number];
  readonly wasmfirmwareloader_get_firmware_size: (a: number) => number;
  readonly wasmfirmwareloader_get_progress: (a: number) => number;
  readonly wasmfirmwareloader_handle_incoming_bytes: (a: number, b: number, c: number) => [number, number];
  readonly wasmfirmwareloader_init_session: (a: number) => [number, number];
  readonly wasmfirmwareloader_is_session_ended: (a: number) => number;
  readonly wasmfirmwareloader_new: (a: number, b: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
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
