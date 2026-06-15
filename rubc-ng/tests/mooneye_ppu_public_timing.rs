use std::path::{Path, PathBuf};

use rubc_ng::{GbModel, MachineNg, RunStopNg};

const PPU_PUBLIC_TIMING_ROMS: &[&str] = &[
    "intr_2_0_timing.gb",
    "intr_2_mode0_timing.gb",
    "intr_2_mode0_timing_sprites.gb",
    "intr_2_mode3_timing.gb",
    "intr_2_oam_ok_timing.gb",
    "stat_lyc_onoff.gb",
    "vblank_stat_intr-GS.gb",
    "lcdon_timing-GS.gb",
    "lcdon_write_timing-GS.gb",
    "hblank_ly_scx_timing-GS.gb",
];

const FIXED_PPU_PUBLIC_TIMING_ROMS: &[&str] = &[
    "intr_2_0_timing.gb",
    "intr_2_mode0_timing.gb",
    "intr_2_mode0_timing_sprites.gb",
    "intr_2_mode3_timing.gb",
    "intr_2_oam_ok_timing.gb",
    "stat_lyc_onoff.gb",
    "vblank_stat_intr-GS.gb",
    "lcdon_timing-GS.gb",
    "lcdon_write_timing-GS.gb",
    "hblank_ly_scx_timing-GS.gb",
];

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
}

fn ppu_rom_path(name: &str) -> PathBuf {
    workspace_root()
        .join("reference/test-suites/mooneye-test-suite/build/acceptance/ppu")
        .join(name)
}

fn run_mooneye_ppu_rom(rom_name: &str) -> Result<(), String> {
    let path = ppu_rom_path(rom_name);
    let rom =
        std::fs::read(&path).map_err(|err| format!("{} is present: {err}", path.display()))?;
    let mut machine = MachineNg::from_rom(GbModel::DmgB, &rom)
        .map_err(|err| format!("{rom_name} boots as DMG-B: {err}"))?;

    let stop = machine.run_mooneye(20_000_000);
    if stop == RunStopNg::MooneyeBreakpoint && machine.mooneye_passed() {
        Ok(())
    } else {
        Err(format!(
            "{rom_name}: stop={stop:?}, signature={:02X?}, hram0={:02X?}, serial={:?}",
            machine.mooneye_signature(),
            machine.hram_debug_prefix(),
            machine.serial_output()
        ))
    }
}

#[test]
#[ignore = "diagnostic all-10 gate; documents remaining PPU-public sub-dot failures"]
fn mooneye_ppu_public_timing_roms_reach_fibonacci_signature() {
    let failures = PPU_PUBLIC_TIMING_ROMS
        .iter()
        .filter_map(|rom_name| run_mooneye_ppu_rom(rom_name).err())
        .collect::<Vec<_>>();

    assert!(
        failures.is_empty(),
        "mooneye PPU-public timing failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn fixed_mooneye_ppu_public_timing_roms_reach_fibonacci_signature() {
    let failures = FIXED_PPU_PUBLIC_TIMING_ROMS
        .iter()
        .filter_map(|rom_name| run_mooneye_ppu_rom(rom_name).err())
        .collect::<Vec<_>>();

    assert!(
        failures.is_empty(),
        "fixed mooneye PPU-public timing regressions:\n{}",
        failures.join("\n")
    );
}

#[test]
fn stat_irq_blocking_stays_green() {
    run_mooneye_ppu_rom("stat_irq_blocking.gb").expect("stat_irq_blocking remains green");
}
