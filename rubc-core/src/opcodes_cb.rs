use crate::{bits::*, gameboy::Gameboy, globals::*};

pub fn init_opcodes_cb() -> OpCodeMap {
    phf::phf_map! {
        // RLC B
        0x00u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.b, 7){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.b = (gb.cpu.b <<1) + 0x01;
                },
                false => {
                    gb.cpu.b <<= 1;
                }
            }

            if gb.cpu.b == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC C
        0x01u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.c, 7){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.c = (gb.cpu.c <<1) + 0x01;
                },
                false => {
                    gb.cpu.c <<= 1;
                }
            }

            if gb.cpu.c == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC D
        0x02u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.d, 7){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.d = (gb.cpu.d <<1) + 0x01;
                },
                false => {
                    gb.cpu.d <<= 1;
                }
            }

            if gb.cpu.d == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC E
        0x03u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.e, 7){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.e = (gb.cpu.e <<1) + 0x01;
                },
                false => {
                    gb.cpu.e <<= 1;
                }
            }

            if gb.cpu.e == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC H
        0x04u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.h, 7){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.h = (gb.cpu.h <<1) + 0x01;
                },
                false => {
                    gb.cpu.h <<= 1;
                }
            }

            if gb.cpu.h == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC L
        0x05u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.l, 7){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.l = (gb.cpu.l <<1) + 0x01;
                },
                false => {
                    gb.cpu.l <<= 1;
                }
            }

            if gb.cpu.l == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC (HL)
        0x06u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(value, 7){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.memory_write(hl, (value <<1) + 0x01);
                },
                false => {
                    gb.memory_write(hl, value << 1);
                }
            }

            if value == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // RLC A
        0x07u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.a, 7){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.a = (gb.cpu.a <<1) + 0x01;
                },
                false => {
                    gb.cpu.a <<= 1;
                }
            }

            if gb.cpu.a == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC B
        0x08u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.b, 0){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.b = (gb.cpu.b >>1) + 0x80;
                },
                false => {
                    gb.cpu.b >>= 1;
                }
            }

            if gb.cpu.b == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC C
        0x09u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.c, 0){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.c = (gb.cpu.c >>1) + 0x80;
                },
                false => {
                    gb.cpu.c >>= 1;
                }
            }

            if gb.cpu.c == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC D
        0x0Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.d, 0){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.d = (gb.cpu.d >>1) + 0x80;
                },
                false => {
                    gb.cpu.d >>= 1;
                }
            }

            if gb.cpu.d == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC E
        0x0Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.e, 0){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.e = (gb.cpu.e >>1) + 0x80;
                },
                false => {
                    gb.cpu.e >>= 1;
                }
            }

            if gb.cpu.e == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC H
        0x0Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.h, 0){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.h = (gb.cpu.h >>1) + 0x80;
                },
                false => {
                    gb.cpu.h >>= 1;
                }
            }

            if gb.cpu.h == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC L
        0x0Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.l, 0){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.l = (gb.cpu.l >>1) + 0x80;
                },
                false => {
                    gb.cpu.l >>= 1;
                }
            }

            if gb.cpu.l == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC (HL)
        0x0Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(value, 0){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.memory_write(hl, (value >>1) + 0x80);
                },
                false => {
                    gb.memory_write(hl, value >> 1);
                }
            }

            if value == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // RRC A
        0x0Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(gb.cpu.a, 0){
                true => {
                    set_bits!(gb.cpu.f, BIT_FLAGC);
                    gb.cpu.a = (gb.cpu.a >>1) + 0x80;
                },
                false => {
                    gb.cpu.a >>= 1;
                }
            }

            if gb.cpu.a == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL B
        0x10u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.b, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.b <<= 1;

            if carry {
                gb.cpu.b |= 0x01;
            }

            if gb.cpu.b == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL C
        0x11u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.c, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.c <<= 1;

            if carry {
                gb.cpu.c |= 0x01;
            }

            if gb.cpu.c == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL D
        0x12u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.d, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.d <<= 1;

            if carry {
                gb.cpu.d |= 0x01;
            }

            if gb.cpu.d == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL E
        0x13u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.e, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.e <<= 1;

            if carry {
                gb.cpu.e |= 0x01;
            }

            if gb.cpu.e == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL H
        0x14u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.h, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.h <<= 1;

            if carry {
                gb.cpu.h |= 0x01;
            }

            if gb.cpu.h == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL L
        0x15u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.l, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.l <<= 1;

            if carry {
                gb.cpu.l |= 0x01;
            }

            if gb.cpu.l == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL (HL)
        0x16u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(value, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            let mut result = value << 1;

            if carry {
                result |= 0x01;
            }

            gb.memory_write(hl, result);

            if result == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // RL A
        0x17u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.a, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.a <<= 1;

            if carry {
                gb.cpu.a |= 0x01;
            }

            if gb.cpu.a == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR B
        0x18u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.b, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.b >>= 1;

            if carry {
                gb.cpu.b |= 0x80;
            }

            if gb.cpu.b == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR C
        0x19u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.c, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.c >>= 1;

            if carry {
                gb.cpu.c |= 0x80;
            }

            if gb.cpu.c == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR D
        0x1Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.d, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.d >>= 1;

            if carry {
                gb.cpu.d |= 0x80;
            }

            if gb.cpu.d == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR E
        0x1Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.e, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.e >>= 1;

            if carry {
                gb.cpu.e |= 0x80;
            }

            if gb.cpu.e == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR H
        0x1Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.h, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.h >>= 1;

            if carry {
                gb.cpu.h |= 0x80;
            }

            if gb.cpu.h == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR L
        0x1Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.l, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.l >>= 1;

            if carry {
                gb.cpu.l |= 0x80;
            }

            if gb.cpu.l == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR (HL)
        0x1Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(value, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            let mut result = value >> 1;

            if carry {
                result |= 0x80;
            }

            gb.memory_write(hl, result);

            if result == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // RR A
        0x1Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.a, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.a >>= 1;

            if carry {
                gb.cpu.a |= 0x80;
            }

            if gb.cpu.a == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA B
        0x20u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.b, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.b <<= 1;

            if gb.cpu.b == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA C
        0x21u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.c, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.c <<= 1;

            if gb.cpu.c == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA D
        0x22u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.d, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.d <<= 1;

            if gb.cpu.d == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA E
        0x23u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.e, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.e <<= 1;

            if gb.cpu.e == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA H
        0x24u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.h, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.h <<= 1;

            if gb.cpu.h == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA L
        0x25u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.l, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.l <<= 1;

            if gb.cpu.l == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA (HL)
        0x26u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(value, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            let result = value << 1;

            gb.memory_write(hl, result);

            if result == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // SLA A
        0x27u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.a, 7){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.a <<= 1;

            if gb.cpu.a == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA B
        0x28u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.b, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            // gb.cpu.b = gb.cpu.b.rotate_right(1);

            match is_bit_set(gb.cpu.b, 7){
                true => {
                    gb.cpu.b = (gb.cpu.b >> 1) | 0x80;
                },
                false => {
                    gb.cpu.b >>= 1;
                }
            }

            if gb.cpu.b == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA C
        0x29u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.c, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            // gb.cpu.c = gb.cpu.c.rotate_right(1);

            match is_bit_set(gb.cpu.c, 7){
                true => {
                    gb.cpu.c = (gb.cpu.c >> 1) | 0x80;
                },
                false => {
                    gb.cpu.c >>= 1;
                }
            }

            if gb.cpu.c == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA D
        0x2Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.d, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            // gb.cpu.d = gb.cpu.d.rotate_right(1);

            match is_bit_set(gb.cpu.d, 7){
                true => {
                    gb.cpu.d = (gb.cpu.d >> 1) | 0x80;
                },
                false => {
                    gb.cpu.d >>= 1;
                }
            }

            if gb.cpu.d == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA E
        0x2Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.e, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            // gb.cpu.e = gb.cpu.e.rotate_right(1);

            match is_bit_set(gb.cpu.e, 7){
                true => {
                    gb.cpu.e = (gb.cpu.e >> 1) | 0x80;
                },
                false => {
                    gb.cpu.e >>= 1;
                }
            }

            if gb.cpu.e == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA H
        0x2Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.h, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            // gb.cpu.h = gb.cpu.h.rotate_right(1);

            match is_bit_set(gb.cpu.h, 7){
                true => {
                    gb.cpu.h = (gb.cpu.h >> 1) | 0x80;
                },
                false => {
                    gb.cpu.h >>= 1;
                }
            }

            if gb.cpu.h == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA L
        0x2Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.l, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            // gb.cpu.l = gb.cpu.l.rotate_right(1);

            match is_bit_set(gb.cpu.l, 7){
                true => {
                    gb.cpu.l = (gb.cpu.l >> 1) | 0x80;
                },
                false => {
                    gb.cpu.l >>= 1;
                }
            }

            if gb.cpu.l == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA (HL)
        0x2Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  value&0x01 != 0{
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            if value&0x80 != 0{
                value = (value >> 1) | 0x80;
            } else {
                value >>= 1;
            }

            if value == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.memory_write(hl, value);

            CYCLE_RETURN_16
        },

        // SRA A
        0x2Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(gb.cpu.a, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            // gb.cpu.a = gb.cpu.a.rotate_right(1);

            match is_bit_set(gb.cpu.a, 7){
                true => {
                    gb.cpu.a = (gb.cpu.a >> 1) | 0x80;
                },
                false => {
                    gb.cpu.a >>= 1;
                }
            }

            if gb.cpu.a == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SWAP B
        0x30u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut b = gb.cpu.b;
            b = (b >> 4) | (b << 4);

            if b == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.cpu.b = b;

            CYCLE_RETURN_8
        },

        // SWAP C
        0x31u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut c = gb.cpu.c;
            c = (c >> 4) | (c << 4);

            if c == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.cpu.c = c;

            CYCLE_RETURN_8
        },

        // SWAP D
        0x32u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut d = gb.cpu.d;
            d = (d >> 4) | (d << 4);

            if d == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.cpu.d = d;

            CYCLE_RETURN_8
        },

        // SWAP E
        0x33u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut e = gb.cpu.e;
            e = (e >> 4) | (e << 4);

            if e == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.cpu.e = e;

            CYCLE_RETURN_8
        },

        // SWAP H
        0x34u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut h = gb.cpu.h;
            h = (h >> 4) | (h << 4);

            if h == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.cpu.h = h;

            CYCLE_RETURN_8
        },

        // SWAP L
        0x35u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut l = gb.cpu.l;
            l = (l >> 4) | (l << 4);

            if l == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.cpu.l = l;

            CYCLE_RETURN_8
        },

        // SWAP (HL)
        0x36u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            value = (value >> 4) | (value << 4);

            if value == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.memory_write(hl, value);

            CYCLE_RETURN_16
        },

        // SWAP A
        0x37u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut a = gb.cpu.a;
            a = (a >> 4) | (a << 4);

            if a == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.cpu.a = a;

            CYCLE_RETURN_8
        },

        // SRL B
        0x38u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(gb.cpu.b, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.b >>= 1;

            if gb.cpu.b == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL C
        0x39u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(gb.cpu.c, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.c >>= 1;

            if gb.cpu.c == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL D
        0x3Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(gb.cpu.d, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.d >>= 1;

            if gb.cpu.d == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL E
        0x3Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(gb.cpu.e, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.e >>= 1;

            if gb.cpu.e == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL H
        0x3Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(gb.cpu.h, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.h >>= 1;

            if gb.cpu.h == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL L
        0x3Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(gb.cpu.l, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.l >>= 1;

            if gb.cpu.l == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL (HL)
        0x3Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  value&0x01 != 0{
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            value >>= 1;

            if value == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            gb.memory_write(hl, value);

            CYCLE_RETURN_16
        },

        // SRL A
        0x3Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(gb.cpu.a, 0){
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.a >>= 1;

            if gb.cpu.a == 0 {
                set_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 0, B
        0x40u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.b, 0) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }


            CYCLE_RETURN_8
        },

        // BIT 0, C
        0x41u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.c, 0) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 0, D
        0x42u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.d, 0) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 0, E
        0x43u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.e, 0) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 0, H
        0x44u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.h, 0) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 0, L
        0x45u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.l, 0) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 0, (HL)
        0x46u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(value, 0) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_12
        },

        // BIT 0, A
        0x47u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.a, 0) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 1, B
        0x48u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.b, 1) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 1, C
        0x49u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.c, 1) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 1, D
        0x4Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.d, 1) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 1, E
        0x4Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.e, 1) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 1, H
        0x4Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.h, 1) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 1, L
        0x4Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.l, 1) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 1, (HL)
        0x4Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(value, 1) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_12
        },

        // BIT 1, A
        0x4Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.a, 1) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 2, B
        0x50u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.b, 2) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 2, C
        0x51u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.c, 2) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 2, D
        0x52u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.d, 2) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 2, E
        0x53u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.e, 2) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 2, H
        0x54u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.h, 2) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 2, L
        0x55u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.l, 2) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 2, (HL)
        0x56u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(value, 2) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_12
        },

        // BIT 2, A
        0x57u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.a, 2) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 3, B
        0x58u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.b, 3) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 3, C
        0x59u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.c, 3) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 3, D
        0x5Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.d, 3) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 3, E
        0x5Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.e, 3) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 3, H
        0x5Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.h, 3) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 3, L
        0x5Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.l, 3) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 3, (HL)
        0x5Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(value, 3) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_12
        },

        // BIT 3, A
        0x5Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.a, 3) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 4, B
        0x60u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.b, 4) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 4, C
        0x61u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.c, 4) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 4, D
        0x62u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.d, 4) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 4, E
        0x63u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.e, 4) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 4, H
        0x64u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.h, 4) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 4, L
        0x65u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.l, 4) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 4, (HL)
        0x66u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(value, 4) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_12
        },

        // BIT 4, A
        0x67u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.a, 4) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 5, B
        0x68u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.b, 5) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 5, C
        0x69u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.c, 5) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 5, D
        0x6Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.d, 5) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 5, E
        0x6Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.e, 5) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 5, H
        0x6Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.h, 5) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 5, L
        0x6Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.l, 5) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 5, (HL)
        0x6Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(value, 5) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_12
        },

        // BIT 5, A
        0x6Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.a, 5) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 6, B
        0x70u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.b, 6) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 6, C
        0x71u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.c, 6) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 6, D
        0x72u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.d, 6) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 6, E
        0x73u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.e, 6) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 6, H
        0x74u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.h, 6) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 6, L
        0x75u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.l, 6) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 6, (HL)
        0x76u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(value, 6) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_12
        },

        // BIT 6, A
        0x77u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.a, 6) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 7, B
        0x78u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.b, 7) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 7, C
        0x79u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.c, 7) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 7, D
        0x7Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.d, 7) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 7, E
        0x7Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.e, 7) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 7, H
        0x7Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.h, 7) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 7, L
        0x7Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.l, 7) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // BIT 7, (HL)
        0x7Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(value, 7) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_12
        },

        // BIT 7, A
        0x7Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN);
            set_bits!(gb.cpu.f, BIT_FLAGH, BIT_FLAGZ);

            if is_bit_set(gb.cpu.a, 7) {
                clear_bits!(gb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RES 0, B
        0x80u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b &= 0xFE;
            CYCLE_RETURN_8
        },

        // RES 0, C
        0x81u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c &= 0xFE;
            CYCLE_RETURN_8
        },

        // RES 0, D
        0x82u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d &= 0xFE;
            CYCLE_RETURN_8
        },

        // RES 0, E
        0x83u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e &= 0xFE;
            CYCLE_RETURN_8
        },

        // RES 0, H
        0x84u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h &= 0xFE;
            CYCLE_RETURN_8
        },

        // RES 0, L
        0x85u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l &= 0xFE;
            CYCLE_RETURN_8
        },

        // RES 0, (HL)
        0x86u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value &= 0xFE;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // RES 0, A
        0x87u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a &= 0xFE;
            CYCLE_RETURN_8
        },

        // RES 1, B
        0x88u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b &= 0xFD;
            CYCLE_RETURN_8
        },

        // RES 1, C
        0x89u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c &= 0xFD;
            CYCLE_RETURN_8
        },

        // RES 1, D
        0x8Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d &= 0xFD;
            CYCLE_RETURN_8
        },

        // RES 1, E
        0x8Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e &= 0xFD;
            CYCLE_RETURN_8
        },

        // RES 1, H
        0x8Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h &= 0xFD;
            CYCLE_RETURN_8
        },

        // RES 1, L
        0x8Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l &= 0xFD;
            CYCLE_RETURN_8
        },

        // RES 1, (HL)
        0x8Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value &= 0xFD;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // RES 1, A
        0x8Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a &= 0xFD;
            CYCLE_RETURN_8
        },

        // RES 2, B
        0x90u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b &= 0xFB;
            CYCLE_RETURN_8
        },

        // RES 2, C
        0x91u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c &= 0xFB;
            CYCLE_RETURN_8
        },

        // RES 2, D
        0x92u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d &= 0xFB;
            CYCLE_RETURN_8
        },

        // RES 2, E
        0x93u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e &= 0xFB;
            CYCLE_RETURN_8
        },

        // RES 2, H
        0x94u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h &= 0xFB;
            CYCLE_RETURN_8
        },

        // RES 2, L
        0x95u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l &= 0xFB;
            CYCLE_RETURN_8
        },

        // RES 2, (HL)
        0x96u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value &= 0xFB;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // RES 2, A
        0x97u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a &= 0xFB;
            CYCLE_RETURN_8
        },

        // RES 3, B
        0x98u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b &= 0xF7;
            CYCLE_RETURN_8
        },

        // RES 3, C
        0x99u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c &= 0xF7;
            CYCLE_RETURN_8
        },

        // RES 3, D
        0x9Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d &= 0xF7;
            CYCLE_RETURN_8
        },

        // RES 3, E
        0x9Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e &= 0xF7;
            CYCLE_RETURN_8
        },

        // RES 3, H
        0x9Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h &= 0xF7;
            CYCLE_RETURN_8
        },

        // RES 3, L
        0x9Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l &= 0xF7;
            CYCLE_RETURN_8
        },

        // RES 3, (HL)
        0x9Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value &= 0xF7;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // RES 3, A
        0x9Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a &= 0xF7;
            CYCLE_RETURN_8
        },

        // RES 4, B
        0xA0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b &= 0xEF;
            CYCLE_RETURN_8
        },

        // RES 4, C
        0xA1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c &= 0xEF;
            CYCLE_RETURN_8
        },

        // RES 4, D
        0xA2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d &= 0xEF;
            CYCLE_RETURN_8
        },

        // RES 4, E
        0xA3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e &= 0xEF;
            CYCLE_RETURN_8
        },

        // RES 4, H
        0xA4u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h &= 0xEF;
            CYCLE_RETURN_8
        },

        // RES 4, L
        0xA5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l &= 0xEF;
            CYCLE_RETURN_8
        },

        // RES 4, (HL)
        0xA6u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value &= 0xEF;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // RES 4, A
        0xA7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a &= 0xEF;
            CYCLE_RETURN_8
        },

        // RES 5, B
        0xA8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b &= 0xDF;
            CYCLE_RETURN_8
        },

        // RES 5, C
        0xA9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c &= 0xDF;
            CYCLE_RETURN_8
        },

        // RES 5, D
        0xAAu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d &= 0xDF;
            CYCLE_RETURN_8
        },

        // RES 5, E
        0xABu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e &= 0xDF;
            CYCLE_RETURN_8
        },

        // RES 5, H
        0xACu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h &= 0xDF;
            CYCLE_RETURN_8
        },

        // RES 5, L
        0xADu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l &= 0xDF;
            CYCLE_RETURN_8
        },

        // RES 5, (HL)
        0xAEu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value &= 0xDF;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // RES 5, A
        0xAFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a &= 0xDF;
            CYCLE_RETURN_8
        },

        // RES 6, B
        0xB0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b &= 0xBF;
            CYCLE_RETURN_8
        },

        // RES 6, C
        0xB1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c &= 0xBF;
            CYCLE_RETURN_8
        },

        // RES 6, D
        0xB2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d &= 0xBF;
            CYCLE_RETURN_8
        },

        // RES 6, E
        0xB3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e &= 0xBF;
            CYCLE_RETURN_8
        },

        // RES 6, H
        0xB4u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h &= 0xBF;
            CYCLE_RETURN_8
        },

        // RES 6, L
        0xB5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l &= 0xBF;
            CYCLE_RETURN_8
        },

        // RES 6, (HL)
        0xB6u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value &= 0xBF;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // RES 6, A
        0xB7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a &= 0xBF;
            CYCLE_RETURN_8
        },

        // RES 7, B
        0xB8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b &= 0x7F;
            CYCLE_RETURN_8
        },

        // RES 7, C
        0xB9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c &= 0x7F;
            CYCLE_RETURN_8
        },

        // RES 7, D
        0xBAu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d &= 0x7F;
            CYCLE_RETURN_8
        },

        // RES 7, E
        0xBBu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e &= 0x7F;
            CYCLE_RETURN_8
        },

        // RES 7, H
        0xBCu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h &= 0x7F;
            CYCLE_RETURN_8
        },

        // RES 7, L
        0xBDu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l &= 0x7F;
            CYCLE_RETURN_8
        },

        // RES 7, (HL)
        0xBEu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value &= 0x7F;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // RES 7, A
        0xBFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a &= 0x7F;
            CYCLE_RETURN_8
        },

        // SET 0, B
        0xC0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b |= 0x01;
            CYCLE_RETURN_8
        },


        // SET 0, C
        0xC1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c |= 0x01;
            CYCLE_RETURN_8
        },

        // SET 0, D
        0xC2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d |= 0x01;
            CYCLE_RETURN_8
        },

        // SET 0, E
        0xC3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e |= 0x01;
            CYCLE_RETURN_8
        },

        // SET 0, H
        0xC4u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h |= 0x01;
            CYCLE_RETURN_8
        },

        // SET 0, L
        0xC5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l |= 0x01;
            CYCLE_RETURN_8
        },

        // SET 0, (HL)
        0xC6u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value |= 0x01;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // SET 0, A
        0xC7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a |= 0x01;
            CYCLE_RETURN_8
        },

        // SET 1, B
        0xC8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b |= 0x02;
            CYCLE_RETURN_8
        },

        // SET 1, C
        0xC9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c |= 0x02;
            CYCLE_RETURN_8
        },

        // SET 1, D
        0xCAu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d |= 0x02;
            CYCLE_RETURN_8
        },

        // SET 1, E
        0xCBu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e |= 0x02;
            CYCLE_RETURN_8
        },

        // SET 1, H
        0xCCu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h |= 0x02;
            CYCLE_RETURN_8
        },

        // SET 1, L
        0xCDu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l |= 0x02;
            CYCLE_RETURN_8
        },

        // SET 1, (HL)
        0xCEu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value |= 0x02;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // SET 1, A
        0xCFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a |= 0x02;
            CYCLE_RETURN_8
        },

        // SET 2, B
        0xD0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b |= 0x04;
            CYCLE_RETURN_8
        },

        // SET 2, C
        0xD1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c |= 0x04;
            CYCLE_RETURN_8
        },

        // SET 2, D
        0xD2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d |= 0x04;
            CYCLE_RETURN_8
        },

        // SET 2, E
        0xD3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e |= 0x04;
            CYCLE_RETURN_8
        },

        // SET 2, H
        0xD4u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h |= 0x04;
            CYCLE_RETURN_8
        },

        // SET 2, L
        0xD5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l |= 0x04;
            CYCLE_RETURN_8
        },

        // SET 2, (HL)
        0xD6u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value |= 0x04;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // SET 2, A
        0xD7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a |= 0x04;
            CYCLE_RETURN_8
        },

        // SET 3, B
        0xD8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b |= 0x08;
            CYCLE_RETURN_8
        },

        // SET 3, C
        0xD9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c |= 0x08;
            CYCLE_RETURN_8
        },

        // SET 3, D
        0xDAu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d |= 0x08;
            CYCLE_RETURN_8
        },

        // SET 3, E
        0xDBu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e |= 0x08;
            CYCLE_RETURN_8
        },

        // SET 3, H
        0xDCu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h |= 0x08;
            CYCLE_RETURN_8
        },

        // SET 3, L
        0xDDu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l |= 0x08;
            CYCLE_RETURN_8
        },

        // SET 3, (HL)
        0xDEu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value |= 0x08;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // SET 3, A
        0xDFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a |= 0x08;
            CYCLE_RETURN_8
        },

        // SET 4, B
        0xE0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b |= 0x10;
            CYCLE_RETURN_8
        },

        // SET 4, C
        0xE1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c |= 0x10;
            CYCLE_RETURN_8
        },

        // SET 4, D
        0xE2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d |= 0x10;
            CYCLE_RETURN_8
        },

        // SET 4, E
        0xE3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e |= 0x10;
            CYCLE_RETURN_8
        },

        // SET 4, H
        0xE4u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h |= 0x10;
            CYCLE_RETURN_8
        },

        // SET 4, L
        0xE5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l |= 0x10;
            CYCLE_RETURN_8
        },

        // SET 4, (HL)
        0xE6u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value |= 0x10;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // SET 4, A
        0xE7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a |= 0x10;
            CYCLE_RETURN_8
        },

        // SET 5, B
        0xE8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b |= 0x20;
            CYCLE_RETURN_8
        },

        // SET 5, C
        0xE9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c |= 0x20;
            CYCLE_RETURN_8
        },

        // SET 5, D
        0xEAu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d |= 0x20;
            CYCLE_RETURN_8
        },

        // SET 5, E
        0xEBu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e |= 0x20;
            CYCLE_RETURN_8
        },

        // SET 5, H
        0xECu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h |= 0x20;
            CYCLE_RETURN_8
        },

        // SET 5, L
        0xEDu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l |= 0x20;
            CYCLE_RETURN_8
        },

        // SET 5, (HL)
        0xEEu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value |= 0x20;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // SET 5, A
        0xEFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a |= 0x20;
            CYCLE_RETURN_8
        },

        // SET 6, B
        0xF0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b |= 0x40;
            CYCLE_RETURN_8
        },

        // SET 6, C
        0xF1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c |= 0x40;
            CYCLE_RETURN_8
        },

        // SET 6, D
        0xF2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d |= 0x40;
            CYCLE_RETURN_8
        },

        // SET 6, E
        0xF3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e |= 0x40;
            CYCLE_RETURN_8
        },

        // SET 6, H
        0xF4u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h |= 0x40;
            CYCLE_RETURN_8
        },

        // SET 6, L
        0xF5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l |= 0x40;
            CYCLE_RETURN_8
        },

        // SET 6, (HL)
        0xF6u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value |= 0x40;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // SET 6, A
        0xF7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a |= 0x40;
            CYCLE_RETURN_8
        },

        // SET 7, B
        0xF8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b |= 0x80;
            CYCLE_RETURN_8
        },

        // SET 7, C
        0xF9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c |= 0x80;
            CYCLE_RETURN_8
        },

        // SET 7, D
        0xFAu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d |= 0x80;
            CYCLE_RETURN_8
        },

        // SET 7, E
        0xFBu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e |= 0x80;
            CYCLE_RETURN_8
        },

        // SET 7, H
        0xFCu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h |= 0x80;
            CYCLE_RETURN_8
        },

        // SET 7, L
        0xFDu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l |= 0x80;
            CYCLE_RETURN_8
        },

        // SET 7, (HL)
        0xFEu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let mut value = gb.memory_read(hl);
            value |= 0x80;
            gb.memory_write(hl, value);
            CYCLE_RETURN_16
        },

        // SET 7, A
        0xFFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a |= 0x80;
            CYCLE_RETURN_8
        },

    }
}
