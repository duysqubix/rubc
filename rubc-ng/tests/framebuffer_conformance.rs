use std::path::{Path, PathBuf};

use rubc_ng::{FramePixel, MachineNg, RunStopNg};

const FRAMEBUFFER_PIXELS: usize = 160 * 144;
const MAX_INSTRUCTIONS: u64 = 20_000_000;
const MAX_DMG_ACID2_DIFF: usize = 818;
const MAX_CGB_ACID2_DIFF: usize = 1090;

#[test]
fn machine_framebuffer_is_render_pipeline_output_not_tilemap_stub() {
    let rom = vec![0x00; 0x8000];
    let machine = MachineNg::boot_dmg(&rom).expect("valid DMG machine");

    assert_eq!(machine.framebuffer().len(), 160 * 144);
    assert!(matches!(machine.framebuffer()[0], FramePixel::DmgShade(_)));
}

#[test]
fn acid2_framebuffer_gate_uses_independent_reference() {
    let rom = suites_dir().join("acid2/dmg-acid2.gb");
    let reference = suites_dir().join("acid2/dmg-acid2-reference.bin");
    let (Ok(rom), Ok(reference)) = (std::fs::read(&rom), std::fs::read(&reference)) else {
        eprintln!("dmg-acid2 assets absent -- skipping");
        return;
    };

    let mut machine = MachineNg::boot_dmg(&rom).expect("valid dmg-acid2 ROM");
    assert_eq!(
        machine.run_mooneye(MAX_INSTRUCTIONS),
        RunStopNg::MooneyeBreakpoint
    );

    let diff = machine
        .framebuffer()
        .iter()
        .zip(reference.iter())
        .filter(|(actual, expected)| match actual {
            FramePixel::DmgShade(shade) => (*shade & 3) != (**expected & 3),
            FramePixel::CgbRgb555(_) => true,
        })
        .count();
    let first = machine
        .framebuffer()
        .iter()
        .zip(reference.iter())
        .enumerate()
        .find_map(|(i, (actual, expected))| match actual {
            FramePixel::DmgShade(shade) if (*shade & 3) != (*expected & 3) => {
                Some((i % 160, i / 160, *shade & 3, *expected & 3))
            }
            FramePixel::CgbRgb555(_) => Some((i % 160, i / 160, 0xFF, *expected & 3)),
            _ => None,
        });
    let bbox = machine
        .framebuffer()
        .iter()
        .zip(reference.iter())
        .enumerate()
        .filter_map(|(i, (actual, expected))| match actual {
            FramePixel::DmgShade(shade) if (*shade & 3) != (*expected & 3) => {
                Some((i % 160, i / 160))
            }
            FramePixel::CgbRgb555(_) => Some((i % 160, i / 160)),
            _ => None,
        })
        .fold(None, |bbox, (x, y)| match bbox {
            None => Some((x, x, y, y)),
            Some((min_x, max_x, min_y, max_y)) => {
                Some((min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y)))
            }
        });
    eprintln!("dmg-acid2 diff={diff}/23040 first={first:?} bbox={bbox:?}");
    let frame_dump = machine
        .framebuffer()
        .iter()
        .map(|pixel| match pixel {
            FramePixel::DmgShade(shade) => *shade & 3,
            FramePixel::CgbRgb555(_) => 0xFF,
        })
        .collect::<Vec<_>>();
    let _ = std::fs::write("/tmp/rubc-ng-dmg-acid2-frame.bin", &frame_dump);

    let mut corrupted = reference.clone();
    corrupted[0] ^= 0x03;
    let corrupted_diff = machine
        .framebuffer()
        .iter()
        .zip(corrupted.iter())
        .filter(|(actual, expected)| match actual {
            FramePixel::DmgShade(shade) => (*shade & 3) != (**expected & 3),
            FramePixel::CgbRgb555(_) => true,
        })
        .count();

    assert_ne!(
        diff, corrupted_diff,
        "corrupting the independent reference must change the measured diff"
    );
    assert_eq!(
        diff, MAX_DMG_ACID2_DIFF,
        "dmg-acid2 framebuffer diff changed; update only with an honest measured floor"
    );
}

fn suites_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
        .join("reference/test-suites")
}

fn load_reference_rgb555(rel: &str) -> Option<Vec<u16>> {
    let data = std::fs::read(suites_dir().join(rel)).ok()?;
    (data.len() == FRAMEBUFFER_PIXELS * 2).then(|| {
        data.chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]) & 0x7FFF)
            .collect()
    })
}

fn rgb555_diff(frame: &[FramePixel], reference: &[u16]) -> usize {
    frame
        .iter()
        .zip(reference)
        .filter(|(pixel, reference)| match pixel {
            FramePixel::CgbRgb555(rgb) => (*rgb & 0x7FFF) != **reference,
            FramePixel::DmgShade(_) => true,
        })
        .count()
}

#[test]
fn cgb_acid_framebuffer_diffs_are_reported_against_independent_references() {
    let Some(cgb_ref) = load_reference_rgb555("acid2/cgb-acid2-reference-rgb555.bin") else {
        eprintln!("cgb-acid2 RGB555 reference absent -- skipping");
        return;
    };
    let cgb_rom = match std::fs::read(suites_dir().join("acid2/cgb-acid2.gbc")) {
        Ok(rom) => rom,
        Err(_) => {
            eprintln!("cgb-acid2 ROM absent -- skipping");
            return;
        }
    };
    let mut cgb = MachineNg::boot_cgb_native(&cgb_rom).expect("valid cgb-acid2 ROM");
    assert_eq!(
        cgb.run_mooneye(MAX_INSTRUCTIONS),
        RunStopNg::MooneyeBreakpoint
    );
    let cgb_diff = rgb555_diff(cgb.framebuffer(), &cgb_ref);
    eprintln!("rubc-ng cgb-acid2 diff: {cgb_diff}/{FRAMEBUFFER_PIXELS}");
    assert_eq!(
        cgb_diff, MAX_CGB_ACID2_DIFF,
        "cgb-acid2 framebuffer diff changed; update only with an honest measured floor"
    );

    let Some(hell_ref) = load_reference_rgb555("cgb-acid-hell/cgb-acid-hell-reference-rgb555.bin")
    else {
        eprintln!("cgb-acid-hell RGB555 reference absent -- skipping");
        return;
    };
    let hell_rom = match std::fs::read(suites_dir().join("cgb-acid-hell/cgb-acid-hell.gbc")) {
        Ok(rom) => rom,
        Err(_) => {
            eprintln!("cgb-acid-hell ROM absent -- skipping");
            return;
        }
    };
    let mut hell = MachineNg::boot_cgb_native(&hell_rom).expect("valid cgb-acid-hell ROM");
    let stop = hell.run_mooneye(MAX_INSTRUCTIONS);
    if stop != RunStopNg::MooneyeBreakpoint {
        eprintln!("rubc-ng cgb-acid-hell did not reach LD B,B; stop={stop:?}");
        return;
    }
    let hell_diff = rgb555_diff(hell.framebuffer(), &hell_ref);
    eprintln!("rubc-ng cgb-acid-hell diff: {hell_diff}/{FRAMEBUFFER_PIXELS}");
}
