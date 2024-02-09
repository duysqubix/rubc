use crate::bits;
use crate::gameboy::Motherboard;
use crate::globals::*;

pub fn init_opcodes() -> OpCodeMap {
    phf::phf_map! {

        // NOP
        0x00u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc += 1;
            CYCLE_RETURN
        },

        // LD BC, u16
        0x01u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.b = (value >> 8) as u8;
            mb.cpu.c = value as u8;
            mb.cpu.pc += 3;
            CYCLE_RETURN
        },

        // STOP 0
        0x10u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            // handle CGB stuff here.
            mb.cpu.pc += 2;
            CYCLE_RETURN
        },

        // JR NZ, r8 - Relative jump if last result was not zero
        0x20u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if !bits::is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                let offset = mb.cpu.pc.wrapping_add((value as i8) as u16);
                mb.cpu.pc = offset;
                CYCLE_RETURN * 3
            }else {
                mb.cpu.pc += 2;
                CYCLE_RETURN * 2
            }
        },


    }
}
