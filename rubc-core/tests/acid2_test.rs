//! Visual PPU test harness for the acid2 + mealybug-tearoom suites.
//!
//! These are *rendering* tests: a ROM draws a frame, halts at the `LD B,B`
//! breakpoint, and the resulting screen is compared pixel-for-pixel against a
//! reference image. We pre-convert the reference PNGs to raw shade-index `.bin`
//! files (one byte per pixel, 0..=3, light->dark) at vendor time, so this
//! harness needs no PNG decoder -- just `std::fs` and a framebuffer diff.
//!
//! ## What is gated vs. reported
//! - **dmg-acid2** is a hard gate: it is a line-based renderer test (no
//!   mid-mode-3 writes, no CGB palette), which our PPU FIFO can render exactly.
//!   The DMG palette at the breakpoint is the identity BGP=0xE4, so our raw
//!   2-bit framebuffer indices map 1:1 onto the reference shade indices.
//! - **mealybug-tearoom** remains partly reporting, partly gated: it prints every
//!   ROM diff and asserts the current minimum pixel-exact count so mid-mode-3
//!   timing work cannot regress silently.
//! - **cgb-acid2** is a hard RGB555 gate; **cgb-acid-hell** is gated only against
//!   the current tiny residual until the last CGB mid-line race is resolved.
//!
//! Skips cleanly when the (git-ignored) reference material is absent, so a
//! fresh checkout still passes `cargo test`.

use rubc_core::bus::ppu::{FramePixel, FRAMEBUFFER_PIXELS};
use rubc_core::machine::{Machine, RunStop};
use std::path::{Path, PathBuf};

const MAX_INSTRUCTIONS: u64 = 20_000_000;
const MIN_MEALYBUG_DMG_EXACT: usize = 2;
const MAX_MEALYBUG_M3_BGP_CHANGE_DIFF: usize = 820;
const MAX_MEALYBUG_M3_LCDC_WIN_EN_CHANGE_MULTIPLE_DIFF: usize = 0;
const MAX_MEALYBUG_M3_LCDC_WIN_EN_CHANGE_MULTIPLE_WX_DIFF: usize = 952;
const MAX_MEALYBUG_M3_WINDOW_TIMING_DIFF: usize = 103;
const MAX_MEALYBUG_M3_WINDOW_TIMING_WX_0_DIFF: usize = 1346;
const MAX_MEALYBUG_M3_SCY_CHANGE_DIFF: usize = 8819;
const MAX_MEALYBUG_M3_WX_4_CHANGE_DIFF: usize = 3077;
const MAX_MEALYBUG_M3_WX_5_CHANGE_DIFF: usize = 3267;
const MAX_MEALYBUG_M3_WX_6_CHANGE_DIFF: usize = 13018;
// Boot-ROM setup-state gains (commit 937294d): real DMG boot ROM gives these
// setup-sensitive ROMs the hardware-correct PPU phase. Gated to lock the gains.
const MAX_MEALYBUG_M3_BGP_CHANGE_SPRITES_DIFF: usize = 2124;
const MAX_MEALYBUG_M3_OBP0_CHANGE_DIFF: usize = 221;
const MAX_MEALYBUG_M3_SCX_LOW_3_BITS_DIFF: usize = 396;
const MAX_MEALYBUG_M3_LCDC_TILE_SEL_CHANGE_DIFF: usize = 1755;
const MAX_MEALYBUG_M3_LCDC_TILE_SEL_WIN_CHANGE_DIFF: usize = 2755;
const MAX_MEALYBUG_M3_LCDC_WIN_MAP_CHANGE_DIFF: usize = 1925;
const MAX_MEALYBUG_M3_LCDC_BG_MAP_CHANGE_DIFF: usize = 845;
const MAX_CGB_ACID_HELL_DIFF: usize = 0;
const MIN_MEALYBUG_CGBC_EXACT: usize = 2;
const MEALYBUG_CGBC_PLACEHOLDER_FNV1A64: u64 = 0x9ac7_cebb_9006_6345;

const MEALYBUG_CGBC_BASELINES: &[(&str, usize)] = &[
    ("m2_win_en_toggle", 0),
    ("m3_bgp_change", 1028),
    ("m3_bgp_change_sprites", 2736),
    ("m3_lcdc_bg_en_change", 2548),
    ("m3_lcdc_bg_en_change2", 666),
    ("m3_lcdc_bg_map_change", 714),
    ("m3_lcdc_bg_map_change2", 545),
    ("m3_lcdc_obj_en_change", 206),
    ("m3_lcdc_obj_en_change_variant", 626),
    ("m3_lcdc_obj_size_change", 155),
    ("m3_lcdc_obj_size_change_scx", 140),
    // CGB-C re-baseline notes (2026-06-10): the old values for
    // tile_sel_change (1358), tile_sel_win_change (1204), and win_map_change
    // (1102) were measured under native-CGB boot, which is physically wrong for
    // these DMG-flagged carts. Their real CGB-C captures are compat-mode frames,
    // so the old diffs included palette noise and were not comparable. The pins
    // below are the first honest compat measurements. The remaining gap is the
    // LCDC write-conflict silicon split: SameBoy models DMG and CGB LCDC conflict
    // maps separately (GB_CONFLICT_DMG_LCDC vs GB_CONFLICT_LCDC_CGB), while rubc
    // still has one unified T3 write landing tuned to both DMG mealybug pins and
    // native cgb-acid-hell 0px. This is a measurement-regime change, not gate
    // weakening: 24 sibling CGB-C pins drop under the same compat regime.
    ("m3_lcdc_tile_sel_change", 1840),
    ("m3_lcdc_tile_sel_change2", 1391),
    ("m3_lcdc_tile_sel_win_change", 2476),
    ("m3_lcdc_tile_sel_win_change2", 2442),
    ("m3_lcdc_win_en_change_multiple", 0),
    ("m3_lcdc_win_map_change", 2126),
    ("m3_lcdc_win_map_change2", 560),
    ("m3_obp0_change", 432),
    ("m3_scx_high_5_bits", 19),
    ("m3_scx_high_5_bits_change2", 170),
    ("m3_scx_low_3_bits", 540),
    ("m3_scy_change", 8450),
    ("m3_scy_change2", 356),
    ("m3_window_timing", 225),
    ("m3_window_timing_wx_0", 1202),
    ("m3_wx_4_change_sprites", 111),
];

fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn is_mealybug_cgbc_placeholder_reference(data: &[u8]) -> bool {
    // The mealybug repository ships this repeated "Expected result not currently
    // available" placeholder where no CGB-C hardware capture exists yet.
    data.len() == FRAMEBUFFER_PIXELS * 2 && fnv1a64(data) == MEALYBUG_CGBC_PLACEHOLDER_FNV1A64
}

fn suites_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/test-suites")
}

/// Run a ROM to the `LD B,B` breakpoint and return its framebuffer as DMG shade
/// indices (0..=3), one byte per pixel. The PPU stores resolved `FramePixel`s;
/// we extract the DMG shade for comparison against the reference `.bin`. Returns
/// None if the ROM is absent or never reached the breakpoint.
fn render(rom_rel: &str, cgb: bool) -> Option<Vec<u8>> {
    let path = suites_dir().join(rom_rel);
    let rom = std::fs::read(&path).ok()?;
    let mut m = if cgb {
        Machine::boot_cgb(&rom)
    } else {
        Machine::boot_dmg(&rom)
    };
    match m.run_mooneye(MAX_INSTRUCTIONS) {
        RunStop::MooneyeBreakpoint => Some(
            m.bus
                .ppu
                .framebuffer
                .iter()
                .map(|p| match p {
                    FramePixel::DmgShade(s) => *s,
                    FramePixel::CgbRgb555(_) => 0,
                })
                .collect(),
        ),
        _ => None,
    }
}

fn render_dmg_with_bootrom(rom_rel: &str) -> Option<Vec<u8>> {
    let path = suites_dir().join(rom_rel);
    let rom = std::fs::read(&path).ok()?;
    let mut m = Machine::boot_dmg_with_bootrom(&rom);
    match m.run_mooneye(MAX_INSTRUCTIONS) {
        RunStop::MooneyeBreakpoint => Some(
            m.bus
                .ppu
                .framebuffer
                .iter()
                .map(|p| match p {
                    FramePixel::DmgShade(s) => *s,
                    FramePixel::CgbRgb555(_) => 0,
                })
                .collect(),
        ),
        _ => None,
    }
}

/// Run a CGB ROM to the `LD B,B` breakpoint and return its framebuffer as
/// RGB555 values (one u16 per pixel). The PPU emits resolved `CgbRgb555` pixels
/// in CGB mode; we compare these against the hardware-native RGB555 reference
/// (color-curve independent). Returns None if absent or no breakpoint.
fn render_cgb(rom_rel: &str) -> Option<Vec<u16>> {
    let path = suites_dir().join(rom_rel);
    let rom = std::fs::read(&path).ok()?;
    let mut m = Machine::boot_cgb(&rom);
    match m.run_mooneye(MAX_INSTRUCTIONS) {
        RunStop::MooneyeBreakpoint => Some(
            m.bus
                .ppu
                .framebuffer
                .iter()
                .map(|p| match p {
                    FramePixel::CgbRgb555(rgb) => *rgb,
                    FramePixel::DmgShade(s) => *s as u16,
                })
                .collect(),
        ),
        _ => None,
    }
}

/// Load an RGB555 reference (.bin, two little-endian bytes/pixel) if present.
fn load_reference_rgb555(rel: &str) -> Option<Vec<u16>> {
    let path = suites_dir().join(rel);
    let data = std::fs::read(&path).ok()?;
    if data.len() != FRAMEBUFFER_PIXELS * 2 {
        return None;
    }
    Some(
        data.chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]) & 0x7FFF)
            .collect(),
    )
}

/// Load a raw shade-index reference (.bin, one byte/pixel) if present.
fn load_reference(rel: &str) -> Option<Vec<u8>> {
    let path = suites_dir().join(rel);
    let data = std::fs::read(&path).ok()?;
    (data.len() == FRAMEBUFFER_PIXELS).then_some(data)
}

fn pixel_diff(frame: &[u8], reference: &[u8]) -> usize {
    frame
        .iter()
        .zip(reference.iter())
        .filter(|(a, b)| (**a & 3) != (**b & 3))
        .count()
}

#[test]
fn dmg_acid2_renders_exactly() {
    let Some(frame) = render("acid2/dmg-acid2.gb", false) else {
        eprintln!(
            "dmg-acid2: ROM absent or no breakpoint -- skipping (run with reference/ present)"
        );
        return;
    };
    let Some(reference) = load_reference("acid2/dmg-acid2-reference.bin") else {
        eprintln!("dmg-acid2: reference .bin absent -- skipping");
        return;
    };
    let diff = pixel_diff(&frame, &reference);
    println!("dmg-acid2: {diff} / {FRAMEBUFFER_PIXELS} pixels differ");
    assert_eq!(
        diff, 0,
        "dmg-acid2 must render pixel-exact ({diff} pixels differ)"
    );
}

/// Count RGB555 pixels that differ between a rendered frame and a reference.
fn pixel_diff_rgb555(frame: &[u16], reference: &[u16]) -> usize {
    frame
        .iter()
        .zip(reference.iter())
        .filter(|(a, b)| a != b)
        .count()
}

/// cgb-acid2 must render pixel-exact in RGB555. We compare against the
/// hardware-native RGB555 reference (reverse-mapped from the reference PNG), so
/// the result is independent of any display color-correction curve.
#[test]
fn cgb_acid2_renders_exactly() {
    let Some(frame) = render_cgb("acid2/cgb-acid2.gbc") else {
        eprintln!("cgb-acid2: ROM absent or no breakpoint -- skipping");
        return;
    };
    let Some(reference) = load_reference_rgb555("acid2/cgb-acid2-reference-rgb555.bin") else {
        eprintln!("cgb-acid2: RGB555 reference absent -- skipping");
        return;
    };
    let diff = pixel_diff_rgb555(&frame, &reference);
    println!("cgb-acid2: {diff} / {FRAMEBUFFER_PIXELS} RGB555 pixels differ");
    assert_eq!(
        diff, 0,
        "cgb-acid2 must render pixel-exact ({diff} pixels differ)"
    );
}

/// Reporting harness for the mealybug-tearoom suite. It prints every per-ROM
/// DMG diff and gates the current minimum exact-pass count.
#[test]
fn mealybug_report() {
    let raw_dir = suites_dir().join("mealybug/expected/DMG-raw");
    let Ok(entries) = std::fs::read_dir(&raw_dir) else {
        eprintln!("mealybug: expected refs absent -- skipping");
        return;
    };
    let mut refs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("bin"))
        .collect();
    refs.sort();

    let mut total = 0usize;
    let mut exact = 0usize;
    for ref_path in &refs {
        let name = ref_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let Some(reference) = std::fs::read(ref_path)
            .ok()
            .filter(|d| d.len() == FRAMEBUFFER_PIXELS)
        else {
            continue;
        };
        let Some(frame) = render_dmg_with_bootrom(&format!("mealybug/{name}.gb")) else {
            println!("mealybug {name}: no breakpoint");
            continue;
        };
        total += 1;
        let diff = pixel_diff(&frame, &reference);
        if diff == 0 {
            exact += 1;
        }
        println!("mealybug {name}: {diff} px differ");
        if name == "m3_bgp_change" {
            assert!(
                diff <= MAX_MEALYBUG_M3_BGP_CHANGE_DIFF,
                "mealybug m3_bgp_change must not regress past {MAX_MEALYBUG_M3_BGP_CHANGE_DIFF} pixels ({diff} differ)"
            );
        }
        match name {
            "m3_lcdc_win_en_change_multiple" => assert_eq!(
                diff,
                MAX_MEALYBUG_M3_LCDC_WIN_EN_CHANGE_MULTIPLE_DIFF,
                "mealybug {name} must stay pixel-exact ({diff} differ)"
            ),
            "m3_lcdc_win_en_change_multiple_wx" => assert!(
                diff <= MAX_MEALYBUG_M3_LCDC_WIN_EN_CHANGE_MULTIPLE_WX_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_LCDC_WIN_EN_CHANGE_MULTIPLE_WX_DIFF} pixels ({diff} differ)"
            ),
            "m3_window_timing" => assert!(
                diff <= MAX_MEALYBUG_M3_WINDOW_TIMING_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_WINDOW_TIMING_DIFF} pixels ({diff} differ)"
            ),
            "m3_window_timing_wx_0" => assert!(
                diff <= MAX_MEALYBUG_M3_WINDOW_TIMING_WX_0_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_WINDOW_TIMING_WX_0_DIFF} pixels ({diff} differ)"
            ),
            "m3_scy_change" => assert!(
                diff <= MAX_MEALYBUG_M3_SCY_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_SCY_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            "m3_wx_4_change" => assert!(
                diff <= MAX_MEALYBUG_M3_WX_4_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_WX_4_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            "m3_wx_5_change" => assert!(
                diff <= MAX_MEALYBUG_M3_WX_5_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_WX_5_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            "m3_wx_6_change" => assert!(
                diff <= MAX_MEALYBUG_M3_WX_6_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_WX_6_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            "m3_bgp_change_sprites" => assert!(
                diff <= MAX_MEALYBUG_M3_BGP_CHANGE_SPRITES_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_BGP_CHANGE_SPRITES_DIFF} pixels ({diff} differ)"
            ),
            "m3_obp0_change" => assert!(
                diff <= MAX_MEALYBUG_M3_OBP0_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_OBP0_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            "m3_scx_low_3_bits" => assert!(
                diff <= MAX_MEALYBUG_M3_SCX_LOW_3_BITS_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_SCX_LOW_3_BITS_DIFF} pixels ({diff} differ)"
            ),
            "m3_lcdc_tile_sel_change" => assert!(
                diff <= MAX_MEALYBUG_M3_LCDC_TILE_SEL_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_LCDC_TILE_SEL_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            "m3_lcdc_tile_sel_win_change" => assert!(
                diff <= MAX_MEALYBUG_M3_LCDC_TILE_SEL_WIN_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_LCDC_TILE_SEL_WIN_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            "m3_lcdc_win_map_change" => assert!(
                diff <= MAX_MEALYBUG_M3_LCDC_WIN_MAP_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_LCDC_WIN_MAP_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            "m3_lcdc_bg_map_change" => assert!(
                diff <= MAX_MEALYBUG_M3_LCDC_BG_MAP_CHANGE_DIFF,
                "mealybug {name} must stay <= {MAX_MEALYBUG_M3_LCDC_BG_MAP_CHANGE_DIFF} pixels ({diff} differ)"
            ),
            _ => {}
        }
    }
    println!("----");
    println!(
        "mealybug DMG: {exact}/{total} pixel-exact \
         (mid-mode-3 timing needs sub-M-cycle bus -- rubc-7ks)"
    );
    assert!(
        exact >= MIN_MEALYBUG_DMG_EXACT,
        "mealybug DMG must keep at least {MIN_MEALYBUG_DMG_EXACT}/{total} pixel-exact ROMs (got {exact})"
    );
}

#[test]
fn cgb_mealybug_report() {
    let raw_dir = suites_dir().join("mealybug/expected/CGB-C-rgb555");
    let Ok(entries) = std::fs::read_dir(&raw_dir) else {
        eprintln!("mealybug CGB-C: RGB555 refs absent -- skipping");
        return;
    };
    let mut refs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("bin"))
        .collect();
    refs.sort();

    let mut total = 0usize;
    let mut exact = 0usize;
    for ref_path in &refs {
        let name = ref_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let Some(reference_bytes) = std::fs::read(ref_path)
            .ok()
            .filter(|d| d.len() == FRAMEBUFFER_PIXELS * 2)
        else {
            continue;
        };
        if is_mealybug_cgbc_placeholder_reference(&reference_bytes) {
            println!(
                "mealybug CGB-C {name}: reference is the mealybug placeholder image -- skipped (no hardware capture)"
            );
            continue;
        }
        let reference = reference_bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]) & 0x7FFF)
            .collect::<Vec<_>>();
        let Some(frame) = render_cgb(&format!("mealybug/{name}.gb")) else {
            println!("mealybug CGB-C {name}: no breakpoint");
            continue;
        };
        let Some((_, max_diff)) = MEALYBUG_CGBC_BASELINES
            .iter()
            .find(|(baseline_name, _)| *baseline_name == name)
        else {
            panic!("mealybug CGB-C {name}: missing baseline gate");
        };
        total += 1;
        let diff = pixel_diff_rgb555(&frame, &reference);
        if diff == 0 {
            exact += 1;
        }
        println!("mealybug CGB-C {name}: {diff} px differ");
        assert!(
            diff <= *max_diff,
            "mealybug CGB-C {name} must stay <= {max_diff} pixels ({diff} differ)"
        );
    }
    println!("----");
    println!("mealybug CGB-C: {exact}/{total} pixel-exact");
    assert!(
        exact >= MIN_MEALYBUG_CGBC_EXACT,
        "mealybug CGB-C must keep at least {MIN_MEALYBUG_CGBC_EXACT}/{total} measurable pixel-exact ROMs (got {exact})"
    );
}

#[test]
fn cgb_mealybug_placeholder_references_are_only_upstream_missing_captures() {
    let raw_dir = suites_dir().join("mealybug/expected/CGB-C-rgb555");
    let Ok(entries) = std::fs::read_dir(&raw_dir) else {
        eprintln!("mealybug CGB-C: RGB555 refs absent -- skipping placeholder guard");
        return;
    };
    let mut placeholders = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("bin"))
        .filter_map(|path| {
            let data = std::fs::read(&path).ok()?;
            is_mealybug_cgbc_placeholder_reference(&data).then(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_owned()
            })
        })
        .collect::<Vec<_>>();
    placeholders.sort();
    assert_eq!(
        placeholders,
        [
            "m3_lcdc_win_en_change_multiple_wx",
            "m3_wx_4_change",
            "m3_wx_5_change",
            "m3_wx_6_change"
        ],
        "CGB-C placeholder detection must re-enable automatically when upstream hardware captures replace the mealybug placeholder"
    );
}

/// cgb-acid-hell is an extremely demanding CGB PPU test (mid-scanline
/// LCDC/palette/VRAM-bank changes). Keep the current residual gated so it cannot
/// grow while rubc-bqi remains open.
#[test]
fn cgb_acid_hell_report() {
    let Some(frame) = render_cgb("cgb-acid-hell/cgb-acid-hell.gbc") else {
        eprintln!("cgb-acid-hell: ROM absent or no breakpoint -- skipping");
        return;
    };
    let Some(reference) = load_reference_rgb555("cgb-acid-hell/cgb-acid-hell-reference-rgb555.bin")
    else {
        eprintln!("cgb-acid-hell: RGB555 reference absent -- skipping");
        return;
    };
    let diff = pixel_diff_rgb555(&frame, &reference);
    println!("----");
    println!(
        "cgb-acid-hell: {diff} / {FRAMEBUFFER_PIXELS} RGB555 pixels differ \
         (mid-mode-3 CGB timing -- rubc-1cu)"
    );
    assert_eq!(
        diff, MAX_CGB_ACID_HELL_DIFF,
        "cgb-acid-hell must remain pixel-exact ({diff} RGB555 pixels differ)"
    );
}
