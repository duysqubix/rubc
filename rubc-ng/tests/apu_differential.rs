use rubc_core::bus::apu::Apu as OldApu;
use std::path::Path;

use rubc_ng::{
    Apu as NewApu, ConformanceConfig, ConformanceOutcome, ConformanceReport, MachineNg, RunStopNg,
};

fn write_both(old: &mut OldApu, new: &mut NewApu, addr: u16, value: u8, cgb: bool) {
    old.write(addr, value, cgb);
    new.write(addr, value, cgb);
}

fn reference_rom(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
        .join("reference/test-suites/gb-test-roms")
        .join(relative);
    std::fs::read(&path).unwrap_or_else(|_| panic!("reference ROM must exist at {path:?}"))
}

#[test]
fn new_apu_matches_old_core_registers_and_samples_for_mixed_channel_program() {
    let mut old = OldApu::default();
    let mut new = NewApu::default();
    old.set_sample_rate(48_000);
    new.set_sample_rate(48_000);

    for (addr, value) in [
        (0xFF26, 0x80),
        (0xFF24, 0x77),
        (0xFF25, 0xFF),
        (0xFF10, 0x21),
        (0xFF11, 0x80),
        (0xFF12, 0xF2),
        (0xFF13, 0x00),
        (0xFF14, 0x82),
        (0xFF16, 0x40),
        (0xFF17, 0x84),
        (0xFF18, 0xC0),
        (0xFF19, 0x87),
        (0xFF1A, 0x80),
        (0xFF1C, 0x20),
        (0xFF1D, 0xFF),
        (0xFF1E, 0x87),
        (0xFF20, 0x20),
        (0xFF21, 0xF3),
        (0xFF22, 0x08),
        (0xFF23, 0x80),
    ] {
        write_both(&mut old, &mut new, addr, value, false);
    }
    for i in 0..16u16 {
        write_both(
            &mut old,
            &mut new,
            0xFF30 + i,
            (0x10 + i as u8) ^ 0xA5,
            false,
        );
    }

    for t in 1..=20_000u16 {
        old.tick_t();
        new.tick_spine(t, false);
        let old_high = t & 0x1000 != 0;
        let new_high = t.wrapping_add(1) & 0x1000 != 0;
        if old_high && !new_high {
            old.tick_div_apu();
        }
    }

    for addr in [
        0xFF10, 0xFF12, 0xFF14, 0xFF17, 0xFF19, 0xFF1A, 0xFF1C, 0xFF1E, 0xFF21, 0xFF22, 0xFF23,
        0xFF24, 0xFF25, 0xFF26,
    ] {
        assert_eq!(
            new.read(addr),
            old.read(addr),
            "register {addr:#06X} diverged"
        );
    }

    let mut old_samples = Vec::new();
    let mut new_samples = Vec::new();
    old.drain_samples(&mut old_samples);
    new.drain_samples(&mut new_samples);
    assert_eq!(
        new_samples.len(),
        old_samples.len(),
        "sample count diverged"
    );
    for (idx, (new_sample, old_sample)) in new_samples.iter().zip(old_samples.iter()).enumerate() {
        assert_eq!(
            new_sample.to_bits(),
            old_sample.to_bits(),
            "sample {idx} diverged"
        );
    }
}

#[test]
fn cgb_div_reset_clocks_length_before_next_register_write_like_old_core() {
    let mut old = OldApu::default();
    let mut new = NewApu::default();

    write_both(&mut old, &mut new, 0xFF21, 0xF0, true);
    write_both(&mut old, &mut new, 0xFF20, 0x3F, true);
    write_both(&mut old, &mut new, 0xFF23, 0xC0, true);
    assert_eq!(old.read(0xFF26) & 0x08, 0x08, "old CH4 starts enabled");
    assert_eq!(new.read(0xFF26) & 0x08, 0x08, "new CH4 starts enabled");

    old.tick_div_apu();
    new.observe_div_apu_counter_change(0x1000, 0x0000, false);

    assert_eq!(
        new.read(0xFF26),
        old.read(0xFF26),
        "DIV reset falling edge must clock length immediately, before following APU writes"
    );
}

#[test]
fn cgb_div_reset_clocks_sweep_before_next_register_write_like_old_core() {
    let mut old = OldApu::default();
    let mut new = NewApu::default();

    write_both(&mut old, &mut new, 0xFF10, 0x11, true);
    write_both(&mut old, &mut new, 0xFF12, 0xF0, true);
    write_both(&mut old, &mut new, 0xFF13, 0x00, true);
    write_both(&mut old, &mut new, 0xFF14, 0x80, true);
    old.tick_div_apu();
    old.tick_div_apu();
    new.tick_div_apu();
    new.tick_div_apu();

    old.tick_div_apu();
    new.observe_div_apu_counter_change(0x1000, 0x0000, false);

    assert_eq!(
        new.read(0xFF26),
        old.read(0xFF26),
        "DIV reset falling edge must clock sweep on frame sequencer step 2 immediately"
    );
}

#[test]
fn cgb_power_cycle_length_state_matches_old_core_after_div_reset_clock() {
    let mut old = OldApu::default();
    let mut new = NewApu::default();

    write_both(&mut old, &mut new, 0xFF21, 0xF0, true);
    write_both(&mut old, &mut new, 0xFF20, 0x3F, true);
    write_both(&mut old, &mut new, 0xFF23, 0xC0, true);
    old.tick_div_apu();
    new.observe_div_apu_counter_change(0x1000, 0x0000, false);
    write_both(&mut old, &mut new, 0xFF26, 0x00, true);
    write_both(&mut old, &mut new, 0xFF26, 0x80, true);
    write_both(&mut old, &mut new, 0xFF21, 0xF0, true);
    write_both(&mut old, &mut new, 0xFF23, 0xC0, true);

    assert_eq!(
        new.read(0xFF26),
        old.read(0xFF26),
        "CGB power cycle after DIV-reset length clock must clear/reload length like old core"
    );
}

#[test]
fn cgb_wave_ram_access_after_div_reset_sync_matches_old_core() {
    let mut old = OldApu::default();
    let mut new = NewApu::default();

    for i in 0..16u16 {
        write_both(&mut old, &mut new, 0xFF30 + i, 0x10 + i as u8, true);
    }
    write_both(&mut old, &mut new, 0xFF1A, 0x80, true);
    write_both(&mut old, &mut new, 0xFF1C, 0x20, true);
    write_both(&mut old, &mut new, 0xFF1D, 0xF0, true);
    write_both(&mut old, &mut new, 0xFF1E, 0x80, true);
    for _ in 0..40 {
        old.tick_t();
        new.tick_t();
    }

    old.tick_div_apu();
    new.observe_div_apu_counter_change(0x1000, 0x0000, false);
    write_both(&mut old, &mut new, 0xFF30, 0xA5, true);

    for addr in 0xFF30..=0xFF3F {
        assert_eq!(
            new.read_for_model(addr, true),
            old.read_for_model(addr, true),
            "CGB wave RAM register {addr:#06X} diverged after DIV-reset sync"
        );
    }
}

#[test]
fn machine_ng_cgb_sound_combined_rom_reaches_blargg_pass() {
    let rom = reference_rom("cgb_sound/cgb_sound.gb");
    let mut machine = MachineNg::boot_cgb_native(&rom).expect("CGB sound ROM boots");

    let stop = machine.run_blargg(120_000_000);
    println!(
        "cgb_sound serial={:?} cart={:?}",
        machine.serial_output(),
        machine.blargg_cart_text()
    );

    assert_eq!(
        stop,
        RunStopNg::BlarggDone,
        "cgb_sound must terminate through blargg oracle; serial={:?} cart={:?}",
        machine.serial_output(),
        machine.blargg_cart_text()
    );
    assert!(
        machine.blargg_passed(),
        "cgb_sound must report Passed; serial={:?} cart={:?}",
        machine.serial_output(),
        machine.blargg_cart_text()
    );
}

#[test]
fn machine_ng_dmg_sound_combined_rom_still_reaches_blargg_pass() {
    let rom = reference_rom("dmg_sound/dmg_sound.gb");
    let mut machine = MachineNg::boot_dmg(&rom).expect("DMG sound ROM boots");

    let stop = machine.run_blargg(120_000_000);
    println!(
        "dmg_sound serial={:?} cart={:?}",
        machine.serial_output(),
        machine.blargg_cart_text()
    );

    assert_eq!(
        stop,
        RunStopNg::BlarggDone,
        "dmg_sound must terminate through blargg oracle; serial={:?} cart={:?}",
        machine.serial_output(),
        machine.blargg_cart_text()
    );
    assert!(
        machine.blargg_passed(),
        "dmg_sound must report Passed; serial={:?} cart={:?}",
        machine.serial_output(),
        machine.blargg_cart_text()
    );
}

#[test]
fn conformance_cgb_sound_row_is_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent");
    let report = ConformanceReport::run(
        root,
        ConformanceConfig {
            pass_floor: 1,
            full_manifest: false,
            path_substrings: vec!["gb-test-roms/cgb_sound/cgb_sound.gb".to_owned()],
        },
    )
    .expect("conformance harness runs cgb_sound row");
    println!("{}", report.scoreboard());

    assert_eq!(report.total_roms, 1, "only combined cgb_sound row selected");
    assert_eq!(report.rows[0].outcome, ConformanceOutcome::Pass);
}
