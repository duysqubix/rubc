//! Main (non-CB) opcode execution, phase by phase.
//!
//! `step(cpu, bus, op, phase)` runs ONE bus M-cycle of opcode `op` at `phase`.
//! Single-M-cycle ops do their work and call `cpu.finish()`. Multi-cycle ops
//! advance via `cpu.next_phase(op, phase + 1)` and resume on the next call.

use super::CpuBus;

use super::{alu, core::Cpu};

/// Execute one M-cycle of main opcode `op` at `phase`.
pub fn step<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match op {
        // NOP
        0x00 => cpu.finish(),

        0x01 | 0x11 | 0x21 | 0x31 => step_ld_rr_d16(cpu, bus, op, phase),
        0x08 => step_ld_a16_sp(cpu, bus, op, phase),

        0x02 | 0x12 | 0x22 | 0x32 | 0x0A | 0x1A | 0x2A | 0x3A => {
            step_ld_indirect_a(cpu, bus, op, phase)
        }

        0x03 | 0x13 | 0x23 | 0x33 => step_inc_dec_rr(cpu, bus, op, phase, false),
        0x0B | 0x1B | 0x2B | 0x3B => step_inc_dec_rr(cpu, bus, op, phase, true),
        0x09 | 0x19 | 0x29 | 0x39 => step_add_hl_rr(cpu, bus, op, phase),

        0x07 | 0x0F | 0x17 | 0x1F => step_acc_rotate(cpu, op),

        0x10 => step_stop(cpu, bus, phase),
        0x18 => step_jr(cpu, bus, op, phase, true),
        0x20 | 0x28 | 0x30 | 0x38 => step_jr_cc(cpu, bus, op, phase),

        0x27 => step_daa(cpu),
        0x2F => step_cpl(cpu),
        0x37 => step_scf(cpu),
        0x3F => step_ccf(cpu),

        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
            step_inc_dec_r(cpu, bus, op, phase, false)
        }

        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
            step_inc_dec_r(cpu, bus, op, phase, true)
        }

        0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => step_ld_r_d8(cpu, bus, op, phase),

        0xC0 | 0xC8 | 0xD0 | 0xD8 => step_ret_cc(cpu, bus, op, phase),
        0xC1 | 0xD1 | 0xE1 | 0xF1 => step_pop_rr(cpu, bus, op, phase),
        0xC2 | 0xCA | 0xD2 | 0xDA => step_jp_cc_a16(cpu, bus, op, phase),
        0xC3 => step_jp_a16(cpu, bus, op, phase, true),
        0xC4 | 0xCC | 0xD4 | 0xDC => step_call_cc_a16(cpu, bus, op, phase),
        0xC5 | 0xD5 | 0xE5 | 0xF5 => step_push_rr(cpu, bus, op, phase),
        0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => step_rst(cpu, bus, op, phase),
        0xC9 => step_ret(cpu, bus, op, phase, false),
        0xCB => step_cb_prefix(cpu, bus, phase),
        0xCD => step_call_a16(cpu, bus, op, phase, true),
        0xD9 => step_ret(cpu, bus, op, phase, true),

        0x80..=0xBF => step_alu_a_operand(cpu, bus, op, phase),

        0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => step_alu_a_d8(cpu, bus, op, phase),

        0xE0 | 0xF0 => step_ldh_a8_a(cpu, bus, op, phase),
        0xE2 | 0xF2 => step_ldh_c_a(cpu, bus, op, phase),
        0xE8 => step_add_sp_e8(cpu, bus, op, phase),
        0xE9 => {
            cpu.r.pc = cpu.r.hl();
            cpu.finish();
        }
        0xEA | 0xFA => step_ld_a16_a(cpu, bus, op, phase),
        0xF3 => {
            cpu.di();
            cpu.finish();
        }
        0xF8 => step_ld_hl_sp_e8(cpu, bus, op, phase),
        0xF9 => step_ld_sp_hl(cpu, bus, op, phase),
        0xFB => {
            cpu.schedule_ei();
            cpu.finish();
        }

        0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => cpu.finish(),

        // LD r,r' and LD r,(HL) / LD (HL),r are handled by a decoded table.
        _ => step_ld_block(cpu, bus, op, phase),
    }
}

/// The `0x40..=0x7F` LD block (register-to-register, (HL) loads/stores, HALT).
fn step_ld_block<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    // 0x76 is HALT, the hole in the LD block.
    if op == 0x76 {
        cpu.enter_halt(bus);
        return;
    }
    if (0x40..=0x7F).contains(&op) {
        let dst = (op >> 3) & 0x07;
        let src = op & 0x07;
        ld_reg_reg(cpu, bus, dst, src, phase);
        return;
    }
    cpu.finish();
}

fn read_pc<B: CpuBus>(cpu: &mut Cpu, bus: &mut B) -> u8 {
    let v = bus.read_m(cpu.r.pc);
    cpu.r.pc = cpu.r.pc.wrapping_add(1);
    v
}

fn read_rr(cpu: &Cpu, group: u8) -> u16 {
    match group {
        0 => cpu.r.bc(),
        1 => cpu.r.de(),
        2 => cpu.r.hl(),
        3 => cpu.r.sp,
        _ => unreachable!(),
    }
}

fn write_rr(cpu: &mut Cpu, group: u8, value: u16) {
    match group {
        0 => cpu.r.set_bc(value),
        1 => cpu.r.set_de(value),
        2 => cpu.r.set_hl(value),
        3 => cpu.r.sp = value,
        _ => unreachable!(),
    }
}

fn read_stack_rr(cpu: &Cpu, group: u8) -> u16 {
    match group {
        0 => cpu.r.bc(),
        1 => cpu.r.de(),
        2 => cpu.r.hl(),
        3 => cpu.r.af(),
        _ => unreachable!(),
    }
}

fn write_stack_rr(cpu: &mut Cpu, group: u8, value: u16) {
    match group {
        0 => cpu.r.set_bc(value),
        1 => cpu.r.set_de(value),
        2 => cpu.r.set_hl(value),
        3 => cpu.r.set_af(value),
        _ => unreachable!(),
    }
}

fn condition(cpu: &Cpu, op: u8) -> bool {
    let flags = cpu.r.flags();
    match op {
        0x20 | 0xC0 | 0xC2 | 0xC4 => !flags.z,
        0x28 | 0xC8 | 0xCA | 0xCC => flags.z,
        0x30 | 0xD0 | 0xD2 | 0xD4 => !flags.c,
        0x38 | 0xD8 | 0xDA | 0xDC => flags.c,
        _ => unreachable!(),
    }
}

fn step_ld_rr_d16<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let lo = read_pc(cpu, bus);
            cpu.set_tmp8(lo);
            cpu.next_phase(op, 2);
        }
        _ => {
            let hi = read_pc(cpu, bus);
            let value = u16::from_le_bytes([cpu.tmp8(), hi]);
            write_rr(cpu, (op >> 4) & 0x03, value);
            cpu.finish();
        }
    }
}

fn step_ld_a16_sp<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let lo = read_pc(cpu, bus);
            cpu.set_tmp8(lo);
            cpu.next_phase(op, 2);
        }
        2 => {
            let hi = read_pc(cpu, bus);
            cpu.set_tmp16(u16::from_le_bytes([cpu.tmp8(), hi]));
            cpu.next_phase(op, 3);
        }
        3 => {
            bus.write_m(cpu.tmp16(), cpu.r.sp as u8);
            cpu.next_phase(op, 4);
        }
        _ => {
            bus.write_m(cpu.tmp16().wrapping_add(1), (cpu.r.sp >> 8) as u8);
            cpu.finish();
        }
    }
}

fn step_ld_sp_hl<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        _ => {
            bus.idle_m();
            cpu.r.sp = cpu.r.hl();
            cpu.finish();
        }
    }
}

fn step_ld_hl_sp_e8<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let e8 = read_pc(cpu, bus) as i8;
            let (result, flags) = alu::add_sp_e8(cpu.r.sp, e8);
            cpu.set_tmp16(result);
            cpu.r.set_flags(flags);
            cpu.next_phase(op, 2);
        }
        _ => {
            bus.idle_m();
            cpu.r.set_hl(cpu.tmp16());
            cpu.finish();
        }
    }
}

fn step_ld_indirect_a<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        _ => {
            match op {
                0x02 => bus.write_m(cpu.r.bc(), cpu.r.a),
                0x12 => bus.write_m(cpu.r.de(), cpu.r.a),
                0x22 => {
                    let addr = cpu.r.hl();
                    bus.write_m(addr, cpu.r.a);
                    cpu.r.set_hl(addr.wrapping_add(1));
                }
                0x32 => {
                    let addr = cpu.r.hl();
                    bus.write_m(addr, cpu.r.a);
                    cpu.r.set_hl(addr.wrapping_sub(1));
                }
                0x0A => cpu.r.a = bus.read_m(cpu.r.bc()),
                0x1A => cpu.r.a = bus.read_m(cpu.r.de()),
                0x2A => {
                    let addr = cpu.r.hl();
                    cpu.r.a = bus.read_m_oam_bug_idu(addr);
                    cpu.r.set_hl(addr.wrapping_add(1));
                }
                0x3A => {
                    let addr = cpu.r.hl();
                    cpu.r.a = bus.read_m_oam_bug_idu(addr);
                    cpu.r.set_hl(addr.wrapping_sub(1));
                }
                _ => unreachable!(),
            }
            cpu.finish();
        }
    }
}

fn step_ldh_a8_a<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let addr = 0xFF00 | read_pc(cpu, bus) as u16;
            cpu.set_tmp16(addr);
            cpu.next_phase(op, 2);
        }
        _ => {
            if op == 0xE0 {
                bus.write_m(cpu.tmp16(), cpu.r.a);
            } else {
                cpu.r.a = bus.read_m(cpu.tmp16());
            }
            cpu.finish();
        }
    }
}

fn step_ldh_c_a<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        _ => {
            let addr = 0xFF00 | cpu.r.c as u16;
            if op == 0xE2 {
                bus.write_m(addr, cpu.r.a);
            } else {
                cpu.r.a = bus.read_m(addr);
            }
            cpu.finish();
        }
    }
}

fn step_ld_a16_a<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let lo = read_pc(cpu, bus);
            cpu.set_tmp8(lo);
            cpu.next_phase(op, 2);
        }
        2 => {
            let hi = read_pc(cpu, bus);
            cpu.set_tmp16(u16::from_le_bytes([cpu.tmp8(), hi]));
            cpu.next_phase(op, 3);
        }
        _ => {
            if op == 0xEA {
                bus.write_m(cpu.tmp16(), cpu.r.a);
            } else {
                cpu.r.a = bus.read_m(cpu.tmp16());
            }
            cpu.finish();
        }
    }
}

fn step_inc_dec_rr<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8, dec: bool) {
    match phase {
        0 => cpu.next_phase(op, 1),
        _ => {
            let group = (op >> 4) & 0x03;
            let old = read_rr(cpu, group);
            bus.oam_bug_idu_m(old);
            let value = if dec {
                old.wrapping_sub(1)
            } else {
                old.wrapping_add(1)
            };
            write_rr(cpu, group, value);
            cpu.finish();
        }
    }
}

fn step_add_hl_rr<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        _ => {
            bus.idle_m();
            let old_z = cpu.r.flags().z;
            let (result, mut flags) = alu::add16(cpu.r.hl(), read_rr(cpu, (op >> 4) & 0x03));
            flags.z = old_z;
            cpu.r.set_hl(result);
            cpu.r.set_flags(flags);
            cpu.finish();
        }
    }
}

fn step_add_sp_e8<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let e8 = read_pc(cpu, bus) as i8;
            let (result, flags) = alu::add_sp_e8(cpu.r.sp, e8);
            cpu.set_tmp16(result);
            cpu.r.set_flags(flags);
            cpu.next_phase(op, 2);
        }
        2 => {
            bus.idle_m();
            cpu.next_phase(op, 3);
        }
        _ => {
            bus.idle_m();
            cpu.r.sp = cpu.tmp16();
            cpu.finish();
        }
    }
}

fn step_acc_rotate(cpu: &mut Cpu, op: u8) {
    let carry = cpu.r.flags().c;
    let (result, mut flags) = match op {
        0x07 => alu::rlc(cpu.r.a),
        0x0F => alu::rrc(cpu.r.a),
        0x17 => alu::rl(cpu.r.a, carry),
        0x1F => alu::rr(cpu.r.a, carry),
        _ => unreachable!(),
    };
    flags.z = false;
    cpu.r.a = result;
    cpu.r.set_flags(flags);
    cpu.finish();
}

fn step_daa(cpu: &mut Cpu) {
    let flags = cpu.r.flags();
    let (result, flags) = alu::daa(cpu.r.a, flags.n, flags.h, flags.c);
    cpu.r.a = result;
    cpu.r.set_flags(flags);
    cpu.finish();
}

fn step_cpl(cpu: &mut Cpu) {
    let mut flags = cpu.r.flags();
    cpu.r.a = alu::cpl(cpu.r.a);
    flags.n = true;
    flags.h = true;
    cpu.r.set_flags(flags);
    cpu.finish();
}

fn step_scf(cpu: &mut Cpu) {
    let mut flags = cpu.r.flags();
    flags.n = false;
    flags.h = false;
    flags.c = true;
    cpu.r.set_flags(flags);
    cpu.finish();
}

fn step_ccf(cpu: &mut Cpu) {
    let mut flags = cpu.r.flags();
    flags.n = false;
    flags.h = false;
    flags.c = !flags.c;
    cpu.r.set_flags(flags);
    cpu.finish();
}

fn step_stop<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, phase: u8) {
    match phase {
        0 => {
            // STOP is a 2-byte opcode (0x10 0x00); consume the second byte.
            let _ = read_pc(cpu, bus);
            if bus.speed_switch_armed() {
                // CGB KEY1 speed switch: toggle the clock and RESUME execution
                // (real hardware does not halt here). This is the sequence the
                // combined cpu_instrs runner uses to enter double-speed.
                //
                // TIMING TODO (rubc-te2 CGB core): hardware stalls ~2050 M-cycles
                // during the switch (Pan Docs CGB_Registers KEY1). We resume
                // immediately for now; this is NOT yet cycle-accurate for the
                // switch latency. Tracked by the CGB-core wave, not this fix.
                bus.finish_speed_switch();
                cpu.finish();
            } else {
                cpu.enter_stop();
            }
        }
        _ => cpu.finish(),
    }
}

fn step_cb_prefix<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, phase: u8) {
    match phase {
        0 => {
            let cb = read_pc(cpu, bus);
            cpu.begin_cb(cb);
        }
        _ => cpu.finish(),
    }
}

fn step_jr_cc<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    let taken = condition(cpu, op);
    step_jr(cpu, bus, op, phase, taken);
}

fn step_jr<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8, taken: bool) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let offset = read_pc(cpu, bus) as i8;
            if taken {
                let target = cpu.r.pc.wrapping_add(offset as i16 as u16);
                cpu.set_tmp16(target);
                cpu.next_phase(op, 2);
            } else {
                cpu.finish();
            }
        }
        _ => {
            bus.idle_m();
            cpu.r.pc = cpu.tmp16();
            cpu.finish();
        }
    }
}

fn step_jp_cc_a16<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    let taken = condition(cpu, op);
    step_jp_a16(cpu, bus, op, phase, taken);
}

fn step_jp_a16<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8, taken: bool) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let lo = read_pc(cpu, bus);
            cpu.set_tmp8(lo);
            cpu.next_phase(op, 2);
        }
        2 => {
            let hi = read_pc(cpu, bus);
            let target = u16::from_le_bytes([cpu.tmp8(), hi]);
            if taken {
                cpu.set_tmp16(target);
                cpu.next_phase(op, 3);
            } else {
                cpu.finish();
            }
        }
        _ => {
            bus.idle_m();
            cpu.r.pc = cpu.tmp16();
            cpu.finish();
        }
    }
}

fn step_call_cc_a16<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    let taken = condition(cpu, op);
    step_call_a16(cpu, bus, op, phase, taken);
}

fn step_call_a16<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8, taken: bool) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let lo = read_pc(cpu, bus);
            cpu.set_tmp8(lo);
            cpu.next_phase(op, 2);
        }
        2 => {
            let hi = read_pc(cpu, bus);
            let target = u16::from_le_bytes([cpu.tmp8(), hi]);
            if taken {
                cpu.set_tmp16(target);
                cpu.next_phase(op, 3);
            } else {
                cpu.finish();
            }
        }
        3 => {
            let old_sp = cpu.r.sp;
            bus.oam_bug_idu_m(old_sp);
            cpu.r.sp = old_sp.wrapping_sub(1);
            cpu.next_phase(op, 4);
        }
        4 => {
            bus.write_m(cpu.r.sp, (cpu.r.pc >> 8) as u8);
            cpu.r.sp = cpu.r.sp.wrapping_sub(1);
            cpu.next_phase(op, 5);
        }
        _ => {
            bus.write_m(cpu.r.sp, cpu.r.pc as u8);
            cpu.r.pc = cpu.tmp16();
            cpu.finish();
        }
    }
}

fn step_ret_cc<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            bus.idle_m();
            if condition(cpu, op) {
                cpu.next_phase(op, 2);
            } else {
                cpu.finish();
            }
        }
        2 => {
            let lo = bus.read_m_oam_bug_idu(cpu.r.sp);
            cpu.r.sp = cpu.r.sp.wrapping_add(1);
            cpu.set_tmp8(lo);
            cpu.next_phase(op, 3);
        }
        3 => {
            let hi = bus.read_m(cpu.r.sp);
            cpu.r.sp = cpu.r.sp.wrapping_add(1);
            cpu.set_tmp16(u16::from_le_bytes([cpu.tmp8(), hi]));
            cpu.next_phase(op, 4);
        }
        _ => {
            bus.idle_m();
            cpu.r.pc = cpu.tmp16();
            cpu.finish();
        }
    }
}

fn step_ret<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8, reti: bool) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let lo = bus.read_m_oam_bug_idu(cpu.r.sp);
            cpu.r.sp = cpu.r.sp.wrapping_add(1);
            cpu.set_tmp8(lo);
            cpu.next_phase(op, 2);
        }
        2 => {
            let hi = bus.read_m(cpu.r.sp);
            cpu.r.sp = cpu.r.sp.wrapping_add(1);
            cpu.set_tmp16(u16::from_le_bytes([cpu.tmp8(), hi]));
            cpu.next_phase(op, 3);
        }
        _ => {
            bus.idle_m();
            cpu.r.pc = cpu.tmp16();
            if reti {
                cpu.set_ime_now(true);
            }
            cpu.finish();
        }
    }
}

fn step_rst<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let old_sp = cpu.r.sp;
            bus.oam_bug_idu_m(old_sp);
            cpu.r.sp = old_sp.wrapping_sub(1);
            cpu.next_phase(op, 2);
        }
        2 => {
            bus.write_m(cpu.r.sp, (cpu.r.pc >> 8) as u8);
            cpu.r.sp = cpu.r.sp.wrapping_sub(1);
            cpu.next_phase(op, 3);
        }
        _ => {
            bus.write_m(cpu.r.sp, cpu.r.pc as u8);
            cpu.r.pc = (op & 0x38) as u16;
            cpu.finish();
        }
    }
}

fn step_push_rr<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let old_sp = cpu.r.sp;
            bus.oam_bug_idu_m(old_sp);
            cpu.r.sp = old_sp.wrapping_sub(1);
            cpu.next_phase(op, 2);
        }
        2 => {
            let value = read_stack_rr(cpu, (op >> 4) & 0x03);
            bus.write_m(cpu.r.sp, (value >> 8) as u8);
            cpu.r.sp = cpu.r.sp.wrapping_sub(1);
            cpu.next_phase(op, 3);
        }
        _ => {
            let value = read_stack_rr(cpu, (op >> 4) & 0x03);
            bus.write_m(cpu.r.sp, value as u8);
            cpu.finish();
        }
    }
}

fn step_pop_rr<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        1 => {
            let lo = bus.read_m_oam_bug_idu(cpu.r.sp);
            cpu.r.sp = cpu.r.sp.wrapping_add(1);
            cpu.set_tmp8(lo);
            cpu.next_phase(op, 2);
        }
        _ => {
            let hi = bus.read_m(cpu.r.sp);
            cpu.r.sp = cpu.r.sp.wrapping_add(1);
            let value = u16::from_le_bytes([cpu.tmp8(), hi]);
            write_stack_rr(cpu, (op >> 4) & 0x03, value);
            cpu.finish();
        }
    }
}

/// Operand index 0..=7 maps to B,C,D,E,H,L,(HL),A.
fn read_operand<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, idx: u8, phase: u8) -> Option<u8> {
    match idx {
        0 => Some(cpu.r.b),
        1 => Some(cpu.r.c),
        2 => Some(cpu.r.d),
        3 => Some(cpu.r.e),
        4 => Some(cpu.r.h),
        5 => Some(cpu.r.l),
        7 => Some(cpu.r.a),
        6 => {
            // (HL): needs a memory M-cycle. Caller manages phase; here we only
            // read when phase indicates the read cycle is due.
            let _ = phase;
            Some(bus.read_m(cpu.r.hl()))
        }
        _ => unreachable!(),
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

fn step_ld_r_d8<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    let dst = (op >> 3) & 0x07;
    match (dst, phase) {
        (_, 0) => cpu.next_phase(op, 1),
        (6, 1) => {
            let v = bus.read_m(cpu.r.pc);
            cpu.r.pc = cpu.r.pc.wrapping_add(1);
            cpu.set_tmp8(v);
            cpu.next_phase(op, 2);
        }
        (6, _) => {
            bus.write_m(cpu.r.hl(), cpu.tmp8());
            cpu.finish();
        }
        (_, _) => {
            let v = bus.read_m(cpu.r.pc);
            cpu.r.pc = cpu.r.pc.wrapping_add(1);
            write_reg(cpu, dst, v);
            cpu.finish();
        }
    }
}

fn step_inc_dec_r<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8, dec: bool) {
    let idx = (op >> 3) & 0x07;
    match (idx, phase) {
        (6, 0) => cpu.next_phase(op, 1),
        (6, 1) => {
            let v = bus.read_m(cpu.r.hl());
            let result = apply_inc_dec(cpu, v, dec);
            cpu.set_tmp8(result);
            cpu.next_phase(op, 2);
        }
        (6, _) => {
            bus.write_m(cpu.r.hl(), cpu.tmp8());
            cpu.finish();
        }
        (_, _) => {
            let v = read_reg(cpu, idx);
            let result = apply_inc_dec(cpu, v, dec);
            write_reg(cpu, idx, result);
            cpu.finish();
        }
    }
}

fn step_alu_a_operand<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    let src = op & 0x07;
    match (src, phase) {
        (6, 0) => cpu.next_phase(op, 1),
        (6, _) => {
            let v = bus.read_m(cpu.r.hl());
            apply_alu_a(cpu, (op >> 3) & 0x07, v);
            cpu.finish();
        }
        (_, _) => {
            let v = read_reg(cpu, src);
            apply_alu_a(cpu, (op >> 3) & 0x07, v);
            cpu.finish();
        }
    }
}

fn step_alu_a_d8<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, op: u8, phase: u8) {
    match phase {
        0 => cpu.next_phase(op, 1),
        _ => {
            let v = bus.read_m(cpu.r.pc);
            cpu.r.pc = cpu.r.pc.wrapping_add(1);
            apply_alu_a(cpu, alu_d8_family(op), v);
            cpu.finish();
        }
    }
}

fn alu_d8_family(op: u8) -> u8 {
    match op {
        0xC6 => 0,
        0xCE => 1,
        0xD6 => 2,
        0xDE => 3,
        0xE6 => 4,
        0xEE => 5,
        0xF6 => 6,
        0xFE => 7,
        _ => unreachable!(),
    }
}

fn apply_inc_dec(cpu: &mut Cpu, value: u8, dec: bool) -> u8 {
    let old_c = cpu.r.flags().c;
    let (result, mut flags) = if dec {
        alu::dec8(value)
    } else {
        alu::inc8(value)
    };
    flags.c = old_c;
    cpu.r.set_flags(flags);
    result
}

fn apply_alu_a(cpu: &mut Cpu, family: u8, value: u8) {
    match family {
        0 => {
            let (result, flags) = alu::add8(cpu.r.a, value, false);
            cpu.r.a = result;
            cpu.r.set_flags(flags);
        }
        1 => {
            let carry = cpu.r.flags().c;
            let (result, flags) = alu::add8(cpu.r.a, value, carry);
            cpu.r.a = result;
            cpu.r.set_flags(flags);
        }
        2 => {
            let (result, flags) = alu::sub8(cpu.r.a, value, false);
            cpu.r.a = result;
            cpu.r.set_flags(flags);
        }
        3 => {
            let carry = cpu.r.flags().c;
            let (result, flags) = alu::sub8(cpu.r.a, value, carry);
            cpu.r.a = result;
            cpu.r.set_flags(flags);
        }
        4 => {
            let (result, flags) = alu::and8(cpu.r.a, value);
            cpu.r.a = result;
            cpu.r.set_flags(flags);
        }
        5 => {
            let (result, flags) = alu::xor8(cpu.r.a, value);
            cpu.r.a = result;
            cpu.r.set_flags(flags);
        }
        6 => {
            let (result, flags) = alu::or8(cpu.r.a, value);
            cpu.r.a = result;
            cpu.r.set_flags(flags);
        }
        7 => {
            let flags = alu::cp8(cpu.r.a, value);
            cpu.r.set_flags(flags);
        }
        _ => unreachable!(),
    }
}

/// `LD dst, src` where either side may be `(HL)` (index 6).
fn ld_reg_reg<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, dst: u8, src: u8, phase: u8) {
    match (dst, src) {
        // register <- register: 1 M-cycle (the fetch).
        (d, s) if d != 6 && s != 6 => {
            let v = read_operand(cpu, bus, s, phase).unwrap();
            write_reg(cpu, d, v);
            cpu.finish();
        }
        // register <- (HL): 2 M-cycles (fetch + read).
        (d, 6) => match phase {
            0 => cpu.next_phase(op_ld_r_hl(d), 1),
            _ => {
                let v = bus.read_m(cpu.r.hl());
                write_reg(cpu, d, v);
                cpu.finish();
            }
        },
        // (HL) <- register: 2 M-cycles (fetch + write).
        (6, s) => match phase {
            0 => cpu.next_phase(op_ld_hl_r(s), 1),
            _ => {
                let v = match s {
                    0 => cpu.r.b,
                    1 => cpu.r.c,
                    2 => cpu.r.d,
                    3 => cpu.r.e,
                    4 => cpu.r.h,
                    5 => cpu.r.l,
                    7 => cpu.r.a,
                    _ => unreachable!(),
                };
                bus.write_m(cpu.r.hl(), v);
                cpu.finish();
            }
        },
        _ => unreachable!(),
    }
}

// Reconstruct the opcode byte so `next_phase` resumes the same instruction.
fn op_ld_r_hl(dst: u8) -> u8 {
    0x40 | (dst << 3) | 0x06
}
fn op_ld_hl_r(src: u8) -> u8 {
    0x40 | (0x06 << 3) | src
}
