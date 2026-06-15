//! Headless screenshot + GIF capture for rubc.
//!
//! Boots a ROM with no window, steps the emulator for a number of frames, and
//! encodes the 160x144 PPU framebuffer to a PNG (single frame) or an animated
//! GIF (a sequence of frames). Both encoders are pure Rust:
//! - PNG via the `png` crate (DEFLATE through `fdeflate`/`miniz_oxide`),
//! - GIF via the `gif` crate (LZW through `weezl`).
//!
//! Nothing here participates in emulation; it only reads the resolved
//! framebuffer (see [`framebuffer_rgba`]) the same way the windowed frontend
//! does, so captures match what a player sees.

use rubc_ng::{FramePixel, MachineNg, RunStopNg};
use std::collections::HashMap;
use std::io::BufWriter;
use std::path::Path;

/// The 4 DMG shades (lightest -> darkest) as RGBA. Shared with the windowed
/// renderer so screenshots match the on-screen palette exactly.
pub const DMG_SHADES: [[u8; 4]; 4] = [
    [0xE0, 0xF8, 0xD0, 0xFF], // 0: lightest
    [0x88, 0xC0, 0x70, 0xFF], // 1
    [0x34, 0x68, 0x56, 0xFF], // 2
    [0x08, 0x18, 0x20, 0xFF], // 3: darkest
];

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;
const W: usize = SCREEN_WIDTH;
const H: usize = SCREEN_HEIGHT;

/// Map one resolved PPU pixel to RGBA. DMG pixels index [`DMG_SHADES`]; CGB
/// pixels expand each 5-bit RGB555 channel to 8-bit via `(x<<3)|(x>>2)` so full
/// intensity (31) maps to 255 (not 248).
#[inline]
pub fn frame_pixel_rgba(pixel: FramePixel) -> [u8; 4] {
    match pixel {
        FramePixel::DmgShade(shade) => DMG_SHADES[shade as usize],
        FramePixel::CgbRgb555(rgb) => {
            let expand = |c: u16| -> u8 {
                let c = (c & 0x1F) as u8;
                (c << 3) | (c >> 2)
            };
            [expand(rgb), expand(rgb >> 5), expand(rgb >> 10), 0xFF]
        }
    }
}

/// Resolve the current PPU framebuffer to a fresh RGBA buffer of exactly
/// `SCREEN_WIDTH * SCREEN_HEIGHT * 4` bytes. This is the single source of truth
/// for turning the PPU framebuffer into pixels; the windowed renderer maps the
/// same per-pixel function into its surface buffer.
pub fn framebuffer_rgba(machine: &MachineNg) -> Vec<u8> {
    let fb = machine.framebuffer();
    let mut out = vec![0u8; W * H * 4];
    for (px, &pixel) in out.chunks_exact_mut(4).zip(fb.iter()) {
        px.copy_from_slice(&frame_pixel_rgba(pixel));
    }
    out
}

/// Nearest-neighbour upscale of an RGBA image by an integer factor `k` (>=1).
/// Each source pixel becomes a `k x k` block. Returns the scaled buffer.
fn scale_rgba(src: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    if k <= 1 {
        return src.to_vec();
    }
    let dw = w * k;
    let mut out = vec![0u8; dw * h * k * 4];
    for y in 0..h {
        for x in 0..w {
            let s = (y * w + x) * 4;
            let px = &src[s..s + 4];
            for dy in 0..k {
                let row = (y * k + dy) * dw;
                for dx in 0..k {
                    let d = (row + x * k + dx) * 4;
                    out[d..d + 4].copy_from_slice(px);
                }
            }
        }
    }
    out
}

/// Nearest-neighbour upscale of an 8-bit indexed image by factor `k` (>=1).
fn scale_indexed(src: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    if k <= 1 {
        return src.to_vec();
    }
    let dw = w * k;
    let mut out = vec![0u8; dw * h * k];
    for y in 0..h {
        for x in 0..w {
            let v = src[y * w + x];
            for dy in 0..k {
                let row = (y * k + dy) * dw + x * k;
                out[row..row + k].fill(v);
            }
        }
    }
    out
}

/// Encode an RGBA image to a PNG file at `path`.
fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)
        .map_err(|e| anyhow::anyhow!("failed to create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}

/// Boot `machine` is the caller's job; here we step `frames` whole frames and
/// capture the final framebuffer to a PNG, optionally upscaled by `scale`.
pub fn capture_screenshot(
    machine: &mut MachineNg,
    out: &Path,
    frames: u32,
    scale: u32,
) -> anyhow::Result<()> {
    for _ in 0..frames {
        machine.step_frame();
    }
    let rgba = framebuffer_rgba(machine);
    let k = scale.max(1) as usize;
    let scaled = scale_rgba(&rgba, W, H, k);
    write_png(out, &scaled, (W * k) as u32, (H * k) as u32)?;
    Ok(())
}

/// Run until the ROM hits its `LD B,B` (`0x40`) completion breakpoint -- the
/// signal mooneye/acid test ROMs use to mark "image is final" -- then capture.
/// Acid tests (esp. cgb-acid-hell, which mutates LCDC every scanline) only
/// present their finished face at this breakpoint, never at a fixed frame count.
/// Falls back to capturing whatever is on screen if `max_instructions` elapses
/// without a breakpoint.
pub fn capture_screenshot_at_breakpoint(
    machine: &mut MachineNg,
    out: &Path,
    scale: u32,
    max_instructions: u64,
) -> anyhow::Result<bool> {
    let stop = machine.run_mooneye(max_instructions);
    let hit = matches!(stop, RunStopNg::MooneyeBreakpoint);
    let rgba = framebuffer_rgba(machine);
    let k = scale.max(1) as usize;
    let scaled = scale_rgba(&rgba, W, H, k);
    write_png(out, &scaled, (W * k) as u32, (H * k) as u32)?;
    Ok(hit)
}

/// A 256-colour (max) global palette plus the bit-shift used to quantize colours
/// into it. `shift == 0` means the palette is exact (the image had <=256 distinct
/// colours); a larger shift drops low bits per channel until the distinct count
/// fits, which is ample for Game Boy content (<=4 DMG shades or a modest CGB set).
struct Palette {
    /// Flat RGB triplets, length is a power-of-two * 3 (GIF colour-table rule).
    rgb: Vec<u8>,
    /// Index lookup keyed by the *quantized* (masked) RGB triplet.
    index: HashMap<[u8; 3], u8>,
    shift: u8,
}

impl Palette {
    /// Quantize one RGBA pixel to its palette index.
    #[inline]
    fn lookup(&self, px: &[u8]) -> u8 {
        let key = [
            (px[0] >> self.shift) << self.shift,
            (px[1] >> self.shift) << self.shift,
            (px[2] >> self.shift) << self.shift,
        ];
        *self.index.get(&key).unwrap_or(&0)
    }
}

/// Build a global palette across every captured RGBA frame. Tries an exact
/// palette first; if there are >256 distinct colours, drops one low bit per
/// channel at a time until the set fits in 256. Deterministic: colours are
/// sorted before indices are assigned.
fn build_palette(frames: &[Vec<u8>]) -> Palette {
    for shift in 0u8..=8 {
        let mut set: std::collections::BTreeSet<[u8; 3]> = std::collections::BTreeSet::new();
        for frame in frames {
            for px in frame.chunks_exact(4) {
                set.insert([
                    (px[0] >> shift) << shift,
                    (px[1] >> shift) << shift,
                    (px[2] >> shift) << shift,
                ]);
            }
        }
        if set.len() <= 256 {
            let colors: Vec<[u8; 3]> = set.into_iter().collect();
            let mut index = HashMap::with_capacity(colors.len());
            let mut rgb = Vec::with_capacity(colors.len() * 3);
            for (i, c) in colors.iter().enumerate() {
                index.insert(*c, i as u8);
                rgb.extend_from_slice(c);
            }
            // GIF colour tables must be a power-of-two count of entries.
            let mut entries = colors.len().max(1);
            let mut pow = 1usize;
            while pow < entries {
                pow <<= 1;
            }
            entries = pow;
            rgb.resize(entries * 3, 0);
            return Palette { rgb, index, shift };
        }
    }
    // Unreachable for real images: shift==8 collapses everything to one colour.
    Palette {
        rgb: vec![0, 0, 0],
        index: HashMap::new(),
        shift: 8,
    }
}

/// GIF inter-frame delay (centiseconds) for capturing every `every` GB frames.
/// One GB frame is ~16.742 ms; clamp to >=2 cs so viewers don't run flat-out.
fn gif_delay_cs(every: u32) -> u16 {
    let ms = every.max(1) as f64 * 16.742_f64;
    ((ms / 10.0).round() as u16).max(2)
}

/// Record an animated GIF: skip `skip` warm-up frames (e.g. boot logos), then
/// step the emulator, capturing one frame every `every` steps until `frames`
/// frames are collected, then encode (looping forever), upscaled by `scale`.
pub fn capture_gif(
    machine: &mut MachineNg,
    out: &Path,
    frames: u32,
    every: u32,
    scale: u32,
    skip: u32,
) -> anyhow::Result<()> {
    let every = every.max(1);
    // Warm-up: advance past boot logos / static lead-in without capturing.
    for _ in 0..skip {
        machine.step_frame();
    }
    let mut shots: Vec<Vec<u8>> = Vec::with_capacity(frames as usize);
    for _ in 0..frames {
        for _ in 0..every {
            machine.step_frame();
        }
        shots.push(framebuffer_rgba(machine));
    }
    if shots.is_empty() {
        anyhow::bail!("gif capture requested 0 frames");
    }

    let palette = build_palette(&shots);
    let k = scale.max(1) as usize;
    let (dw, dh) = ((W * k) as u16, (H * k) as u16);
    let delay = gif_delay_cs(every);

    let file = std::fs::File::create(out)
        .map_err(|e| anyhow::anyhow!("failed to create {}: {e}", out.display()))?;
    let mut encoder = gif::Encoder::new(BufWriter::new(file), dw, dh, &palette.rgb)?;
    encoder.set_repeat(gif::Repeat::Infinite)?;

    for shot in &shots {
        // RGBA -> palette indices at native resolution, then upscale the index
        // buffer (nearest-neighbour on indices == nearest-neighbour on colour).
        let mut indices = vec![0u8; W * H];
        for (dst, px) in indices.iter_mut().zip(shot.chunks_exact(4)) {
            *dst = palette.lookup(px);
        }
        let scaled = scale_indexed(&indices, W, H, k);
        let frame = gif::Frame {
            width: dw,
            height: dh,
            delay,
            buffer: std::borrow::Cow::Owned(scaled),
            ..Default::default()
        };
        encoder.write_frame(&frame)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gif_delay_floor_and_scaling() {
        // every=1 -> ~17ms -> 2cs; every=2 -> ~33ms -> 3cs.
        assert_eq!(gif_delay_cs(1), 2);
        assert_eq!(gif_delay_cs(2), 3);
        assert_eq!(gif_delay_cs(0), 2); // clamped
    }

    #[test]
    fn scale_rgba_blocks_each_pixel() {
        // 1x1 red image, scale 2 -> 2x2 all red.
        let src = [255u8, 0, 0, 255];
        let out = scale_rgba(&src, 1, 1, 2);
        assert_eq!(out.len(), 2 * 2 * 4);
        for px in out.chunks_exact(4) {
            assert_eq!(px, &[255, 0, 0, 255]);
        }
    }

    #[test]
    fn palette_exact_for_few_colors() {
        // Two distinct colours -> exact palette (shift 0), padded to power of two.
        let frame: Vec<u8> = [[0u8, 0, 0, 255], [255, 255, 255, 255]]
            .iter()
            .flat_map(|p| p.iter().copied())
            .collect();
        let pal = build_palette(&[frame]);
        assert_eq!(pal.shift, 0);
        assert!(pal.index.len() == 2);
        // Power-of-two entries: 2 colours -> 2 entries -> 6 bytes.
        assert_eq!(pal.rgb.len(), 2 * 3);
    }

    /// Path to a reference ROM under the (git-ignored) `reference/` symlink at
    /// the workspace root, relative to this crate's manifest dir.
    fn ref_rom(rel: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(rel)
    }

    #[test]
    fn framebuffer_rgba_is_exactly_screen_sized() {
        // A blank DMG machine still yields a full-size RGBA buffer.
        let machine = MachineNg::boot_dmg(&[0u8; 0x8000]).expect("ROM boots");
        let rgba = framebuffer_rgba(&machine);
        assert_eq!(rgba.len(), W * H * 4);
    }

    #[test]
    fn screenshot_of_dmg_acid2_is_non_uniform() {
        // dmg-acid2 renders the acid2 face: a real, multi-colour image. Boot it,
        // capture a PNG, decode it back, and assert it is valid 160x144 with more
        // than one distinct colour (i.e. not a blank/solid frame).
        let rom_path = ref_rom("reference/test-suites/acid2/dmg-acid2.gb");
        if !rom_path.exists() {
            eprintln!("skipping: {} not present", rom_path.display());
            return;
        }
        let rom = std::fs::read(&rom_path).expect("read dmg-acid2.gb");
        let mut machine = MachineNg::boot_dmg(&rom).expect("ROM boots");
        let out = std::env::temp_dir().join("rubc-test-dmg-acid2.png");
        capture_screenshot(&mut machine, &out, 120, 1).expect("capture screenshot");

        let meta = std::fs::metadata(&out).expect("png exists");
        assert!(meta.len() > 0, "png is empty");

        // Decode the PNG back and count distinct RGBA colours.
        let decoder = png::Decoder::new(std::fs::File::open(&out).expect("open png"));
        let mut reader = decoder.read_info().expect("png header");
        let info = reader.info();
        assert_eq!(info.width, W as u32);
        assert_eq!(info.height, H as u32);
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut buf).expect("png frame");
        let bytes = &buf[..frame.buffer_size()];
        let distinct: std::collections::BTreeSet<&[u8]> = bytes.chunks_exact(4).collect();
        assert!(
            distinct.len() > 1,
            "expected a non-uniform image, got {} colour(s)",
            distinct.len()
        );
        let _ = std::fs::remove_file(&out);
    }
}
