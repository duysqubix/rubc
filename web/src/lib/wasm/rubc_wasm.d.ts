/* tslint:disable */
/* eslint-disable */

/**
 * A browser-driveable Game Boy: wraps a [`MachineNg`] plus reusable scratch
 * buffers for the RGBA frame and drained audio samples.
 */
export class RubcWasm {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Drain accumulated APU samples (interleaved stereo L/R `f32`) for the Web
     * Audio API and return them as a `Float32Array`. Returns an empty array if
     * no samples are queued.
     */
    drain_audio(): Float32Array;
    /**
     * Resolve the current PPU framebuffer into the internal RGBA buffer and
     * return a pointer to its first byte.
     *
     * The buffer is exactly `width * height * 4` bytes (RGBA8888). Read it from
     * JS with a typed-array view over the wasm memory, recreating the view each
     * call because memory growth can detach the previous `ArrayBuffer`:
     * ```js
     * const ptr = emu.frame_rgba();
     * const px  = new Uint8ClampedArray(wasm.memory.buffer, ptr, emu.frame_len);
     * imageData.data.set(px);
     * ```
     */
    frame_rgba(): number;
    /**
     * Copy the RGBA framebuffer into a fresh `Uint8Array` (a convenience
     * alternative to the zero-copy [`Self::frame_rgba`] pointer path).
     */
    frame_rgba_copy(): Uint8Array;
    /**
     * Restore battery-backed RAM previously produced by [`Self::save_ram`].
     * Sizes that don't match the cart's RAM are ignored by the core. Call this
     * right after constructing the machine, before the first frame.
     */
    load_ram(data: Uint8Array): void;
    load_state(data: Uint8Array): boolean;
    /**
     * Boot a machine from a raw ROM image.
     *
     * Mode (DMG vs CGB) is auto-detected from the cartridge header byte at
     * `0x0143` (bit 7 set => Game Boy Color). `sample_rate` is the Web Audio
     * context rate in Hz; pass `0` to use the 48 kHz default.
     */
    constructor(rom: Uint8Array, sample_rate: number, boot_mode?: string | null);
    /**
     * Snapshot the cartridge's battery-backed RAM as a fresh `Uint8Array`,
     * suitable for writing to browser storage (IndexedDB). Empty if the cart
     * has no battery.
     */
    save_ram(): Uint8Array;
    save_state(): Uint8Array;
    /**
     * Set a joypad button's pressed state. `button` is one of the codes
     * documented on [`button_from_code`] (0=A, 1=B, 2=Select, 3=Start,
     * 4=Right, 5=Left, 6=Up, 7=Down); out-of-range codes are ignored.
     */
    set_button(button: number, pressed: boolean): void;
    /**
     * Advance the emulator until the next VBlank (one full rendered frame).
     */
    step_frame(): void;
    /**
     * Length in bytes of the RGBA framebuffer returned by [`Self::frame_rgba`].
     */
    readonly frame_len: number;
    /**
     * True if the loaded cartridge has battery-backed RAM (i.e. a persistable
     * `.sav`). When false, `save_ram` returns an empty array and there is
     * nothing to persist.
     */
    readonly has_battery: boolean;
    /**
     * Screen height in pixels (144).
     */
    readonly height: number;
    /**
     * True if the loaded cartridge runs in Game Boy Color mode.
     */
    readonly is_cgb: boolean;
    /**
     * Screen width in pixels (160).
     */
    readonly width: number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_rubcwasm_free: (a: number, b: number) => void;
    readonly rubcwasm_drain_audio: (a: number, b: number) => void;
    readonly rubcwasm_frame_rgba: (a: number) => number;
    readonly rubcwasm_frame_rgba_copy: (a: number, b: number) => void;
    readonly rubcwasm_has_battery: (a: number) => number;
    readonly rubcwasm_is_cgb: (a: number) => number;
    readonly rubcwasm_load_ram: (a: number, b: number, c: number) => void;
    readonly rubcwasm_load_state: (a: number, b: number, c: number) => number;
    readonly rubcwasm_new: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly rubcwasm_save_ram: (a: number, b: number) => void;
    readonly rubcwasm_save_state: (a: number, b: number) => void;
    readonly rubcwasm_set_button: (a: number, b: number, c: number) => void;
    readonly rubcwasm_step_frame: (a: number) => void;
    readonly rubcwasm_frame_len: (a: number) => number;
    readonly rubcwasm_height: (a: number) => number;
    readonly rubcwasm_width: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export2: (a: number, b: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
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
