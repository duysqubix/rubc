use std::path::{Path, PathBuf};

use rubc_ng::{GbModel, MachineNg, RunStopNg};

const TIMING_ROMS: &[&str] = &[
    "add_sp_e_timing.gb",
    "call_cc_timing.gb",
    "call_cc_timing2.gb",
    "call_timing.gb",
    "call_timing2.gb",
    "jp_cc_timing.gb",
    "jp_timing.gb",
    "ld_hl_sp_e_timing.gb",
    "push_timing.gb",
    "ret_cc_timing.gb",
    "ret_timing.gb",
    "reti_timing.gb",
    "rst_timing.gb",
];

const CGB_TIMING_ROMS: &[&str] = &[
    "add_sp_e_timing.gb",
    "call_cc_timing.gb",
    "call_cc_timing2.gb",
    "call_timing.gb",
    "call_timing2.gb",
    "jp_cc_timing.gb",
    "jp_timing.gb",
    "ld_hl_sp_e_timing.gb",
    "push_timing.gb",
    "ret_cc_timing.gb",
    "ret_timing.gb",
    "reti_timing.gb",
    "rst_timing.gb",
    "oam_dma_restart.gb",
    "oam_dma_start.gb",
    "oam_dma_timing.gb",
    "timer/rapid_toggle.gb",
];

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
}

fn timing_rom_path(name: &str) -> PathBuf {
    workspace_root()
        .join("reference/test-suites/mooneye-test-suite/build/acceptance")
        .join(name)
}

fn assert_timing_roms_pass(model: GbModel, roms: &[&str], label: &str) {
    let mut failures = Vec::new();

    for rom_name in roms {
        let path = timing_rom_path(rom_name);
        let rom = std::fs::read(&path)
            .unwrap_or_else(|err| panic!("{} is present: {err}", path.display()));
        let mut machine = MachineNg::from_rom(model, &rom)
            .unwrap_or_else(|err| panic!("{rom_name} boots as {model:?}: {err}"));

        let stop = machine.run_mooneye(20_000_000);
        let passed = stop == RunStopNg::MooneyeBreakpoint && machine.mooneye_passed();
        if !passed {
            failures.push(format!(
                "{rom_name}: stop={stop:?}, signature={:02X?}, hram0={:02X?}, serial={:?}",
                machine.mooneye_signature(),
                machine.hram_debug_prefix(),
                machine.serial_output()
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "mooneye {label} timing failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn mooneye_cpu_instruction_timing_roms_reach_fibonacci_signature() {
    assert_timing_roms_pass(GbModel::DmgB, TIMING_ROMS, "DMG-B CPU instruction");
}

#[test]
fn mooneye_universal_timing_roms_pass_on_cgb_e() {
    assert_timing_roms_pass(GbModel::CgbE, CGB_TIMING_ROMS, "CGB-E universal");
}

#[test]
fn mooneye_universal_timing_roms_stay_passing_on_dmg_b() {
    assert_timing_roms_pass(GbModel::DmgB, CGB_TIMING_ROMS, "DMG-B universal");
}
