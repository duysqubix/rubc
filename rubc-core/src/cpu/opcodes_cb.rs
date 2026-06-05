//! CB-prefixed opcode execution, phase by phase.
//!
//! `step(cpu, bus, op, phase)` runs ONE bus M-cycle of CB opcode `op` at
//! `phase`. Register-target ops are 1 M-cycle (after the CB fetch); `(HL)`
//! targets add read/write M-cycles.

use crate::bus::CpuBus;

use super::{alu, core::Cpu};

/// Execute one M-cycle of CB opcode `op` at `phase`.
pub fn step<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match op {
        0x00..=0x3F => step_rotate_shift(cpu, bus, op, phase),
        0x40..=0x7F => step_bit(cpu, bus, op, phase),
        0x80..=0xBF => step_res_set(cpu, bus, op, phase, false),
        0xC0..=0xFF => step_res_set(cpu, bus, op, phase, true),
    }
}

fn read_reg(cpu: &Cpu, idx: u8) -> u8 {
    match idx {
        0 => cpu.r.b,
        1 => cpu.r.c,
        2 => cpu.r.d,
        3 => cpu.r.e,
        4 => cpu.r.h,
        5 => cpu.r.l,
        7 => cpu.r.a,
        _ => unreachable!("(HL) read handled separately"),
    }
}

fn write_reg(cpu: &mut Cpu, idx: u8, value: u8) {
    match idx {
        0 => cpu.r.b = value,
        1 => cpu.r.c = value,
        2 => cpu.r.d = value,
        3 => cpu.r.e = value,
        4 => cpu.r.h = value,
        5 => cpu.r.l = value,
        7 => cpu.r.a = value,
        _ => unreachable!("(HL) store handled separately"),
    }
}

fn step_rotate_shift<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    let target = op & 0x07;
    let family = (op >> 3) & 0x07;
    match (target, phase) {
        (6, 0) => cpu.next_cb_phase(op, 1),
        (6, 1) => {
            let value = bus.read_m(cpu.r.hl());
            let result = apply_rotate_shift(cpu, family, value);
            cpu.set_tmp8(result);
            cpu.next_cb_phase(op, 2);
        }
        (6, _) => {
            bus.write_m(cpu.r.hl(), cpu.tmp8());
            cpu.finish();
        }
        (_, _) => {
            let value = read_reg(cpu, target);
            let result = apply_rotate_shift(cpu, family, value);
            write_reg(cpu, target, result);
            cpu.finish();
        }
    }
}

fn apply_rotate_shift(cpu: &mut Cpu, family: u8, value: u8) -> u8 {
    let carry_in = cpu.r.flags().c;
    let (result, flags) = match family {
        0 => alu::rlc(value),
        1 => alu::rrc(value),
        2 => alu::rl(value, carry_in),
        3 => alu::rr(value, carry_in),
        4 => alu::sla(value),
        5 => alu::sra(value),
        6 => alu::swap(value),
        7 => alu::srl(value),
        _ => unreachable!(),
    };
    cpu.r.set_flags(flags);
    result
}

fn step_bit<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    let target = op & 0x07;
    let bit = (op >> 3) & 0x07;
    match (target, phase) {
        (6, 0) => cpu.next_cb_phase(op, 1),
        (6, _) => {
            let value = bus.read_m(cpu.r.hl());
            apply_bit(cpu, value, bit);
            cpu.finish();
        }
        (_, _) => {
            let value = read_reg(cpu, target);
            apply_bit(cpu, value, bit);
            cpu.finish();
        }
    }
}

fn apply_bit(cpu: &mut Cpu, value: u8, bit: u8) {
    let old_c = cpu.r.flags().c;
    let mut flags = alu::bit(value, bit);
    flags.c = old_c;
    cpu.r.set_flags(flags);
}

fn step_res_set<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8, set_bit: bool) {
    let target = op & 0x07;
    let bit = (op >> 3) & 0x07;
    match (target, phase) {
        (6, 0) => cpu.next_cb_phase(op, 1),
        (6, 1) => {
            let value = bus.read_m(cpu.r.hl());
            cpu.set_tmp8(apply_res_set(value, bit, set_bit));
            cpu.next_cb_phase(op, 2);
        }
        (6, _) => {
            bus.write_m(cpu.r.hl(), cpu.tmp8());
            cpu.finish();
        }
        (_, _) => {
            let value = read_reg(cpu, target);
            write_reg(cpu, target, apply_res_set(value, bit, set_bit));
            cpu.finish();
        }
    }
}

fn apply_res_set(value: u8, bit: u8, set_bit: bool) -> u8 {
    if set_bit {
        alu::set(value, bit)
    } else {
        alu::res(value, bit)
    }
}
