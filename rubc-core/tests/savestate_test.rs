use rubc_core::machine::Machine;

fn blank_rom() -> Vec<u8> {
    let mut rom = vec![0; 0x8000];
    rom[0x0100] = 0x00;
    rom[0x0101] = 0x00;
    rom
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
