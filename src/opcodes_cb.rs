use crate::{bits::*, gameboy::Motherboard, globals::*, utils::*};

pub fn init_opcodes_cb() -> OpCodeMap {
    phf::phf_map! {
        // RLC B
        0x00u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.b, 7){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.b = (mb.cpu.b <<1) + 0x01;
                },
                false => {
                    mb.cpu.b <<= 1;
                }
            }

            if mb.cpu.b == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC C
        0x01u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.c, 7){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.c = (mb.cpu.c <<1) + 0x01;
                },
                false => {
                    mb.cpu.c <<= 1;
                }
            }

            if mb.cpu.c == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC D
        0x02u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.d, 7){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.d = (mb.cpu.d <<1) + 0x01;
                },
                false => {
                    mb.cpu.d <<= 1;
                }
            }

            if mb.cpu.d == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC E
        0x03u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.e, 7){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.e = (mb.cpu.e <<1) + 0x01;
                },
                false => {
                    mb.cpu.e <<= 1;
                }
            }

            if mb.cpu.e == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC H
        0x04u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.h, 7){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.h = (mb.cpu.h <<1) + 0x01;
                },
                false => {
                    mb.cpu.h <<= 1;
                }
            }

            if mb.cpu.h == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC L
        0x05u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.l, 7){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.l = (mb.cpu.l <<1) + 0x01;
                },
                false => {
                    mb.cpu.l <<= 1;
                }
            }

            if mb.cpu.l == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RLC (HL)
        0x06u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let value = memory_read(hl);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(value, 7){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    memory_write(hl, (value <<1) + 0x01);
                },
                false => {
                    memory_write(hl, value << 1);
                }
            }

            if value == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // RLC A
        0x07u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.a, 7){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a = (mb.cpu.a <<1) + 0x01;
                },
                false => {
                    mb.cpu.a <<= 1;
                }
            }

            if mb.cpu.a == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC B
        0x08u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.b, 0){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.b = (mb.cpu.b >>1) + 0x80;
                },
                false => {
                    mb.cpu.b >>= 1;
                }
            }

            if mb.cpu.b == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC C
        0x09u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.c, 0){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.c = (mb.cpu.c >>1) + 0x80;
                },
                false => {
                    mb.cpu.c >>= 1;
                }
            }

            if mb.cpu.c == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC D
        0x0Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.d, 0){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.d = (mb.cpu.d >>1) + 0x80;
                },
                false => {
                    mb.cpu.d >>= 1;
                }
            }

            if mb.cpu.d == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC E
        0x0Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.e, 0){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.e = (mb.cpu.e >>1) + 0x80;
                },
                false => {
                    mb.cpu.e >>= 1;
                }
            }

            if mb.cpu.e == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC H
        0x0Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.h, 0){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.h = (mb.cpu.h >>1) + 0x80;
                },
                false => {
                    mb.cpu.h >>= 1;
                }
            }

            if mb.cpu.h == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC L
        0x0Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.l, 0){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.l = (mb.cpu.l >>1) + 0x80;
                },
                false => {
                    mb.cpu.l >>= 1;
                }
            }

            if mb.cpu.l == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RRC (HL)
        0x0Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let value = memory_read(hl);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(value, 0){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    memory_write(hl, (value >>1) + 0x80);
                },
                false => {
                    memory_write(hl, value >> 1);
                }
            }

            if value == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // RRC A
        0x0Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            match is_bit_set(mb.cpu.a, 0){
                true => {
                    set_bits!(mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a = (mb.cpu.a >>1) + 0x80;
                },
                false => {
                    mb.cpu.a >>= 1;
                }
            }

            if mb.cpu.a == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL B
        0x10u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.b, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.b <<= 1;

            if carry {
                mb.cpu.b |= 0x01;
            }

            if mb.cpu.b == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL C
        0x11u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.c, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.c <<= 1;

            if carry {
                mb.cpu.c |= 0x01;
            }

            if mb.cpu.c == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL D
        0x12u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.d, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.d <<= 1;

            if carry {
                mb.cpu.d |= 0x01;
            }

            if mb.cpu.d == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL E
        0x13u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.e, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.e <<= 1;

            if carry {
                mb.cpu.e |= 0x01;
            }

            if mb.cpu.e == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL H
        0x14u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.h, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.h <<= 1;

            if carry {
                mb.cpu.h |= 0x01;
            }

            if mb.cpu.h == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL L
        0x15u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.l, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.l <<= 1;

            if carry {
                mb.cpu.l |= 0x01;
            }

            if mb.cpu.l == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RL (HL)
        0x16u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let value = memory_read(hl);
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(value, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            let mut result = value << 1;

            if carry {
                result |= 0x01;
            }

            memory_write(hl, result);

            if result == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // RL A
        0x17u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.a, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.a <<= 1;

            if carry {
                mb.cpu.a |= 0x01;
            }

            if mb.cpu.a == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR B
        0x18u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.b, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.b >>= 1;

            if carry {
                mb.cpu.b |= 0x80;
            }

            if mb.cpu.b == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR C
        0x19u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.c, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.c >>= 1;

            if carry {
                mb.cpu.c |= 0x80;
            }

            if mb.cpu.c == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR D
        0x1Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.d, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.d >>= 1;

            if carry {
                mb.cpu.d |= 0x80;
            }

            if mb.cpu.d == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR E
        0x1Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.e, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.e >>= 1;

            if carry {
                mb.cpu.e |= 0x80;
            }

            if mb.cpu.e == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR H
        0x1Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.h, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.h >>= 1;

            if carry {
                mb.cpu.h |= 0x80;
            }

            if mb.cpu.h == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR L
        0x1Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.l, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.l >>= 1;

            if carry {
                mb.cpu.l |= 0x80;
            }

            if mb.cpu.l == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // RR (HL)
        0x1Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let value = memory_read(hl);
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(value, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            let mut result = value >> 1;

            if carry {
                result |= 0x80;
            }

            memory_write(hl, result);

            if result == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // RR A
        0x1Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.a, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.a >>= 1;

            if carry {
                mb.cpu.a |= 0x80;
            }

            if mb.cpu.a == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA B
        0x20u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.b, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.b <<= 1;

            if mb.cpu.b == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA C
        0x21u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.c, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.c <<= 1;

            if mb.cpu.c == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA D
        0x22u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.d, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.d <<= 1;

            if mb.cpu.d == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA E
        0x23u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.e, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.e <<= 1;

            if mb.cpu.e == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA H
        0x24u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.h, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.h <<= 1;

            if mb.cpu.h == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA L
        0x25u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.l, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.l <<= 1;

            if mb.cpu.l == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SLA (HL)
        0x26u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let value = memory_read(hl);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(value, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            let result = value << 1;

            memory_write(hl, result);

            if result == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_16
        },

        // SLA A
        0x27u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.a, 7){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.a <<= 1;

            if mb.cpu.a == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA B
        0x28u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.b, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            // mb.cpu.b = mb.cpu.b.rotate_right(1);

            match is_bit_set(mb.cpu.b, 7){
                true => {
                    mb.cpu.b = (mb.cpu.b >> 1) | 0x80;
                },
                false => {
                    mb.cpu.b >>= 1;
                }
            }

            if mb.cpu.b == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA C
        0x29u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.c, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            // mb.cpu.c = mb.cpu.c.rotate_right(1);

            match is_bit_set(mb.cpu.c, 7){
                true => {
                    mb.cpu.c = (mb.cpu.c >> 1) | 0x80;
                },
                false => {
                    mb.cpu.c >>= 1;
                }
            }

            if mb.cpu.c == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA D
        0x2Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.d, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            // mb.cpu.d = mb.cpu.d.rotate_right(1);

            match is_bit_set(mb.cpu.d, 7){
                true => {
                    mb.cpu.d = (mb.cpu.d >> 1) | 0x80;
                },
                false => {
                    mb.cpu.d >>= 1;
                }
            }

            if mb.cpu.d == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA E
        0x2Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.e, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            // mb.cpu.e = mb.cpu.e.rotate_right(1);

            match is_bit_set(mb.cpu.e, 7){
                true => {
                    mb.cpu.e = (mb.cpu.e >> 1) | 0x80;
                },
                false => {
                    mb.cpu.e >>= 1;
                }
            }

            if mb.cpu.e == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA H
        0x2Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.h, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            // mb.cpu.h = mb.cpu.h.rotate_right(1);

            match is_bit_set(mb.cpu.h, 7){
                true => {
                    mb.cpu.h = (mb.cpu.h >> 1) | 0x80;
                },
                false => {
                    mb.cpu.h >>= 1;
                }
            }

            if mb.cpu.h == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA L
        0x2Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.l, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            // mb.cpu.l = mb.cpu.l.rotate_right(1);

            match is_bit_set(mb.cpu.l, 7){
                true => {
                    mb.cpu.l = (mb.cpu.l >> 1) | 0x80;
                },
                false => {
                    mb.cpu.l >>= 1;
                }
            }

            if mb.cpu.l == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRA (HL)
        0x2Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let mut value = memory_read(hl);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  value&0x01 != 0{
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            if value&0x80 != 0{
                value = (value >> 1) | 0x80;
            } else {
                value >>= 1;
            }

            if value == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            memory_write(hl, value);

            CYCLE_RETURN_16
        },

        // SRA A
        0x2Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);

            if  is_bit_set(mb.cpu.a, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            // mb.cpu.a = mb.cpu.a.rotate_right(1);

            match is_bit_set(mb.cpu.a, 7){
                true => {
                    mb.cpu.a = (mb.cpu.a >> 1) | 0x80;
                },
                false => {
                    mb.cpu.a >>= 1;
                }
            }

            if mb.cpu.a == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SWAP B
        0x30u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut b = mb.cpu.b;
            b = (b >> 4) | (b << 4);

            if b == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            mb.cpu.b = b;

            CYCLE_RETURN_8
        },

        // SWAP C
        0x31u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut c = mb.cpu.c;
            c = (c >> 4) | (c << 4);

            if c == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            mb.cpu.c = c;

            CYCLE_RETURN_8
        },

        // SWAP D
        0x32u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut d = mb.cpu.d;
            d = (d >> 4) | (d << 4);

            if d == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            mb.cpu.d = d;

            CYCLE_RETURN_8
        },

        // SWAP E
        0x33u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut e = mb.cpu.e;
            e = (e >> 4) | (e << 4);

            if e == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            mb.cpu.e = e;

            CYCLE_RETURN_8
        },

        // SWAP H
        0x34u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut h = mb.cpu.h;
            h = (h >> 4) | (h << 4);

            if h == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            mb.cpu.h = h;

            CYCLE_RETURN_8
        },

        // SWAP L
        0x35u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut l = mb.cpu.l;
            l = (l >> 4) | (l << 4);

            if l == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            mb.cpu.l = l;

            CYCLE_RETURN_8
        },

        // SWAP (HL)
        0x36u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let mut value = memory_read(hl);
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            value = (value >> 4) | (value << 4);

            if value == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            memory_write(hl, value);

            CYCLE_RETURN_16
        },

        // SWAP A
        0x37u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ);

            let mut a = mb.cpu.a;
            a = (a >> 4) | (a << 4);

            if a == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            mb.cpu.a = a;

            CYCLE_RETURN_8
        },

        // SRL B
        0x38u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(mb.cpu.b, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.b >>= 1;

            if mb.cpu.b == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL C
        0x39u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(mb.cpu.c, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.c >>= 1;

            if mb.cpu.c == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL D
        0x3Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(mb.cpu.d, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.d >>= 1;

            if mb.cpu.d == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL E
        0x3Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(mb.cpu.e, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.e >>= 1;

            if mb.cpu.e == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL H
        0x3Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(mb.cpu.h, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.h >>= 1;

            if mb.cpu.h == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL L
        0x3Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  is_bit_set(mb.cpu.l, 0){
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.l >>= 1;

            if mb.cpu.l == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            CYCLE_RETURN_8
        },

        // SRL (HL)
        0x3Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let mut value = memory_read(hl);
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGZ, BIT_FLAGC);

            if  value&0x01 != 0{
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            value >>= 1;

            if value == 0 {
                set_bits!(mb.cpu.f, BIT_FLAGZ);
            }

            memory_write(hl, value);

            CYCLE_RETURN_16
        },










    }
}
