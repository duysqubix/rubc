use std::path::{Path, PathBuf};

use rubc_core::bus::ppu::FramePixel as OldFramePixel;
use rubc_core::machine::{Machine as OldMachine, RunStop as OldRunStop};
use rubc_ng::{FramePixel as NewFramePixel, MachineNg, RunStopNg};

const BLARGG_MAX_INSTRUCTIONS: u64 = 120_000_000;
const MOONEYE_MAX_INSTRUCTIONS: u64 = 20_000_000;
const RESULT_SCREEN_SETTLE_FRAMES: usize = 3;
const FRAMEBUFFER_PIXELS: usize = 160 * 144;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommonPixel {
    DmgShade(u8),
    CgbRgb555(u16),
}

struct OldRun {
    stop: OldRunStop,
    serial: String,
    framebuffer: Vec<CommonPixel>,
}

struct NewRun {
    stop: RunStopNg,
    serial: String,
    framebuffer: Vec<CommonPixel>,
}

#[test]
fn blargg_cpu_instrs_matches_old_core_serial_and_result_screen() {
    let rom = load_rom("gb-test-roms/cpu_instrs/cpu_instrs.gb");
    let old = run_old_blargg(&rom);
    let new = run_new_blargg(&rom);

    assert_blargg_passed("cpu_instrs", &old, &new);
    assert_framebuffers_match(
        "cpu_instrs result screen",
        &old.framebuffer,
        &new.framebuffer,
    );
}

#[test]
fn blargg_instr_timing_matches_old_core_serial_and_result_screen() {
    let rom = load_rom("gb-test-roms/instr_timing/instr_timing.gb");
    let old = run_old_blargg(&rom);
    let new = run_new_blargg(&rom);

    assert_blargg_passed("instr_timing", &old, &new);
    assert_framebuffers_match(
        "instr_timing result screen",
        &old.framebuffer,
        &new.framebuffer,
    );
}

#[test]
fn dmg_acid2_matches_old_core_framebuffer_at_breakpoint() {
    let rom = load_rom("acid2/dmg-acid2.gb");
    let old = run_old_mooneye(&rom);
    let new = run_new_mooneye(&rom);

    assert_eq!(
        old.stop,
        OldRunStop::MooneyeBreakpoint,
        "old dmg-acid2 stop"
    );
    assert_eq!(new.stop, RunStopNg::MooneyeBreakpoint, "new dmg-acid2 stop");
    assert_framebuffers_match("dmg-acid2", &old.framebuffer, &new.framebuffer);
}

#[test]
fn cgb_acid2_matches_old_core_framebuffer_at_breakpoint() {
    let rom = load_rom("acid2/cgb-acid2.gbc");
    let old = run_old_mooneye(&rom);
    let new = run_new_mooneye(&rom);

    assert_eq!(
        old.stop,
        OldRunStop::MooneyeBreakpoint,
        "old cgb-acid2 stop"
    );
    assert_eq!(new.stop, RunStopNg::MooneyeBreakpoint, "new cgb-acid2 stop");
    assert_framebuffers_match("cgb-acid2", &old.framebuffer, &new.framebuffer);
}

fn run_old_blargg(rom: &[u8]) -> OldRun {
    let mut machine = boot_old_by_header(rom);
    let stop = machine.run_blargg(BLARGG_MAX_INSTRUCTIONS);
    for _ in 0..RESULT_SCREEN_SETTLE_FRAMES {
        machine.step_frame();
    }
    OldRun {
        stop,
        serial: machine.serial_text().unwrap_or_default(),
        framebuffer: old_framebuffer(&machine),
    }
}

fn run_new_blargg(rom: &[u8]) -> NewRun {
    let mut machine = boot_new_by_header(rom);
    let stop = machine.run_blargg(BLARGG_MAX_INSTRUCTIONS);
    for _ in 0..RESULT_SCREEN_SETTLE_FRAMES {
        machine.step_frame();
    }
    NewRun {
        stop,
        serial: machine.serial_output().to_owned(),
        framebuffer: new_framebuffer(&machine),
    }
}

fn run_old_mooneye(rom: &[u8]) -> OldRun {
    let mut machine = boot_old_by_header(rom);
    let stop = machine.run_mooneye(MOONEYE_MAX_INSTRUCTIONS);
    OldRun {
        stop,
        serial: machine.serial_text().unwrap_or_default(),
        framebuffer: old_framebuffer(&machine),
    }
}

fn run_new_mooneye(rom: &[u8]) -> NewRun {
    let mut machine = boot_new_by_header(rom);
    let stop = machine.run_mooneye(MOONEYE_MAX_INSTRUCTIONS);
    NewRun {
        stop,
        serial: machine.serial_output().to_owned(),
        framebuffer: new_framebuffer(&machine),
    }
}

fn boot_old_by_header(rom: &[u8]) -> OldMachine {
    if is_cgb_rom(rom) {
        OldMachine::boot_cgb(rom)
    } else {
        OldMachine::boot_dmg(rom)
    }
}

fn boot_new_by_header(rom: &[u8]) -> MachineNg {
    if is_cgb_rom(rom) {
        MachineNg::boot_cgb(rom).expect("CGB ROM boots in rubc-ng")
    } else {
        MachineNg::boot_dmg(rom).expect("DMG ROM boots in rubc-ng")
    }
}

fn is_cgb_rom(rom: &[u8]) -> bool {
    rom.get(0x0143).is_some_and(|flag| flag & 0x80 != 0)
}

fn old_framebuffer(machine: &OldMachine) -> Vec<CommonPixel> {
    machine
        .bus
        .ppu
        .framebuffer
        .iter()
        .map(|pixel| match pixel {
            OldFramePixel::DmgShade(shade) => CommonPixel::DmgShade(*shade & 0x03),
            OldFramePixel::CgbRgb555(rgb) => CommonPixel::CgbRgb555(*rgb & 0x7FFF),
        })
        .collect()
}

fn new_framebuffer(machine: &MachineNg) -> Vec<CommonPixel> {
    machine
        .framebuffer()
        .iter()
        .map(|pixel| match pixel {
            NewFramePixel::DmgShade(shade) => CommonPixel::DmgShade(*shade & 0x03),
            NewFramePixel::CgbRgb555(rgb) => CommonPixel::CgbRgb555(*rgb & 0x7FFF),
        })
        .collect()
}

fn assert_blargg_passed(name: &str, old: &OldRun, new: &NewRun) {
    assert_eq!(old.stop, OldRunStop::BlarggDone, "old {name} stop");
    assert_eq!(new.stop, RunStopNg::BlarggDone, "new {name} stop");
    assert!(
        old.serial.contains("Passed"),
        "old {name} serial did not pass: {:?}",
        old.serial
    );
    assert!(
        new.serial.contains("Passed"),
        "new {name} serial did not pass: {:?}",
        new.serial
    );
    assert_eq!(old.serial, new.serial, "{name} serial transcript diverged");
}

fn assert_framebuffers_match(name: &str, old: &[CommonPixel], new: &[CommonPixel]) {
    assert_eq!(old.len(), FRAMEBUFFER_PIXELS, "old {name} framebuffer size");
    assert_eq!(new.len(), FRAMEBUFFER_PIXELS, "new {name} framebuffer size");

    if let Some((idx, (old_pixel, new_pixel))) = old
        .iter()
        .zip(new.iter())
        .enumerate()
        .find(|(_, (old_pixel, new_pixel))| old_pixel != new_pixel)
    {
        let diff = old.iter().zip(new.iter()).filter(|(a, b)| a != b).count();
        panic!(
            "{name} framebuffer diverged: diff={diff}/{FRAMEBUFFER_PIXELS}, first=({}, {}) idx={idx} old={old_pixel:?} new={new_pixel:?}",
            idx % 160,
            idx / 160
        );
    }
}

fn load_rom(relative: &str) -> Vec<u8> {
    let path = suites_dir().join(relative);
    std::fs::read(&path).unwrap_or_else(|err| panic!("failed to read ROM {path:?}: {err}"))
}

fn suites_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
        .join("reference/test-suites")
}
