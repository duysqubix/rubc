use rubc_core::machine::Machine;

fn blank_rom() -> Vec<u8> {
    let mut rom = vec![0; 0x8000];
    rom[0x0100] = 0x00;
    rom[0x0101] = 0x00;
    rom
}

fn run_instructions(machine: &mut Machine, count: usize) {
    for _ in 0..count {
        machine.step_instruction();
    }
}

#[test]
fn save_state_restores_cpu_registers_into_fresh_machine() {
    let rom = blank_rom();
    let mut saved = Machine::boot_dmg(&rom);
    saved.cpu.r.a = 0x42;
    saved.cpu.r.f = 0x90;
    saved.cpu.r.b = 0x12;
    saved.cpu.r.c = 0x34;
    saved.cpu.r.d = 0x56;
    saved.cpu.r.e = 0x78;
    saved.cpu.r.h = 0x9A;
    saved.cpu.r.l = 0xBC;
    saved.cpu.r.sp = 0xD123;
    saved.cpu.r.pc = 0xC000;
    saved.cpu.ime = true;

    let blob = saved.save_state();
    let mut restored = Machine::boot_dmg(&rom);
    restored.load_state(&blob).expect("valid save state");

    assert_eq!(restored.cpu.r, saved.cpu.r);
    assert_eq!(restored.cpu.ime, saved.cpu.ime);
    assert_eq!(restored.cpu.mode, saved.cpu.mode);
}

#[test]
fn save_state_restores_wram_and_hram() {
    let rom = blank_rom();
    let mut saved = Machine::boot_dmg(&rom);
    saved.bus.poke(0xC123, 0xA5);
    saved.bus.poke(0xD234, 0x5A);
    saved.bus.poke(0xFF80, 0xC3);

    let blob = saved.save_state();
    saved.bus.poke(0xC123, 0);
    saved.bus.poke(0xD234, 0);
    saved.bus.poke(0xFF80, 0);

    let mut restored = Machine::boot_dmg(&rom);
    restored.load_state(&blob).expect("valid save state");

    assert_eq!(restored.bus.peek(0xC123), 0xA5);
    assert_eq!(restored.bus.peek(0xD234), 0x5A);
    assert_eq!(restored.bus.peek(0xFF80), 0xC3);
}

#[test]
fn save_state_restores_vram_and_oam() {
    let rom = blank_rom();
    let mut saved = Machine::boot_dmg(&rom);
    saved.bus.poke(0x8000, 0x11);
    saved.bus.poke(0x9ABC, 0x22);
    saved.bus.poke(0xFE00, 0x33);
    saved.bus.poke(0xFE9F, 0x44);

    let blob = saved.save_state();
    saved.bus.poke(0x8000, 0);
    saved.bus.poke(0x9ABC, 0);
    saved.bus.poke(0xFE00, 0);
    saved.bus.poke(0xFE9F, 0);

    let mut restored = Machine::boot_dmg(&rom);
    restored.load_state(&blob).expect("valid save state");

    assert_eq!(restored.bus.peek(0x8000), 0x11);
    assert_eq!(restored.bus.peek(0x9ABC), 0x22);
    assert_eq!(restored.bus.peek(0xFE00), 0x33);
    assert_eq!(restored.bus.peek(0xFE9F), 0x44);
}

#[test]
fn save_state_restores_mid_frame_execution_equivalence() {
    let rom = blank_rom();
    let mut reference = Machine::boot_dmg(&rom);
    run_instructions(&mut reference, 512);

    let blob = reference.save_state();
    let mut restored = Machine::boot_dmg(&rom);
    restored.load_state(&blob).expect("valid save state");

    run_instructions(&mut reference, 512);
    run_instructions(&mut restored, 512);

    assert_eq!(restored.cpu.r, reference.cpu.r);
    assert_eq!(restored.cpu.ime, reference.cpu.ime);
    assert_eq!(restored.cpu.mode, reference.cpu.mode);
    assert_eq!(restored.bus.ppu.framebuffer, reference.bus.ppu.framebuffer);
    assert_eq!(restored.save_state(), reference.save_state());
}

#[test]
fn save_state_restores_mbc_bank_ram_and_rtc_selection() {
    let mut rom = vec![0; 0xC000];
    rom[0x0147] = 0x13;
    rom[0x0149] = 0x03;
    rom[0x4000] = 0xB1;
    rom[0x8000] = 0xB2;

    let mut saved = Machine::boot_dmg(&rom);
    saved.bus.poke(0x0000, 0x0A);
    saved.bus.poke(0x2000, 0x02);
    saved.bus.poke(0x4000, 0x01);
    saved.bus.poke(0xA000, 0x77);
    saved.bus.poke(0x4000, 0x08);
    saved.bus.poke(0xA000, 0x45);
    saved.bus.poke(0x6000, 0x00);
    saved.bus.poke(0x6000, 0x01);

    let blob = saved.save_state();
    let mut restored = Machine::boot_dmg(&rom);
    restored.load_state(&blob).expect("valid save state");

    assert_eq!(restored.bus.peek(0x4000), 0xB2);
    assert_eq!(restored.bus.peek(0xA000), 0x45);
    restored.bus.poke(0x4000, 0x01);
    assert_eq!(restored.bus.peek(0xA000), 0x77);
}

#[test]
fn load_state_rejects_bad_magic_and_version() {
    let rom = blank_rom();
    let mut machine = Machine::boot_dmg(&rom);
    let good = machine.save_state();

    let mut bad_magic = good.clone();
    bad_magic[0] = b'X';
    assert!(machine.load_state(&bad_magic).is_err());

    let mut bad_version = good;
    bad_version[4] = 0xFF;
    bad_version[5] = 0xFF;
    assert!(machine.load_state(&bad_version).is_err());
}

#[test]
fn save_state_v2_preserves_cgb_hardware_flag() {
    let rom = blank_rom();
    let saved = Machine::boot_cgb(&rom);
    assert!(saved.bus.cgb.is_cgb);
    assert!(!saved.bus.cgb.cgb_mode);
    let blob = saved.save_state();
    assert_eq!(u16::from_le_bytes([blob[4], blob[5]]), 2);

    let mut restored = Machine::boot_dmg(&rom);
    restored.load_state(&blob).expect("valid v2 save state");
    assert!(restored.bus.cgb.is_cgb);
    assert!(!restored.bus.cgb.cgb_mode);
}

#[test]
fn save_state_v1_missing_is_cgb_defaults_to_cgb_mode() {
    let rom = blank_rom();
    let saved = Machine::boot_cgb_native(&rom);
    let payload = serde_json::to_value((&saved.cpu, &saved.bus)).expect("serialize payload");
    let mut payload = payload.as_array().expect("tuple payload").clone();
    payload[1]["cgb"].as_object_mut().unwrap().remove("is_cgb");
    let payload = serde_json::to_vec(&payload).expect("serialize v1-ish payload");
    let mut blob = Vec::new();
    blob.extend_from_slice(b"RUSV");
    blob.extend_from_slice(&1u16.to_le_bytes());
    blob.extend_from_slice(&payload);

    let mut restored = Machine::boot_dmg(&rom);
    restored.load_state(&blob).expect("valid v1 save state");
    assert!(restored.bus.cgb.cgb_mode);
    assert!(restored.bus.cgb.is_cgb);
}
