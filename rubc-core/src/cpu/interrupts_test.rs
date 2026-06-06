#![cfg(test)]

use crate::bus::{Bus, CpuBus};

use super::{Cpu, CpuMode};

fn load_rom(bus: &mut Bus, program: &[u8]) {
    let mut rom = vec![0u8; 0x8000];
    rom[..program.len()].copy_from_slice(program);
    bus.cart = crate::bus::Cartridge::from_rom(&rom);
}

fn m_cycles(bus: &Bus) -> u64 {
    bus.total_ticks() / 4
}

fn run_one_instr(cpu: &mut Cpu, bus: &mut Bus) -> u64 {
    cpu.run_one_instruction(bus, m_cycles)
}

fn set_visible_irq(bus: &mut Bus, mask: u8) {
    bus.interrupts.ie = mask;
    bus.interrupts.if_ = 0xE0 | mask;
}

fn run_until_running_boundary(cpu: &mut Cpu, bus: &mut Bus) -> u64 {
    let start = m_cycles(bus);
    for _ in 0..16 {
        cpu.step_m(bus);
        if matches!(cpu.mode, CpuMode::Running) && cpu.exec_is_boundary() {
            return m_cycles(bus) - start;
        }
    }
    panic!(
        "CPU did not return to a running boundary: pc={:04X} mode={:?}",
        cpu.r.pc, cpu.mode
    );
}

fn enter_halt(cpu: &mut Cpu, bus: &mut Bus) {
    let start = m_cycles(bus);
    cpu.step_m(bus);
    cpu.step_m(bus);
    assert_eq!(m_cycles(bus) - start, 1, "HALT fetch is 1 M-cycle");
    assert!(matches!(cpu.mode, CpuMode::Halt));
    assert!(cpu.exec_is_boundary());
}

#[test]
fn ei_waits_one_instruction_and_di_disables_immediately() {
    let mut cpu = Cpu::new();
    let mut bus = Bus::new();
    load_rom(&mut bus, &[0xFB, 0x00, 0x00]);
    cpu.r.sp = 0xFFFE;
    set_visible_irq(&mut bus, 0x01);

    assert_eq!(run_one_instr(&mut cpu, &mut bus), 1, "EI is 1 M-cycle");
    assert!(!cpu.ime, "EI must not enable IME immediately");
    assert_eq!(cpu.r.pc, 0x0001);

    // The instruction after EI (NOP) executes with IME STILL OFF, so the IRQ is
    // NOT taken before it. `run_one_instruction` breaks at the post-NOP Running
    // boundary, BEFORE the next `step_m` promotes IME -> polls -> dispatches.
    let _ = run_one_instr(&mut cpu, &mut bus); // the NOP after EI
    assert!(
        !cpu.ime,
        "IME remains disabled through the instruction after EI"
    );
    assert_eq!(cpu.r.pc, 0x0002, "NOP after EI executed normally");

    // Now drive the dispatch: the next boundary promotes IME=1, then services
    // the interrupt. We prove the 1-instruction delay by its OBSERVABLE
    // consequence: the address pushed to the stack is 0x0002 (after the NOP),
    // not 0x0001 (immediately after EI).
    let _ = run_until_running_boundary(&mut cpu, &mut bus);
    assert!(!cpu.ime, "dispatch clears IME");
    assert_eq!(cpu.r.pc, 0x0040, "serviced the VBlank vector");
    let ret_lo = bus.peek(cpu.r.sp);
    let ret_hi = bus.peek(cpu.r.sp.wrapping_add(1));
    let ret = u16::from_le_bytes([ret_lo, ret_hi]);
    assert_eq!(
        ret, 0x0002,
        "EI is delayed one instruction: the NOP ran before dispatch, so the \
         pushed return address is 0x0002 (post-NOP), not 0x0001 (post-EI)"
    );

    let mut cpu = Cpu::new();
    let mut bus = Bus::new();
    load_rom(&mut bus, &[0xF3, 0x00]);
    cpu.ime = true;
    assert_eq!(run_one_instr(&mut cpu, &mut bus), 1, "DI is 1 M-cycle");
    assert!(!cpu.ime, "DI disables IME immediately");

    set_visible_irq(&mut bus, 0x01);
    assert_eq!(run_one_instr(&mut cpu, &mut bus), 1, "NOP runs after DI");
    assert_eq!(
        cpu.r.pc, 0x0002,
        "DI must prevent dispatch at the next boundary"
    );
    assert_eq!(bus.interrupts.if_ & 0x01, 0x01, "DI does not clear IF");
}

#[test]
fn ie_push_cancel_jumps_to_zero_and_preserves_if() {
    let mut cpu = Cpu::new();
    let mut bus = Bus::new();
    cpu.ime = true;
    cpu.r.pc = 0x1200;
    cpu.r.sp = 0x0000;
    set_visible_irq(&mut bus, 0x01);

    let m = run_until_running_boundary(&mut cpu, &mut bus);

    assert_eq!(
        m, 5,
        "cancelled interrupt dispatch still consumes 5 M-cycles"
    );
    assert_eq!(cpu.r.pc, 0x0000, "IE cleared mid-push cancels to PC=0000");
    assert_eq!(
        cpu.r.sp, 0xFFFE,
        "dispatch still performs both stack pushes"
    );
    assert_eq!(bus.interrupts.ie, 0x12, "PC high byte write clobbered IE");
    assert_eq!(
        bus.interrupts.if_ & 0x01,
        0x01,
        "cancelled dispatch must not clear the original IF bit"
    );
}

#[test]
fn halt_bug_reuses_next_byte_as_opcode_operand() {
    let mut cpu = Cpu::new();
    let mut bus = Bus::new();
    load_rom(&mut bus, &[0x76, 0x06, 0x42, 0x00]);
    set_visible_irq(&mut bus, 0x01);

    assert_eq!(
        run_one_instr(&mut cpu, &mut bus),
        1,
        "HALT fetch is 1 M-cycle"
    );
    assert_eq!(cpu.r.pc, 0x0001, "HALT itself advances PC once");
    assert!(matches!(cpu.mode, CpuMode::Running));

    assert_eq!(
        run_one_instr(&mut cpu, &mut bus),
        2,
        "LD B,d8 is 2 M-cycles"
    );
    assert_eq!(
        cpu.r.b, 0x06,
        "HALT bug leaves PC on the opcode byte, so LD B,d8 re-reads 0x06 as d8"
    );
    assert_eq!(cpu.r.pc, 0x0002, "PC is one byte short after the HALT bug");
    assert_eq!(
        bus.interrupts.if_ & 0x01,
        0x01,
        "IME=0 means no service occurred"
    );
}

#[test]
fn halt_wake_with_ime_services_interrupt_in_five_mcycles() {
    let mut cpu = Cpu::new();
    let mut bus = Bus::new();
    load_rom(&mut bus, &[0x76]);
    cpu.ime = true;
    cpu.r.sp = 0xFFFE;
    bus.interrupts.ie = 0x01;
    bus.interrupts.if_ = 0xE0;

    enter_halt(&mut cpu, &mut bus);
    bus.interrupts.request(0);

    let m = run_until_running_boundary(&mut cpu, &mut bus);

    assert_eq!(m, 5, "HALT wake + service is the 5-M interrupt dispatch");
    assert_eq!(cpu.r.pc, 0x0040, "VBlank vector serviced after HALT wake");
    assert_eq!(bus.interrupts.if_ & 0x01, 0x00, "servicing clears IF bit 0");
    assert!(!cpu.ime, "dispatch clears IME");
}

#[test]
fn halt_wake_without_ime_resumes_after_halt_without_service() {
    let mut cpu = Cpu::new();
    let mut bus = Bus::new();
    load_rom(&mut bus, &[0x76, 0x3C, 0x00]);
    bus.interrupts.ie = 0x01;
    bus.interrupts.if_ = 0xE0;

    enter_halt(&mut cpu, &mut bus);
    bus.interrupts.request(0);

    let start = m_cycles(&bus);
    cpu.step_m(&mut bus);
    assert_eq!(m_cycles(&bus) - start, 1, "wake fetches the next opcode");
    assert!(matches!(cpu.mode, CpuMode::Running));
    assert!(
        !cpu.exec_is_boundary(),
        "INC A has been fetched, not serviced"
    );
    assert_eq!(cpu.r.pc, 0x0002);

    assert_eq!(
        run_until_running_boundary(&mut cpu, &mut bus),
        0,
        "INC A uses no extra bus M-cycle"
    );
    assert_eq!(
        cpu.r.a, 0x01,
        "execution resumes with the instruction after HALT"
    );
    assert_eq!(cpu.r.pc, 0x0002, "no jump to an interrupt vector occurred");
    assert_eq!(
        bus.interrupts.if_ & 0x01,
        0x01,
        "IME=0 wake leaves IF pending"
    );
}

#[test]
fn dispatch_priority_services_lowest_pending_bit_first() {
    let mut cpu = Cpu::new();
    let mut bus = Bus::new();
    cpu.ime = true;
    cpu.r.pc = 0x0150;
    cpu.r.sp = 0xFFFE;
    set_visible_irq(&mut bus, 0x05);

    let m = run_until_running_boundary(&mut cpu, &mut bus);

    assert_eq!(m, 5, "interrupt dispatch is 5 M-cycles");
    assert_eq!(
        cpu.r.pc, 0x0040,
        "lowest pending bit has priority: VBlank first"
    );
    assert_eq!(
        bus.interrupts.if_ & 0x05,
        0x04,
        "servicing bit 0 clears only VBlank; Timer remains pending"
    );
    assert_eq!(
        bus.irq_pending_mask(),
        0x04,
        "Timer is still pending for the next service"
    );
}

#[test]
fn stop_with_armed_key1_in_cgb_switches_speed_and_resumes() {
    // CGB speed-switch idiom: LD A,1 ; LDH (KEY1),A ; STOP ; NOP.
    // STOP must perform the switch and RESUME (not halt).
    let mut cpu = Cpu::new();
    let mut bus = Bus::new();
    bus.cgb.cgb_mode = true;
    load_rom(&mut bus, &[0x3E, 0x01, 0xE0, 0x4D, 0x10, 0x00, 0x00]);
    cpu.r.sp = 0xFFFE;
    assert!(!bus.cgb.double_speed, "starts at normal speed");

    run_one_instr(&mut cpu, &mut bus); // LD A,1
    run_one_instr(&mut cpu, &mut bus); // LDH (KEY1),A  -> arms the switch
    run_one_instr(&mut cpu, &mut bus); // STOP -> switch + resume

    assert!(
        matches!(cpu.mode, CpuMode::Running),
        "armed STOP resumes (does not halt) in CGB mode, mode={:?}",
        cpu.mode
    );
    assert!(bus.cgb.double_speed, "STOP performed the speed switch");
}

#[test]
fn stop_in_dmg_halts_even_after_key1_write() {
    // In DMG mode KEY1 is inert, so the same byte sequence must HALT at STOP.
    let mut cpu = Cpu::new();
    let mut bus = Bus::new(); // cgb_mode defaults to false (DMG)
    load_rom(&mut bus, &[0x3E, 0x01, 0xE0, 0x4D, 0x10, 0x00, 0x00]);
    cpu.r.sp = 0xFFFE;

    run_one_instr(&mut cpu, &mut bus); // LD A,1
    run_one_instr(&mut cpu, &mut bus); // LDH (KEY1),A  -> ignored in DMG
                                       // STOP: drive a few M-cycles; the CPU must enter Stopped, not resume.
    for _ in 0..4 {
        cpu.step_m(&mut bus);
    }
    assert!(
        matches!(cpu.mode, CpuMode::Stopped),
        "DMG STOP halts the CPU, mode={:?}",
        cpu.mode
    );
    assert!(!bus.cgb.double_speed, "DMG never switches speed");
}
