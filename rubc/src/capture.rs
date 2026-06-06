//! Headless screenshot capture for rubc.
//!
//! Boots a ROM with no window, steps the emulator for a number of frames, and
//! encodes the 160x144 PPU framebuffer to a PNG via the pure-Rust `png` crate
//! (DEFLATE through `fdeflate`/`miniz_oxide`).
//!
//! Nothing here participates in emulation; it only reads the resolved
//! framebuffer (see [`framebuffer_rgba`]) the same way the windowed frontend
//! does, so captures match what a player sees.

use rubc_core::bus::ppu::{FramePixel, SCREEN_HEIGHT, SCREEN_WIDTH};
use rubc_core::machine::Machine;
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
pub fn framebuffer_rgba(machine: &Machine) -> Vec<u8> {
    let fb = &machine.bus.ppu.framebuffer;
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

/// Booting `machine` is the caller's job; here we step `frames` whole frames and
/// capture the final framebuffer to a PNG, optionally upscaled by `scale`.
pub fn capture_screenshot(
    machine: &mut Machine,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let machine = Machine::boot_dmg(&[0u8; 0x8000]);
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
        let mut machine = Machine::boot_dmg(&rom);
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
