use crate::bits::*;
use crate::gameboy::Motherboard;
use crate::globals::*;
use crate::utils::memory_write;

macro_rules! increment_register {
    ($mb:ident, $reg:ident) => {
        let result = $mb.cpu.$reg.wrapping_add(1);

        clear_bit(&mut $mb.cpu.f, BIT_FLAGN);
        clear_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        clear_bit(&mut $mb.cpu.f, BIT_FLAGH);

        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if ($mb.cpu.$reg & 0x0F) == 0x0F {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        $mb.cpu.$reg = result;
        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
    };
}

macro_rules! decrement_register {
    ($mb:ident, $reg:ident) => {
        let result = $mb.cpu.$reg.wrapping_sub(1);

        set_bit(&mut $mb.cpu.f, BIT_FLAGN);
        clear_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        clear_bit(&mut $mb.cpu.f, BIT_FLAGH);

        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if ($mb.cpu.$reg & 0x0F) == 0x0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        $mb.cpu.$reg = result;
        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
    };
}

pub fn init_opcodes() -> OpCodeMap {
    phf::phf_map! {

        // NOP
        0x00u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN
        },

        // LD BC, u16
        0x01u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.b = (value >> 8) as u8;
            mb.cpu.c = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN
        },

        // LD (BC), A
        0x02u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = (mb.cpu.b as u16) << 8 | mb.cpu.c as u16;
            memory_write(addr, mb.cpu.a);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN
        },

        // INC BC
        0x03u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let value = (mb.cpu.b as u16) << 8 | mb.cpu.c as u16;
            let result = value.wrapping_add(1);
            mb.cpu.b = (result >> 8) as u8;
            mb.cpu.c = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN
        },

        // INC B
        0x04u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            increment_register!(mb, b);
            CYCLE_RETURN
        },

        // DEC B
        0x05u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            decrement_register!(mb, b);
            CYCLE_RETURN
        },

        // LD B, u8
        0x06u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.b = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN
        },


        // STOP 0
        0x10u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            // handle CGB stuff here.
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN
        },

        // JR NZ, r8 - Relative jump if last result was not zero
        0x20u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if !is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                let offset = mb.cpu.pc.wrapping_add((value as i8) as u16);
                mb.cpu.pc = offset;
                CYCLE_RETURN * 3
            }else {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN * 2
            }
        },

        // JP, u16 - Absolute jump
        0xC3u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.pc = value;
            CYCLE_RETURN
        },


    }
}
