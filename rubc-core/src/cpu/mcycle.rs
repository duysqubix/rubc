//! Per-opcode M-cycle count harness.
//!
//! The SM83 vector tests already assert `mcycles == cycles.len()` for each
//! sampled vector, but each vector exercises only ONE branch outcome. This
//! module drives the CPU directly with synthetic programs to pin the canonical
//! M-cycle count of representative opcodes AND both outcomes of every
//! conditional branch (taken vs not-taken differ: JR/JP by 1 M-cycle, CALL/RET
//! cc by 3 M-cycles), plus the 5-M-cycle interrupt dispatch.

#![cfg(test)]

use crate::bus::{CpuBus, FlatBus};

use super::core::{Cpu, CpuMode};

/// Load `program` at 0x0000, set the Z/C flags, run ONE instruction, return the
/// bus M-cycle count it consumed.
fn count_mcycles(program: &[u8], set_flags: super::alu::Flags) -> u64 {
    let mut cpu = Cpu::new();
    let mut bus = FlatBus::new();
    for (i, &b) in program.iter().enumerate() {
        bus.poke(i as u16, b);
    }
    cpu.r.set_flags(set_flags);
    cpu.r.pc = 0x0000;
    cpu.run_one_instruction(&mut bus, |b| b.m_cycles)
}

use super::alu::Flags;

/// Flags with Z set (others clear).
fn z() -> Flags {
    Flags::new(true, false, false, false)
}
/// Flags with C set.
fn c() -> Flags {
    Flags::new(false, false, false, true)
}
/// All flags clear.
fn none() -> Flags {
    Flags::default()
}

#[test]
fn canonical_counts() {
    // (program bytes, flags, expected M-cycles, label)
    let cases: &[(&[u8], Flags, u64, &str)] = &[
        (&[0x00], none(), 1, "NOP"),
        (&[0x41], none(), 1, "LD B,C"),
        (&[0x06, 0x12], none(), 2, "LD B,d8"),
        (&[0x01, 0x34, 0x12], none(), 3, "LD BC,d16"),
        (&[0x46], none(), 2, "LD B,(HL)"),
        (&[0x70], none(), 2, "LD (HL),B"),
        (&[0x36, 0xAB], none(), 3, "LD (HL),d8"),
        (&[0x86], none(), 2, "ADD A,(HL)"),
        (&[0x34], none(), 3, "INC (HL)"),
        (&[0xC5], none(), 4, "PUSH BC"),
        (&[0xC1], none(), 3, "POP BC"),
        (&[0xC3, 0x00, 0x40], none(), 4, "JP a16"),
        (&[0xE9], none(), 1, "JP HL"),
        (&[0xCD, 0x00, 0x40], none(), 6, "CALL a16"),
        (&[0xC9], none(), 4, "RET"),
        (&[0xD9], none(), 4, "RETI"),
        (&[0xC7], none(), 4, "RST 00"),
        (&[0xE0, 0x80], none(), 3, "LDH (a8),A"),
        (&[0xF0, 0x80], none(), 3, "LDH A,(a8)"),
        (&[0xE2], none(), 2, "LD (C),A"),
        (&[0xEA, 0x00, 0xC0], none(), 4, "LD (a16),A"),
        (&[0xFA, 0x00, 0xC0], none(), 4, "LD A,(a16)"),
        (&[0x03], none(), 2, "INC BC"),
        (&[0x09], none(), 2, "ADD HL,BC"),
        (&[0xE8, 0x01], none(), 4, "ADD SP,e8"),
        (&[0xF8, 0x01], none(), 3, "LD HL,SP+e8"),
        (&[0xF9], none(), 2, "LD SP,HL"),
        (&[0x08, 0x00, 0xC0], none(), 5, "LD (a16),SP"),
        (&[0x07], none(), 1, "RLCA"),
        (&[0x27], none(), 1, "DAA"),
        (&[0xF3], none(), 1, "DI"),
        (&[0xFB], none(), 1, "EI"),
        (&[0xC6, 0x01], none(), 2, "ADD A,d8"),
        // CB: register op = 2 M (CB fetch + op), (HL) RMW = 4, BIT n,(HL) = 3.
        (&[0xCB, 0x00], none(), 2, "RLC B"),
        (&[0xCB, 0x06], none(), 4, "RLC (HL)"),
        (&[0xCB, 0x46], none(), 3, "BIT 0,(HL)"),
        (&[0xCB, 0x86], none(), 4, "RES 0,(HL)"),
        (&[0xCB, 0x40], none(), 2, "BIT 0,B"),
    ];

    for (prog, flags, expected, label) in cases {
        let m = count_mcycles(prog, *flags);
        assert_eq!(
            m, *expected,
            "{label}: expected {expected} M-cycles, got {m}"
        );
    }
}

#[test]
fn branch_timing_taken_vs_not_taken() {
    // JR NZ,e8 (0x20): taken (Z=0) = 3M, not-taken (Z=1) = 2M.
    assert_eq!(count_mcycles(&[0x20, 0x05], none()), 3, "JR NZ taken");
    assert_eq!(count_mcycles(&[0x20, 0x05], z()), 2, "JR NZ not-taken");

    // JR Z,e8 (0x28): taken (Z=1) = 3M, not-taken (Z=0) = 2M.
    assert_eq!(count_mcycles(&[0x28, 0x05], z()), 3, "JR Z taken");
    assert_eq!(count_mcycles(&[0x28, 0x05], none()), 2, "JR Z not-taken");

    // JP NZ,a16 (0xC2): taken = 4M, not-taken = 3M.
    assert_eq!(count_mcycles(&[0xC2, 0x00, 0x40], none()), 4, "JP NZ taken");
    assert_eq!(
        count_mcycles(&[0xC2, 0x00, 0x40], z()),
        3,
        "JP NZ not-taken"
    );

    // CALL NZ,a16 (0xC4): taken = 6M, not-taken = 3M.
    assert_eq!(
        count_mcycles(&[0xC4, 0x00, 0x40], none()),
        6,
        "CALL NZ taken"
    );
    assert_eq!(
        count_mcycles(&[0xC4, 0x00, 0x40], z()),
        3,
        "CALL NZ not-taken"
    );

    // RET NZ (0xC0): taken = 5M, not-taken = 2M.
    assert_eq!(count_mcycles(&[0xC0], none()), 5, "RET NZ taken");
    assert_eq!(count_mcycles(&[0xC0], z()), 2, "RET NZ not-taken");

    // RET C (0xD8): taken (C=1) = 5M, not-taken (C=0) = 2M.
    assert_eq!(count_mcycles(&[0xD8], c()), 5, "RET C taken");
    assert_eq!(count_mcycles(&[0xD8], none()), 2, "RET C not-taken");

    // JP C,a16 (0xDA): taken = 4M, not-taken = 3M.
    assert_eq!(count_mcycles(&[0xDA, 0x00, 0x40], c()), 4, "JP C taken");
    assert_eq!(
        count_mcycles(&[0xDA, 0x00, 0x40], none()),
        3,
        "JP C not-taken"
    );
}

#[test]
fn interrupt_dispatch_is_5_mcycles() {
    let mut cpu = Cpu::new();
    let mut bus = crate::bus::Bus::new();
    // NOP at PC; interrupt handler vector area irrelevant for the count.
    bus.poke(0x0000, 0x00);
    cpu.ime = true;
    cpu.r.sp = 0xFFFE; // a real stack, so the PC push doesn't clobber IE at 0xFFFF
    bus.interrupts.ie = 0x01; // enable VBlank
    bus.interrupts.request(0); // request VBlank
    bus.boundary(); // settle into IF so it's visible at the next poll

    // At the boundary, the CPU should dispatch the interrupt over 5 M-cycles.
    assert_eq!(
        bus.irq_pending_mask(),
        0x01,
        "IRQ must be visible before stepping (ie={:02X} if pre-step)",
        bus.ie()
    );
    assert!(cpu.ime, "IME must be set");
    let start = bus.total_ticks() / 4;
    // Step until the dispatch completes (mode returns to Running having jumped
    // to the vector). Count the M-cycles consumed by the dispatch itself.
    let mut guard = 0;
    loop {
        cpu.step_m(&mut bus);
        guard += 1;
        if matches!(cpu.mode, CpuMode::Running) && cpu.r.pc == 0x0040 {
            break;
        }
        if guard > 16 {
            panic!(
                "dispatch incomplete: pc={:04X} mode={:?}",
                cpu.r.pc, cpu.mode
            );
        }
    }
    let dispatched = bus.total_ticks() / 4 - start;
    assert_eq!(
        dispatched, 5,
        "interrupt dispatch is 5 M-cycles, got {dispatched}"
    );
    assert!(!cpu.ime, "IME cleared during dispatch");
}
