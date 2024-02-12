use crate::bits::*;
use crate::gameboy::Motherboard;
use crate::globals::*;
use crate::utils::{memory_read, memory_write};

macro_rules! increment_register {
    ($mb:ident, $reg:ident) => {
        let result = $mb.cpu.$reg.wrapping_add(1);

        crate::clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGZ, BIT_FLAGH);

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
        crate::clear_bits!($mb.cpu.f, BIT_FLAGZ, BIT_FLAGH);

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
            CYCLE_RETURN_4
        },

        // LD BC, u16
        0x01u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.b = (value >> 8) as u8;
            mb.cpu.c = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_4
        },

        // LD (BC), A
        0x02u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = (mb.cpu.b as u16) << 8 | mb.cpu.c as u16;
            memory_write(addr, mb.cpu.a);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // INC BC
        0x03u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let value = (mb.cpu.b as u16) << 8 | mb.cpu.c as u16;
            let result = value.wrapping_add(1);
            mb.cpu.b = (result >> 8) as u8;
            mb.cpu.c = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // INC B
        0x04u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            increment_register!(mb, b);
            CYCLE_RETURN_4
        },

        // DEC B
        0x05u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            decrement_register!(mb, b);
            CYCLE_RETURN_4
        },

        // LD B, u8
        0x06u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.b = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_4
        },

        // RLCA
        0x07u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            crate::clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGZ, BIT_FLAGH);


            match is_bit_set(mb.cpu.a, 7){
                true => {
                    set_bit(&mut mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a = (mb.cpu.a << 1) + 1;
                },
                false => {
                    clear_bit(&mut mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a = mb.cpu.a << 1;
                }
            }
            mb.cpu.pc +=1;
            CYCLE_RETURN_4
        },

        // LD (u16), SP
        0x08u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            memory_write(value, (mb.cpu.sp & 0x00FF) as u8);
            memory_write(value + 1, (mb.cpu.sp >> 8) as u8);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);

            CYCLE_RETURN_20
        },

        // ADD HL, BC
        0x09u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            crate::clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

            let hl = ((mb.cpu.h as u16) << 8 | mb.cpu.l as u16) as u32;
            let bc = ((mb.cpu.b as u16) << 8 | mb.cpu.c as u16) as u32;
            let result = hl + bc;

            if result & 0x10000 != 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGC);
            }

            if (hl ^ bc ^ result) & 0x1000 != 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGH);
            }

            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;

            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (BC)
        0x0Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = (mb.cpu.b as u16) << 8 | mb.cpu.c as u16;
            mb.cpu.a = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DEC BC
        0x0Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let value = (mb.cpu.b as u16) << 8 | mb.cpu.c as u16;
            let result = value.wrapping_sub(1);
            mb.cpu.b = (result >> 8) as u8;
            mb.cpu.c = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC C
        0x0Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            increment_register!(mb, c);
            CYCLE_RETURN_4
        },

        // DEC C
        0x0Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            decrement_register!(mb, c);
            CYCLE_RETURN_4
        },

        // LD C, u8
        0x0Eu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.c = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // RRCA
        0x0Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            crate::clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            match is_bit_set(mb.cpu.a, 0){
                true => {
                    set_bit(&mut mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a = (mb.cpu.a >> 1) | 0x80;
                },
                false => {
                    clear_bit(&mut mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a = mb.cpu.a >> 1;
                }
            }
            mb.cpu.pc +=1;
            CYCLE_RETURN_4
        },


        // STOP 0
        0x10u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            // handle CGB stuff here.
            if mb.cgb_mode {
                let value = memory_read(IO_KEY1);
                if is_bit_set(value, 0){
                    mb.double_speed = !mb.double_speed;
                    memory_write(IO_KEY1, value^0x81);
                }
                memory_write(IO_DIV, 0); //reset timer
            }

            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD DE, u16
        0x11u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.d = (value >> 8) as u8;
            mb.cpu.e = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_12
        },

        // LD (DE), A
        0x12u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = (mb.cpu.d as u16) << 8 | mb.cpu.e as u16;
            memory_write(addr, mb.cpu.a);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC DE
        0x13u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let value = (mb.cpu.d as u16) << 8 | mb.cpu.e as u16;
            let result = value.wrapping_add(1);
            mb.cpu.d = (result >> 8) as u8;
            mb.cpu.e = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC D
        0x14u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            increment_register!(mb, d);
            CYCLE_RETURN_4
        },

        // DEC D
        0x15u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            decrement_register!(mb, d);
            CYCLE_RETURN_4
        },

        // LD D, u8
        0x16u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.d = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // RLA
        0x17u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            crate::clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);

            match is_bit_set(mb.cpu.a, 7){
                true => set_bit(&mut mb.cpu.f, BIT_FLAGC),
                false => clear_bit(&mut mb.cpu.f, BIT_FLAGC),
            }

            mb.cpu.a = (mb.cpu.a << 1)  & 0xff;
            if carry {
                mb.cpu.a |= 1;
            }

            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            CYCLE_RETURN_4
        },

        // JR r8
        0x18u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.pc = (mb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
            CYCLE_RETURN_12
        },

        // ADD HL, DE
        0x19u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            crate::clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

            let hl = ((mb.cpu.h as u16) << 8 | mb.cpu.l as u16) as u32;
            let de = ((mb.cpu.d as u16) << 8 | mb.cpu.e as u16) as u32;
            let result = hl + de;

            if result & 0x10000 != 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGC);
            }

            if (hl ^ de ^ result) & 0x1000 != 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGH);
            }

            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;

            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (DE)
        0x1Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = (mb.cpu.d as u16) << 8 | mb.cpu.e as u16;
            mb.cpu.a = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DEC DE
        0x1Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let value = (mb.cpu.d as u16) << 8 | mb.cpu.e as u16;
            let result = value.wrapping_sub(1);
            mb.cpu.d = (result >> 8) as u8;
            mb.cpu.e = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC E
        0x1Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            increment_register!(mb, e);
            CYCLE_RETURN_4
        },

        // DEC E
        0x1Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            decrement_register!(mb, e);
            CYCLE_RETURN_4
        },

        // LD E, u8
        0x1Eu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.e = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // RRA
        0x1Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            crate::clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);

            match is_bit_set(mb.cpu.a, 0){
                true => set_bit(&mut mb.cpu.f, BIT_FLAGC),
                false => clear_bit(&mut mb.cpu.f, BIT_FLAGC),
            }

            mb.cpu.a = (mb.cpu.a >> 1)  & 0xff;
            if carry {
                mb.cpu.a |= 0x80;
            }

            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            CYCLE_RETURN_4
        },

        // JR NZ, r8 - Relative jump if last result was not zero
        0x20u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if !is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                mb.cpu.pc = (mb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
                CYCLE_RETURN_12
            }else {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN_8
            }
        },

        // // JP, u16 - Absolute jump
        // 0xC3u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
        //     mb.cpu.pc = value;
        //     CYCLE_RETURN
        // },


    }
}
