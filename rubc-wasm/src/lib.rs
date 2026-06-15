//! WebAssembly bindings for the rubc Game Boy / Game Boy Color emulator.
//!
//! This crate is a thin, `wasm-bindgen`-powered shim around the
//! platform-agnostic [`rubc_ng`] library. It owns no emulation logic: it
//! boots a [`MachineNg`], steps frames, and exposes three things JavaScript needs
//! to build a browser frontend:
//!   - an RGBA framebuffer (read straight from wasm linear memory, zero-copy),
//!   - drained stereo audio samples for the Web Audio API,
//!   - a button-press input surface.
//!
//! The core stays `#![forbid(unsafe_code)]` and untouched; all browser-specific
//! concerns (canvas, audio context, key events) live in the JS demo under
//! `web/`. No `cpal`/`winit`/`pixels` (native-only) and no C dependencies.
//!
//! ## Build
//! ```sh
//! wasm-pack build rubc-wasm --target web --out-dir web/pkg
//! # or (without wasm-pack):
//! cargo build -p rubc-wasm --target wasm32-unknown-unknown --release
//! wasm-bindgen target/wasm32-unknown-unknown/release/rubc_wasm.wasm \
//!     --target web --out-dir rubc-wasm/web/pkg
//! ```

use rubc_ng::{Button, FramePixel, MachineNg};
use wasm_bindgen::prelude::*;

const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;

/// Visible screen width in pixels (160).
pub const WIDTH: usize = SCREEN_WIDTH;
/// Visible screen height in pixels (144).
pub const HEIGHT: usize = SCREEN_HEIGHT;
/// Bytes in one RGBA framebuffer: 160 * 144 * 4.
const RGBA_LEN: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 4;

/// Default audio sample rate (Hz) used when `0` is passed to [`RubcWasm::new`].
const DEFAULT_SAMPLE_RATE: u32 = 48_000;

/// The 4 DMG shades (lightest -> darkest) as RGBA. Mirrors the native
/// frontend's palette (`rubc/src/capture.rs::DMG_SHADES`) so the browser image
/// matches what desktop players see.
const DMG_SHADES: [[u8; 4]; 4] = [
    [0xE0, 0xF8, 0xD0, 0xFF], // 0: lightest
    [0x88, 0xC0, 0x70, 0xFF], // 1
    [0x34, 0x68, 0x56, 0xFF], // 2
    [0x08, 0x18, 0x20, 0xFF], // 3: darkest
];

/// Map one resolved PPU pixel to RGBA. DMG pixels index [`DMG_SHADES`]; CGB
/// pixels expand each 5-bit RGB555 channel to 8-bit via `(x<<3)|(x>>2)` so full
/// intensity (31) maps to 255. Identical to the native `frame_pixel_rgba`.
#[inline]
fn frame_pixel_rgba(pixel: FramePixel) -> [u8; 4] {
    match pixel {
        FramePixel::DmgShade(shade) => DMG_SHADES[(shade & 0x03) as usize],
        FramePixel::CgbRgb555(rgb) => {
            let expand = |c: u16| -> u8 {
                let c = (c & 0x1F) as u8;
                (c << 3) | (c >> 2)
            };
            [expand(rgb), expand(rgb >> 5), expand(rgb >> 10), 0xFF]
        }
    }
}

/// JavaScript-facing button codes for [`RubcWasm::set_button`].
///
/// These match the declaration order of [`rubc_ng::Button`]:
///
/// | code | button |
/// |------|--------|
/// | 0    | A      |
/// | 1    | B      |
/// | 2    | Select |
/// | 3    | Start  |
/// | 4    | Right  |
/// | 5    | Left   |
/// | 6    | Up     |
/// | 7    | Down   |
fn button_from_code(code: u8) -> Option<Button> {
    Some(match code {
        0 => Button::A,
        1 => Button::B,
        2 => Button::Select,
        3 => Button::Start,
        4 => Button::Right,
        5 => Button::Left,
        6 => Button::Up,
        7 => Button::Down,
        _ => return None,
    })
}

/// A browser-driveable Game Boy: wraps a [`MachineNg`] plus reusable scratch
/// buffers for the RGBA frame and drained audio samples.
#[wasm_bindgen]
pub struct RubcWasm {
    machine: MachineNg,
    rgba: Vec<u8>,
    samples: Vec<f32>,
}

#[wasm_bindgen]
impl RubcWasm {
    /// Boot a machine from a raw ROM image.
    ///
    /// Mode (DMG vs CGB) is auto-detected from the cartridge header byte at
    /// `0x0143` (bit 7 set => Game Boy Color). `sample_rate` is the Web Audio
    /// context rate in Hz; pass `0` to use the 48 kHz default.
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8], sample_rate: u32, boot_mode: Option<String>) -> RubcWasm {
        console_error_panic_hook::set_once();

        let mut machine = match boot_mode.as_deref() {
            Some("cgb") => MachineNg::boot_cgb(rom).expect("boot CGB machine"),
            Some("dmg") => MachineNg::boot_dmg(rom).expect("boot DMG machine"),
            _ => {
                let is_cgb = rom.get(0x0143).is_some_and(|f| f & 0x80 != 0);
                if is_cgb {
                    MachineNg::boot_cgb(rom).expect("boot CGB machine")
                } else {
                    MachineNg::boot_dmg(rom).expect("boot DMG machine")
                }
            }
        };

        let rate = if sample_rate == 0 {
            DEFAULT_SAMPLE_RATE
        } else {
            sample_rate
        };
        machine.set_sample_rate(rate);

        RubcWasm {
            machine,
            rgba: vec![0u8; RGBA_LEN],
            samples: Vec::new(),
        }
    }

    /// Screen width in pixels (160).
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> usize {
        WIDTH
    }

    /// Screen height in pixels (144).
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> usize {
        HEIGHT
    }

    /// True if the loaded cartridge runs in Game Boy Color mode.
    #[wasm_bindgen(getter)]
    pub fn is_cgb(&self) -> bool {
        self.machine.model().is_cgb()
    }

    /// Advance the emulator until the next VBlank (one full rendered frame).
    pub fn step_frame(&mut self) {
        self.machine.step_frame();
    }

    /// Resolve the current PPU framebuffer into the internal RGBA buffer and
    /// return a pointer to its first byte.
    ///
    /// The buffer is exactly `width * height * 4` bytes (RGBA8888). Read it from
    /// JS with a typed-array view over the wasm memory, recreating the view each
    /// call because memory growth can detach the previous `ArrayBuffer`:
    /// ```js
    /// const ptr = emu.frame_rgba();
    /// const px  = new Uint8ClampedArray(wasm.memory.buffer, ptr, emu.frame_len);
    /// imageData.data.set(px);
    /// ```
    pub fn frame_rgba(&mut self) -> *const u8 {
        let fb = self.machine.framebuffer();
        for (out, &pixel) in self.rgba.chunks_exact_mut(4).zip(fb.iter()) {
            out.copy_from_slice(&frame_pixel_rgba(pixel));
        }
        self.rgba.as_ptr()
    }

    /// Length in bytes of the RGBA framebuffer returned by [`Self::frame_rgba`].
    #[wasm_bindgen(getter)]
    pub fn frame_len(&self) -> usize {
        RGBA_LEN
    }

    /// Copy the RGBA framebuffer into a fresh `Uint8Array` (a convenience
    /// alternative to the zero-copy [`Self::frame_rgba`] pointer path).
    pub fn frame_rgba_copy(&mut self) -> Vec<u8> {
        self.frame_rgba();
        self.rgba.clone()
    }

    /// Set a joypad button's pressed state. `button` is one of the codes
    /// documented on [`button_from_code`] (0=A, 1=B, 2=Select, 3=Start,
    /// 4=Right, 5=Left, 6=Up, 7=Down); out-of-range codes are ignored.
    pub fn set_button(&mut self, button: u8, pressed: bool) {
        if let Some(b) = button_from_code(button) {
            self.machine.set_button(b, pressed);
        }
    }

    /// Drain accumulated APU samples (interleaved stereo L/R `f32`) for the Web
    /// Audio API and return them as a `Float32Array`. Returns an empty array if
    /// no samples are queued.
    pub fn drain_audio(&mut self) -> Vec<f32> {
        self.samples.clear();
        self.machine.drain_samples(&mut self.samples);
        self.samples.clone()
    }

    /// True if the loaded cartridge has battery-backed RAM (i.e. a persistable
    /// `.sav`). When false, `save_ram` returns an empty array and there is
    /// nothing to persist.
    #[wasm_bindgen(getter)]
    pub fn has_battery(&self) -> bool {
        self.machine.has_battery()
    }

    /// Snapshot the cartridge's battery-backed RAM as a fresh `Uint8Array`,
    /// suitable for writing to browser storage (IndexedDB). Empty if the cart
    /// has no battery.
    pub fn save_ram(&self) -> Vec<u8> {
        self.machine.save_ram().to_vec()
    }

    /// Restore battery-backed RAM previously produced by [`Self::save_ram`].
    /// Sizes that don't match the cart's RAM are ignored by the core. Call this
    /// right after constructing the machine, before the first frame.
    pub fn load_ram(&mut self, data: &[u8]) {
        self.machine.load_ram(data);
    }

    pub fn save_state(&self) -> Vec<u8> {
        self.machine.save_state()
    }

    pub fn load_state(&mut self, data: &[u8]) -> bool {
        self.machine.load_state(data).is_ok()
    }
}
