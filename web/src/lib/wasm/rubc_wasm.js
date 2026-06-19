/* @ts-self-types="./rubc_wasm.d.ts" */

/**
 * A browser-driveable Game Boy: wraps a [`MachineNg`] plus reusable scratch
 * buffers for the RGBA frame and drained audio samples.
 */
export class RubcWasm {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RubcWasmFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_rubcwasm_free(ptr, 0);
    }
    /**
     * Drain accumulated APU samples (interleaved stereo L/R `f32`) for the Web
     * Audio API and return them as a `Float32Array`. Returns an empty array if
     * no samples are queued.
     * @returns {Float32Array}
     */
    drain_audio() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rubcwasm_drain_audio(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayF32FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export(r0, r1 * 4, 4);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Length in bytes of the RGBA framebuffer returned by [`Self::frame_rgba`].
     * @returns {number}
     */
    get frame_len() {
        const ret = wasm.rubcwasm_frame_len(this.__wbg_ptr);
        return ret >>> 0;
    }
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
     * @returns {number}
     */
    frame_rgba() {
        const ret = wasm.rubcwasm_frame_rgba(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Copy the RGBA framebuffer into a fresh `Uint8Array` (a convenience
     * alternative to the zero-copy [`Self::frame_rgba`] pointer path).
     * @returns {Uint8Array}
     */
    frame_rgba_copy() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rubcwasm_frame_rgba_copy(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * True if the loaded cartridge has battery-backed RAM (i.e. a persistable
     * `.sav`). When false, `save_ram` returns an empty array and there is
     * nothing to persist.
     * @returns {boolean}
     */
    get has_battery() {
        const ret = wasm.rubcwasm_has_battery(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Screen height in pixels (144).
     * @returns {number}
     */
    get height() {
        const ret = wasm.rubcwasm_height(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * True if the loaded cartridge runs in Game Boy Color mode.
     * @returns {boolean}
     */
    get is_cgb() {
        const ret = wasm.rubcwasm_is_cgb(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Restore battery-backed RAM previously produced by [`Self::save_ram`].
     * Sizes that don't match the cart's RAM are ignored by the core. Call this
     * right after constructing the machine, before the first frame.
     * @param {Uint8Array} data
     */
    load_ram(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.rubcwasm_load_ram(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {Uint8Array} data
     * @returns {boolean}
     */
    load_state(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.rubcwasm_load_state(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Boot a machine from a raw ROM image.
     *
     * Mode (DMG vs CGB) is auto-detected from the cartridge header byte at
     * `0x0143` (bit 7 set => Game Boy Color). `sample_rate` is the Web Audio
     * context rate in Hz; pass `0` to use the 48 kHz default.
     * @param {Uint8Array} rom
     * @param {number} sample_rate
     * @param {string | null} [boot_mode]
     */
    constructor(rom, sample_rate, boot_mode) {
        const ptr0 = passArray8ToWasm0(rom, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(boot_mode) ? 0 : passStringToWasm0(boot_mode, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.rubcwasm_new(ptr0, len0, sample_rate, ptr1, len1);
        this.__wbg_ptr = ret;
        RubcWasmFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Snapshot the cartridge's battery-backed RAM as a fresh `Uint8Array`,
     * suitable for writing to browser storage (IndexedDB). Empty if the cart
     * has no battery.
     * @returns {Uint8Array}
     */
    save_ram() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rubcwasm_save_ram(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @returns {Uint8Array}
     */
    save_state() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rubcwasm_save_state(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set a joypad button's pressed state. `button` is one of the codes
     * documented on [`button_from_code`] (0=A, 1=B, 2=Select, 3=Start,
     * 4=Right, 5=Left, 6=Up, 7=Down); out-of-range codes are ignored.
     * @param {number} button
     * @param {boolean} pressed
     */
    set_button(button, pressed) {
        wasm.rubcwasm_set_button(this.__wbg_ptr, button, pressed);
    }
    /**
     * Advance the emulator until the next VBlank (one full rendered frame).
     */
    step_frame() {
        wasm.rubcwasm_step_frame(this.__wbg_ptr);
    }
    /**
     * Screen width in pixels (160).
     * @returns {number}
     */
    get width() {
        const ret = wasm.rubcwasm_width(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) RubcWasm.prototype[Symbol.dispose] = RubcWasm.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_ea4887a5f8f9a9db: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_error_a6fa202b58aa1cd3: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_export(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_new_227d7c05414eb861: function() {
            const ret = new Error();
            return addHeapObject(ret);
        },
        __wbg_stack_3b0d974bbf31e44f: function(arg0, arg1) {
            const ret = getObject(arg1).stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbindgen_object_drop_ref: function(arg0) {
            takeObject(arg0);
        },
    };
    return {
        __proto__: null,
        "./rubc_wasm_bg.js": import0,
    };
}

const RubcWasmFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_rubcwasm_free(ptr, 1));

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function dropObject(idx) {
    if (idx < 1028) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getObject(idx) { return heap[idx]; }

let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('rubc_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
