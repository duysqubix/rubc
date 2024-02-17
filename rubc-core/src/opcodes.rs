use crate::{bits::*, gameboy::Gameboy, globals::*};

pub fn init_opcodes() -> OpCodeMap {
    phf::phf_map! {

        // NOP
        0x00u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            log::trace!("PC Before NOP: {:#06X}", gb.cpu.pc);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            log::trace!("PC After NOP: {:#06X}", gb.cpu.pc);
            CYCLE_RETURN_4
        },

        // LD BC, u16
        0x01u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.b = (value >> 8) as u8;
            gb.cpu.c = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_4
        },

        // LD (BC), A
        0x02u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = (gb.cpu.b as u16) << 8 | gb.cpu.c as u16;
            gb.memory_write(addr, gb.cpu.a);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // INC BC
        0x03u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let value = (gb.cpu.b as u16) << 8 | gb.cpu.c as u16;
            let result = value.wrapping_add(1);
            gb.cpu.b = (result >> 8) as u8;
            gb.cpu.c = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // INC B
        0x04u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            increment_register!(gb, b);
            CYCLE_RETURN_4
        },

        // DEC B
        0x05u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            decrement_register!(gb, b);
            CYCLE_RETURN_4
        },

        // LD B, u8
        0x06u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.b = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_4
        },

        // RLCA
        0x07u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGZ, BIT_FLAGH);


            match is_bit_set(gb.cpu.a, 7){
                true => {
                    set_bit(&mut gb.cpu.f, BIT_FLAGC);
                    gb.cpu.a = (gb.cpu.a << 1) + 1;
                },
                false => {
                    clear_bit(&mut gb.cpu.f, BIT_FLAGC);
                    gb.cpu.a <<= 1;
                }
            }
            gb.cpu.pc +=1;
            CYCLE_RETURN_4
        },

        // LD (u16), SP
        0x08u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.memory_write(value, (gb.cpu.sp & 0x00FF) as u8);
            gb.memory_write(value + 1, (gb.cpu.sp >> 8) as u8);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);

            CYCLE_RETURN_20
        },

        // ADD HL, BC
        0x09u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

            let hl = ((gb.cpu.h as u16) << 8 | gb.cpu.l as u16) as u32;
            let bc = ((gb.cpu.b as u16) << 8 | gb.cpu.c as u16) as u32;
            let result = hl + bc;

            if result & 0x10000 != 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGC);
            }

            if (hl ^ bc ^ result) & 0x1000 != 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGH);
            }

            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;

            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (BC)
        0x0Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = (gb.cpu.b as u16) << 8 | gb.cpu.c as u16;
            gb.cpu.a = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DEC BC
        0x0Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let value = (gb.cpu.b as u16) << 8 | gb.cpu.c as u16;
            let result = value.wrapping_sub(1);
            gb.cpu.b = (result >> 8) as u8;
            gb.cpu.c = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC C
        0x0Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            increment_register!(gb, c);
            CYCLE_RETURN_4
        },

        // DEC C
        0x0Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            decrement_register!(gb, c);
            CYCLE_RETURN_4
        },

        // LD C, u8
        0x0Eu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.c = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // RRCA
        0x0Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            match is_bit_set(gb.cpu.a, 0){
                true => {
                    set_bit(&mut gb.cpu.f, BIT_FLAGC);
                    gb.cpu.a = (gb.cpu.a >> 1) | 0x80;
                },
                false => {
                    clear_bit(&mut gb.cpu.f, BIT_FLAGC);
                    gb.cpu.a >>= 1;
                }
            }
            gb.cpu.pc +=1;
            CYCLE_RETURN_4
        },


        // STOP 0
        0x10u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            // handle CGB stuff here.
            if gb.cgb_mode {
                let value = gb.memory_read(IO_KEY1);
                if is_bit_set(value, 0){
                    gb.double_speed = !gb.double_speed;
                    gb.memory_write(IO_KEY1, value^0x81);
                }
                gb.memory_write(IO_DIV, 0); //reset timer
            }

            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD DE, u16
        0x11u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.d = (value >> 8) as u8;
            gb.cpu.e = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_12
        },

        // LD (DE), A
        0x12u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = (gb.cpu.d as u16) << 8 | gb.cpu.e as u16;
            gb.memory_write(addr, gb.cpu.a);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC DE
        0x13u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let value = (gb.cpu.d as u16) << 8 | gb.cpu.e as u16;
            let result = value.wrapping_add(1);
            gb.cpu.d = (result >> 8) as u8;
            gb.cpu.e = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC D
        0x14u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            increment_register!(gb, d);
            CYCLE_RETURN_4
        },

        // DEC D
        0x15u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            decrement_register!(gb, d);
            CYCLE_RETURN_4
        },

        // LD D, u8
        0x16u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.d = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // RLA
        0x17u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);

            match is_bit_set(gb.cpu.a, 7){
                true => set_bit(&mut gb.cpu.f, BIT_FLAGC),
                false => clear_bit(&mut gb.cpu.f, BIT_FLAGC),
            }

            gb.cpu.a <<= 1;
            if carry {
                gb.cpu.a |= 1;
            }

            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            CYCLE_RETURN_4
        },

        // JR r8
        0x18u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.pc = (gb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
            CYCLE_RETURN_12
        },

        // ADD HL, DE
        0x19u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

            let hl = ((gb.cpu.h as u16) << 8 | gb.cpu.l as u16) as u32;
            let de = ((gb.cpu.d as u16) << 8 | gb.cpu.e as u16) as u32;
            let result = hl + de;

            if result & 0x10000 != 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGC);
            }

            if (hl ^ de ^ result) & 0x1000 != 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGH);
            }

            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;

            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (DE)
        0x1Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = (gb.cpu.d as u16) << 8 | gb.cpu.e as u16;
            gb.cpu.a = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DEC DE
        0x1Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let value = (gb.cpu.d as u16) << 8 | gb.cpu.e as u16;
            let result = value.wrapping_sub(1);
            gb.cpu.d = (result >> 8) as u8;
            gb.cpu.e = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC E
        0x1Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            increment_register!(gb, e);
            CYCLE_RETURN_4
        },

        // DEC E
        0x1Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            decrement_register!(gb, e);
            CYCLE_RETURN_4
        },

        // LD E, u8
        0x1Eu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.e = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // RRA
        0x1Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            let carry = is_bit_set(gb.cpu.f, BIT_FLAGC);

            match is_bit_set(gb.cpu.a, 0){
                true => set_bit(&mut gb.cpu.f, BIT_FLAGC),
                false => clear_bit(&mut gb.cpu.f, BIT_FLAGC),
            }

            gb.cpu.a >>= 1;
            if carry {
                gb.cpu.a |= 0x80;
            }

            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            CYCLE_RETURN_4
        },

        // JR NZ, r8 - Relative jump if last result was not zero
        0x20u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            if !is_bit_set(gb.cpu.f, BIT_FLAGZ) {
                gb.cpu.pc = (gb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
                CYCLE_RETURN_12
            }else {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN_8
            }
        },

        // LD HL, u16
        0x21u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.h = (value >> 8) as u8;
            gb.cpu.l = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_12
        },

        // LD (HL+), A
        0x22u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.a);
            let result = addr.wrapping_add(1);
            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC HL
        0x23u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let value = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let result = value.wrapping_add(1);
            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC H
        0x24u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            increment_register!(gb, h);
            CYCLE_RETURN_4
        },

        // DEC H
        0x25u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            decrement_register!(gb, h);
            CYCLE_RETURN_4
        },

        // LD H, u8
        0x26u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.h = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // DAA
        0x27u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let mut corr: u8 = 0;

            if is_bit_set(gb.cpu.f, BIT_FLAGH){
                corr |= 0x06;
            }

            if is_bit_set(gb.cpu.f, BIT_FLAGC){
                corr |= 0x60;
            }

            if is_bit_set(gb.cpu.f, BIT_FLAGN){
               gb.cpu.a = gb.cpu.a.wrapping_sub(corr);
            }else{
                if (gb.cpu.a & 0x0F) > 9 {
                    corr |= 0x06;
                }

                if gb.cpu.a > 0x99 {
                    corr |= 0x60;
                }

                gb.cpu.a = gb.cpu.a.wrapping_add(corr);
            }

            let mut flag: u8 = 0;
            if gb.cpu.a == 0 {
                set_bit(&mut flag, BIT_FLAGZ);
            }

            if corr & 0x60 != 0 {
                set_bit(&mut flag, BIT_FLAGC);
            }

            gb.cpu.f &= 0x40;
            gb.cpu.f |= flag;

            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // JR Z, r8 - Relative jump if last result was zero
        0x28u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            if is_bit_set(gb.cpu.f, BIT_FLAGZ) {
                gb.cpu.pc = (gb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
                CYCLE_RETURN_12
            }else {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN_8
            }
        },

        // ADD HL, HL
        0x29u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

            let hl = ((gb.cpu.h as u16) << 8 | gb.cpu.l as u16) as u32;
            let result = hl + hl;

            if result & 0x10000 != 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGC);
            }

            if (hl ^ result ^ hl) & 0x1000 != 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGH);
            }

            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;

            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (HL+)
        0x2Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.a = gb.memory_read(addr);
            let result = addr.wrapping_add(1);
            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DEC HL
        0x2Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let value = (gb.cpu.h as u16) << 8 | gb.cpu.l as u16;
            let result = value.wrapping_sub(1);
            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC L
        0x2Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            increment_register!(gb, l);
            CYCLE_RETURN_4
        },

        // DEC L
        0x2Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            decrement_register!(gb, l);
            CYCLE_RETURN_4
        },

        // LD L, u8
        0x2Eu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.l = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // CPL - Complement A
        0x2Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a = !gb.cpu.a;
            set_bit(&mut gb.cpu.f, BIT_FLAGN);
            set_bit(&mut gb.cpu.f, BIT_FLAGH);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // JR NC, r8 - Relative jump if last result was not carry
        0x30u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            if !is_bit_set(gb.cpu.f, BIT_FLAGC) {
                gb.cpu.pc = (gb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
                CYCLE_RETURN_12
            }else {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN_8
            }
        },

        // LD SP, u16
        0x31u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.sp = value;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_12
        },

        // LD (HL-), A
        0x32u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.a);
            let result = addr.wrapping_sub(1);
            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC SP
        0x33u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC (HL)
        0x34u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            let result = value.wrapping_add(1);

            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGZ, BIT_FLAGH);

            if result == 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGZ);
            }

            if (value & 0x0F) == 0x0F {
                set_bit(&mut gb.cpu.f, BIT_FLAGH);
            }

            gb.memory_write(hl, result);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            CYCLE_RETURN_12
        },

        // DEC (HL)
        0x35u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let hl = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(hl);
            let result = value.wrapping_sub(1);

            set_bit(&mut gb.cpu.f, BIT_FLAGN);
            clear_bits!(gb.cpu.f, BIT_FLAGZ, BIT_FLAGH);

            if result == 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGZ);
            }

            if (value & 0x0F) == 0x0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGH);
            }

            gb.memory_write(hl, result);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            CYCLE_RETURN_12
        },

        // LD (HL), u8
        0x36u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let hl = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(hl, value as u8);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_12
        },

        // SCF - Set carry flag
        0x37u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH);
            set_bit(&mut gb.cpu.f, BIT_FLAGC);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // JR C, r8 - Relative jump if last result was carry
        0x38u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            if is_bit_set(gb.cpu.f, BIT_FLAGC) {
                gb.cpu.pc = (gb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
                CYCLE_RETURN_12
            }else {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN_8
            }
        },

        // ADD HL, SP
        0x39u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            clear_bits!(gb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

            let hl = ((gb.cpu.h as u16) << 8 | gb.cpu.l as u16) as u32;
            let result = hl + gb.cpu.sp as u32;

            if result & 0x10000 != 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGC);
            }

            if (hl ^ gb.cpu.sp as u32 ^ result) & 0x1000 != 0 {
                set_bit(&mut gb.cpu.f, BIT_FLAGH);
            }

            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;

            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (HL-)
        0x3Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.a = gb.memory_read(addr);
            let result = addr.wrapping_sub(1);
            gb.cpu.h = (result >> 8) as u8;
            gb.cpu.l = result as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DEC SP
        0x3Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.sp = gb.cpu.sp.wrapping_sub(1);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC A
        0x3Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            increment_register!(gb, a);
            CYCLE_RETURN_4
        },

        // DEC A
        0x3Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            decrement_register!(gb, a);
            CYCLE_RETURN_4
        },

        // LD A, u8
        0x3Eu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.a = value as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // CCF - Complement carry flag
        0x3Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {

            clear_bits!(gb.cpu.f, BIT_FLAGN, BIT_FLAGH);
            if is_bit_set(gb.cpu.f, BIT_FLAGC) {
                clear_bit(&mut gb.cpu.f, BIT_FLAGC);
            } else {
                set_bit(&mut gb.cpu.f, BIT_FLAGC);
            }
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, B
        0x40u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, C
        0x41u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b = gb.cpu.c;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, D
        0x42u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b = gb.cpu.d;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, E
        0x43u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b = gb.cpu.e;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, H
        0x44u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b = gb.cpu.h;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, L
        0x45u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b = gb.cpu.l;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, (HL)
        0x46u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.b = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD B, A
        0x47u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.b = gb.cpu.a;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, B
        0x48u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c = gb.cpu.b;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, C
        0x49u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, D
        0x4Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c = gb.cpu.d;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, E
        0x4Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c = gb.cpu.e;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, H
        0x4Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c = gb.cpu.h;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, L
        0x4Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c = gb.cpu.l;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, (HL)
        0x4Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.c = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD C, A
        0x4Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.c = gb.cpu.a;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, B
        0x50u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d = gb.cpu.b;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, C
        0x51u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d = gb.cpu.c;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, D
        0x52u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, E
        0x53u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d = gb.cpu.e;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, H
        0x54u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d = gb.cpu.h;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, L
        0x55u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d = gb.cpu.l;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, (HL)
        0x56u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.d = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD D, A
        0x57u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.d = gb.cpu.a;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, B
        0x58u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e = gb.cpu.b;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, C
        0x59u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e = gb.cpu.c;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, D
        0x5Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e = gb.cpu.d;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, E
        0x5Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, H
        0x5Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e = gb.cpu.h;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, L
        0x5Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e = gb.cpu.l;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, (HL)
        0x5Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.e = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD E, A
        0x5Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.e = gb.cpu.a;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, B
        0x60u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h = gb.cpu.b;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, C
        0x61u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h = gb.cpu.c;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, D
        0x62u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h = gb.cpu.d;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, E
        0x63u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h = gb.cpu.e;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, H
        0x64u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, L
        0x65u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h = gb.cpu.l;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, (HL)
        0x66u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.h = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD H, A
        0x67u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.h = gb.cpu.a;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, B
        0x68u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l = gb.cpu.b;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, C
        0x69u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l = gb.cpu.c;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, D
        0x6Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l = gb.cpu.d;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, E
        0x6Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l = gb.cpu.e;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, H
        0x6Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l = gb.cpu.h;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, L
        0x6Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, (HL)
        0x6Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.l = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD L, A
        0x6Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.l = gb.cpu.a;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD (HL), B
        0x70u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.b);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), C
        0x71u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.c);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), D
        0x72u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.d);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), E
        0x73u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.e);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), H
        0x74u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.h);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), L
        0x75u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.l);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // HALT
        0x76u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.halted = true;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD (HL), A
        0x77u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.memory_write(addr, gb.cpu.a);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, B
        0x78u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a = gb.cpu.b;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, C
        0x79u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a = gb.cpu.c;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, D
        0x7Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a = gb.cpu.d;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, E
        0x7Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a = gb.cpu.e;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, H
        0x7Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a = gb.cpu.h;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, L
        0x7Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.a = gb.cpu.l;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, (HL)
        0x7Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.a = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, A
        0x7Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // ADD A, B
        0x80u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_register!(gb, a, b);
            CYCLE_RETURN_4
        },

        // ADD A, C
        0x81u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_register!(gb, a, c);
            CYCLE_RETURN_4
        },

        // ADD A, D
        0x82u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_register!(gb, a, d);
            CYCLE_RETURN_4
        },

        // ADD A, E
        0x83u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_register!(gb, a, e);
            CYCLE_RETURN_4
        },

        // ADD A, H
        0x84u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_register!(gb, a, h);
            CYCLE_RETURN_4
        },

        // ADD A, L
        0x85u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_register!(gb, a, l);
            CYCLE_RETURN_4
        },

        // ADD A, (HL)
        0x86u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(addr);
            add_register_from_value!(gb, a, value);
            CYCLE_RETURN_8
        },

        // ADD A, A
        0x87u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_register!(gb, a, a);
            CYCLE_RETURN_4
        },

        // ADC A, B
        0x88u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_carry_register!(gb, a, b);
            CYCLE_RETURN_4
        },

        // ADC A, C
        0x89u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_carry_register!(gb, a, c);
            CYCLE_RETURN_4
        },

        // ADC A, D
        0x8Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_carry_register!(gb, a, d);
            CYCLE_RETURN_4
        },

        // ADC A, E
        0x8Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_carry_register!(gb, a, e);
            CYCLE_RETURN_4
        },

        // ADC A, H
        0x8Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_carry_register!(gb, a, h);
            CYCLE_RETURN_4
        },

        // ADC A, L
        0x8Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_carry_register!(gb, a, l);
            CYCLE_RETURN_4
        },

        // ADC A, (HL)
        0x8Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(addr);
            add_carry_register_from_value!(gb, a, value);
            CYCLE_RETURN_8
        },

        // ADC A, A
        0x8Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            add_carry_register!(gb, a, a);
            CYCLE_RETURN_4
        },

        // SUB B
        0x90u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_register!(gb, a, b);
            CYCLE_RETURN_4
        },

        // SUB C
        0x91u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_register!(gb, a, c);
            CYCLE_RETURN_4
        },

        // SUB D
        0x92u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_register!(gb, a, d);
            CYCLE_RETURN_4
        },

        // SUB E
        0x93u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_register!(gb, a, e);
            CYCLE_RETURN_4
        },

        // SUB H
        0x94u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_register!(gb, a, h);
            CYCLE_RETURN_4
        },

        // SUB L
        0x95u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_register!(gb, a, l);
            CYCLE_RETURN_4
        },

        // SUB (HL)
        0x96u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(addr);
            sub_register_from_value!(gb, a, value);
            CYCLE_RETURN_8
        },

        // SUB A
        0x97u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_register!(gb, a, a);
            CYCLE_RETURN_4
        },

        // SBC A, B
        0x98u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_carry_register!(gb, a, b);
            CYCLE_RETURN_4
        },

        // SBC A, C
        0x99u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_carry_register!(gb, a, c);
            CYCLE_RETURN_4
        },

        // SBC A, D
        0x9Au8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_carry_register!(gb, a, d);
            CYCLE_RETURN_4
        },

        // SBC A, E
        0x9Bu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_carry_register!(gb, a, e);
            CYCLE_RETURN_4
        },

        // SBC A, H
        0x9Cu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_carry_register!(gb, a, h);
            CYCLE_RETURN_4
        },

        // SBC A, L
        0x9Du8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_carry_register!(gb, a, l);
            CYCLE_RETURN_4
        },

        // SBC A, (HL)
        0x9Eu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(addr);
            sub_carry_register_from_value!(gb, a, value);
            CYCLE_RETURN_8
        },

        // SBC A, A
        0x9Fu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            sub_carry_register!(gb, a, a);
            CYCLE_RETURN_4
        },

        // AND B
        0xA0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            and_register!(gb, a, b);
            CYCLE_RETURN_4
        },

        // AND C
        0xA1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            and_register!(gb, a, c);
            CYCLE_RETURN_4
        },

        // AND D
        0xA2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            and_register!(gb, a, d);
            CYCLE_RETURN_4
        },

        // AND E
        0xA3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            and_register!(gb, a, e);
            CYCLE_RETURN_4
        },

        // AND H
        0xA4u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            and_register!(gb, a, h);
            CYCLE_RETURN_4
        },

        // AND L
        0xA5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            and_register!(gb, a, l);
            CYCLE_RETURN_4
        },

        // AND (HL)
        0xA6u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(addr);
            and_register_with_value!(gb, a, value);
            CYCLE_RETURN_8
        },

        // AND A
        0xA7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            and_register!(gb, a, a);
            CYCLE_RETURN_4
        },

        // XOR B
        0xA8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            xor_register!(gb, a, b);
            CYCLE_RETURN_4
        },

        // XOR C
        0xA9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            xor_register!(gb, a, c);
            CYCLE_RETURN_4
        },

        // XOR D
        0xAAu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            xor_register!(gb, a, d);
            CYCLE_RETURN_4
        },

        // XOR E
        0xABu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            xor_register!(gb, a, e);
            CYCLE_RETURN_4
        },

        // XOR H
        0xACu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            xor_register!(gb, a, h);
            CYCLE_RETURN_4
        },

        // XOR L
        0xADu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            xor_register!(gb, a, l);
            CYCLE_RETURN_4
        },

        // XOR (HL)
        0xAEu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(addr);
            xor_register_with_value!(gb, a, value);
            CYCLE_RETURN_8
        },

        // XOR A
        0xAFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            xor_register!(gb, a, a);
            CYCLE_RETURN_4
        },

        // OR B
        0xB0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            or_register!(gb, a, b);
            CYCLE_RETURN_4
        },

        // OR C
        0xB1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            or_register!(gb, a, c);
            CYCLE_RETURN_4
        },

        // OR D
        0xB2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            or_register!(gb, a, d);
            CYCLE_RETURN_4
        },

        // OR E
        0xB3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            or_register!(gb, a, e);
            CYCLE_RETURN_4
        },

        // OR H
        0xB4u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            or_register!(gb, a, h);
            CYCLE_RETURN_4
        },

        // OR L
        0xB5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            or_register!(gb, a, l);
            CYCLE_RETURN_4
        },

        // OR (HL)
        0xB6u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(addr);
            or_register_with_value!(gb, a, value);
            CYCLE_RETURN_8
        },

        // OR A
        0xB7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            or_register!(gb, a, a);
            CYCLE_RETURN_4
        },

        // CP B
        0xB8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            compare_register!(gb, a, b);
            CYCLE_RETURN_4
        },

        // CP C
        0xB9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            compare_register!(gb, a, c);
            CYCLE_RETURN_4
        },

        // CP D
        0xBAu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            compare_register!(gb, a, d);
            CYCLE_RETURN_4
        },

        // CP E
        0xBBu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            compare_register!(gb, a, e);
            CYCLE_RETURN_4
        },

        // CP H
        0xBCu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            compare_register!(gb, a, h);
            CYCLE_RETURN_4
        },

        // CP L
        0xBDu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            compare_register!(gb, a, l);
            CYCLE_RETURN_4
        },

        // CP (HL)
        0xBEu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            let value = gb.memory_read(addr);
            compare_register_with_value!(gb, a, value);
            CYCLE_RETURN_8
        },

        // CP A
        0xBFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            compare_register!(gb, a, a);
            CYCLE_RETURN_4
        },

        // RET NZ
        0xC0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            if !is_bit_set(gb.cpu.f, BIT_FLAGZ) {
                let lo = gb.memory_read(gb.cpu.sp);
                gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
                let hi = gb.memory_read(gb.cpu.sp);
                gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
                gb.cpu.pc = ((hi as u16) << 8) | lo as u16;
                CYCLE_RETURN_20
            } else {
                gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
                CYCLE_RETURN_8
            }
        },

        // POP BC
        0xC1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let lo = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            let hi = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            gb.cpu.b = hi;
            gb.cpu.c = lo;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_12
        },

        // JP NZ, u16
        0xC2u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            if !is_bit_set(gb.cpu.f, BIT_FLAGZ) {
                gb.cpu.pc = value;
                CYCLE_RETURN_16
            } else {
                gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
                CYCLE_RETURN_12
            }
        },

        // JP, u16 - Absolute jump
        0xC3u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.pc = value;
            CYCLE_RETURN_16
        },

        // CALL NZ, u16
        0xC4u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);

            if !is_bit_set(gb.cpu.f, BIT_FLAGZ) {
                let sp1 = gb.cpu.sp.wrapping_sub(1);
                let sp2 = gb.cpu.sp.wrapping_sub(2);

                let pch = ((gb.cpu.pc >> 8) & 0xFF) as u8;
                let pcl = (gb.cpu.pc & 0xFF) as u8;
                gb.memory_write(sp1, pch);
                gb.memory_write(sp2, pcl);
                gb.cpu.sp = sp2;
                gb.cpu.pc = value;
                CYCLE_RETURN_24
            }else {
                CYCLE_RETURN_12
            }
        },

        // PUSH BC
        0xC5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, gb.cpu.b);
            gb.memory_write(sp2, gb.cpu.c);
            gb.cpu.sp = sp2;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_16
        },

        // ADD A, u8
        0xC6u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let v = value as u8;
            add_register_from_value!(gb, a, v);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 00H
        0xC7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, ((gb.cpu.pc >> 8) & 0xFF) as u8);
            gb.memory_write(sp2, (gb.cpu.pc & 0xFF) as u8);
            gb.cpu.sp = sp2;
            gb.cpu.pc = 0x00;
            CYCLE_RETURN_16
        },

        // RET Z
        0xC8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            if is_bit_set(gb.cpu.f, BIT_FLAGZ) {
                let lo = gb.memory_read(gb.cpu.sp);
                gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
                let hi = gb.memory_read(gb.cpu.sp);
                gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
                gb.cpu.pc = ((hi as u16) << 8) | lo as u16;
                CYCLE_RETURN_20
            } else {
                gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
                CYCLE_RETURN_8
            }
        },

        // RET
        0xC9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let lo = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            let hi = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            gb.cpu.pc = ((hi as u16) << 8) | lo as u16;
            CYCLE_RETURN_16
        },

        // JP Z, u16
        0xCAu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            if is_bit_set(gb.cpu.f, BIT_FLAGZ) {
                gb.cpu.pc = value;
                CYCLE_RETURN_16
            } else {
                gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
                CYCLE_RETURN_12
            }
        },

        // PREFIX CB
        0xCBu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            // let opcode = gb.memory_read(gb.cpu.pc);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            let cb_opcode = gb.memory_read(gb.cpu.pc);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            gb.execute_op_code_cb(cb_opcode).expect("Failed to execute CB opcode")
        },

        // CALL Z, u16
        0xCCu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);

            if is_bit_set(gb.cpu.f, BIT_FLAGZ) {
                let sp1 = gb.cpu.sp.wrapping_sub(1);
                let sp2 = gb.cpu.sp.wrapping_sub(2);

                let pch = ((gb.cpu.pc >> 8) & 0xFF) as u8;
                let pcl = (gb.cpu.pc & 0xFF) as u8;
                gb.memory_write(sp1, pch);
                gb.memory_write(sp2, pcl);
                gb.cpu.sp = sp2;
                gb.cpu.pc = value;
                CYCLE_RETURN_24
            } else {
                CYCLE_RETURN_12
            }
        },

        // CALL u16
        0xCDu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);

            let pch = ((gb.cpu.pc >> 8) & 0xFF) as u8;
            let pcl = (gb.cpu.pc & 0xFF) as u8;
            gb.memory_write(sp1, pch);
            gb.memory_write(sp2, pcl);
            gb.cpu.sp = sp2;
            gb.cpu.pc = value;
            CYCLE_RETURN_24
        },

        // ADC A, u8
        0xCEu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let v = value as u8;
            add_carry_register_from_value!(gb, a, v);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 08H
        0xCFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, ((gb.cpu.pc >> 8) & 0xFF) as u8);
            gb.memory_write(sp2, (gb.cpu.pc & 0xFF) as u8);
            gb.cpu.sp = sp2;
            gb.cpu.pc = 0x08;
            CYCLE_RETURN_16
        },

        // RET NC
        0xD0u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            if !is_bit_set(gb.cpu.f, BIT_FLAGC) {
                let lo = gb.memory_read(gb.cpu.sp);
                gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
                let hi = gb.memory_read(gb.cpu.sp);
                gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
                gb.cpu.pc = ((hi as u16) << 8) | lo as u16;
                CYCLE_RETURN_20
            } else {
                gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
                CYCLE_RETURN_8
            }
        },

        // POP DE
        0xD1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let lo = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            let hi = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            gb.cpu.d = hi;
            gb.cpu.e = lo;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_12
        },

        // JP NC, u16
        0xD2u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            if !is_bit_set(gb.cpu.f, BIT_FLAGC) {
                gb.cpu.pc = value;
                CYCLE_RETURN_16
            } else {
                gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
                CYCLE_RETURN_12
            }
        },

        // CALL NC, u16
        0xD4u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);

            if !is_bit_set(gb.cpu.f, BIT_FLAGC) {
                let sp1 = gb.cpu.sp.wrapping_sub(1);
                let sp2 = gb.cpu.sp.wrapping_sub(2);

                let pch = ((gb.cpu.pc >> 8) & 0xFF) as u8;
                let pcl = (gb.cpu.pc & 0xFF) as u8;
                gb.memory_write(sp1, pch);
                gb.memory_write(sp2, pcl);
                gb.cpu.sp = sp2;
                gb.cpu.pc = value;
                CYCLE_RETURN_24
            } else {
                CYCLE_RETURN_12
            }
        },

        // PUSH DE
        0xD5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, gb.cpu.d);
            gb.memory_write(sp2, gb.cpu.e);
            gb.cpu.sp = sp2;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_16
        },

        // SUB u8
        0xD6u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let v = value as u8;
            sub_register_from_value!(gb, a, v);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 10H
        0xD7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, ((gb.cpu.pc >> 8) & 0xFF) as u8);
            gb.memory_write(sp2, (gb.cpu.pc & 0xFF) as u8);
            gb.cpu.sp = sp2;
            gb.cpu.pc = 0x10;
            CYCLE_RETURN_16
        },

        // RET C
        0xD8u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            if is_bit_set(gb.cpu.f, BIT_FLAGC) {
                let lo = gb.memory_read(gb.cpu.sp);
                gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
                let hi = gb.memory_read(gb.cpu.sp);
                gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
                gb.cpu.pc = ((hi as u16) << 8) | lo as u16;
                CYCLE_RETURN_20
            } else {
                gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
                CYCLE_RETURN_8
            }
        },

        // RETI
        0xD9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let lo = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            let hi = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            gb.cpu.pc = ((hi as u16) << 8) | lo as u16;
            gb.ime = true;
            CYCLE_RETURN_16
        },

        // JP C, u16
        0xDAu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            if is_bit_set(gb.cpu.f, BIT_FLAGC) {
                gb.cpu.pc = value;
                CYCLE_RETURN_16
            } else {
                gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
                CYCLE_RETURN_12
            }
        },

        // CALL C, u16
        0xDCu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);

            if is_bit_set(gb.cpu.f, BIT_FLAGC) {
                let sp1 = gb.cpu.sp.wrapping_sub(1);
                let sp2 = gb.cpu.sp.wrapping_sub(2);

                let pch = ((gb.cpu.pc >> 8) & 0xFF) as u8;
                let pcl = (gb.cpu.pc & 0xFF) as u8;
                gb.memory_write(sp1, pch);
                gb.memory_write(sp2, pcl);
                gb.cpu.sp = sp2;
                gb.cpu.pc = value;
                CYCLE_RETURN_24
            } else {
                CYCLE_RETURN_12
            }
        },

        // SBC A, u8
        0xDEu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let v = value as u8;
            sub_carry_register_from_value!(gb, a, v);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 18H
        0xDFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, ((gb.cpu.pc >> 8) & 0xFF) as u8);
            gb.memory_write(sp2, (gb.cpu.pc & 0xFF) as u8);
            gb.cpu.sp = sp2;
            gb.cpu.pc = 0x18;
            CYCLE_RETURN_16
        },

        // LDH (u8), A
        0xE0u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let addr = 0xFF00 | value;
            gb.memory_write(addr, gb.cpu.a);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_12
        },

        // POP HL
        0xE1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let lo = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            let hi = gb.memory_read(gb.cpu.sp);
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            gb.cpu.h = hi;
            gb.cpu.l = lo;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_12
        },

        // LD (C), A
        0xE2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = 0xFF00 | (gb.cpu.c as u16);
            gb.memory_write(addr, gb.cpu.a);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // PUSH HL
        0xE5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, gb.cpu.h);
            gb.memory_write(sp2, gb.cpu.l);
            gb.cpu.sp = sp2;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_16
        },

        // AND u8
        0xE6u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let v = value as u8;
            and_register_with_value!(gb, a, v);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 20H
        0xE7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, ((gb.cpu.pc >> 8) & 0xFF) as u8);
            gb.memory_write(sp2, (gb.cpu.pc & 0xFF) as u8);
            gb.cpu.sp = sp2;
            gb.cpu.pc = 0x20;
            CYCLE_RETURN_16
        },


        // ADD SP, i8
        0xE8u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let value = (value as u8) as i8;
            let sp = gb.cpu.sp as i32;
            let r = sp + value as i32;
            let i8_32 = value as i32;

            clear_bits!(gb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC);

            if ((sp & 0xf) + (i8_32 & 0xf)) & 0x10 > 0xf {
                set_bits!(gb.cpu.f, BIT_FLAGH);
            }

            if (sp ^ i8_32 ^ r) & 0x100 == 0x100 {
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.sp = r as u16;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);

            CYCLE_RETURN_16
        },

        // JP (HL)
        0xE9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            CYCLE_RETURN_4
        },

        // LD (u16), A
        0xEAu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let addr = value;
            gb.memory_write(addr, gb.cpu.a);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_16
        },

        // XOR u8
        0xEEu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let v = value as u8;
            xor_register_with_value!(gb, a, v);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 28H
        0xEFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, ((gb.cpu.pc >> 8) & 0xFF) as u8);
            gb.memory_write(sp2, (gb.cpu.pc & 0xFF) as u8);
            gb.cpu.sp = sp2;
            gb.cpu.pc = 0x28;
            CYCLE_RETURN_16
        },

        // LDH A, (u8)
        0xF0u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let addr = 0xFF00 | value;
            gb.cpu.a = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_12
        },

        // POP AF
        0xF1u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.f = gb.memory_read(gb.cpu.sp) & 0xF0 & 0xF0;
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            gb.cpu.a = gb.memory_read(gb.cpu.sp) ;
            gb.cpu.sp = gb.cpu.sp.wrapping_add(1);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_12
        },

        // LD A, (C)
        0xF2u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let addr = 0xFF00 | (gb.cpu.c as u16);
            gb.cpu.a = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DI
        0xF3u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.ime = false;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // PUSH AF
        0xF5u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, gb.cpu.a);
            gb.memory_write(sp2, gb.cpu.f);
            gb.cpu.sp = sp2;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_16
        },

        // OR u8
        0xF6u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let v = value as u8;
            or_register_with_value!(gb, a, v);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 30H
        0xF7u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, ((gb.cpu.pc >> 8) & 0xFF) as u8);
            gb.memory_write(sp2, (gb.cpu.pc & 0xFF) as u8);
            gb.cpu.sp = sp2;
            gb.cpu.pc = 0x30;
            CYCLE_RETURN_16
        },

        // LD HL, SP + i8
        0xF8u8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let value = (value as u8) as i8;
            let sp = gb.cpu.sp as i32;
            let r = sp + value as i32;
            let i8_32 = value as i32;

            clear_bits!(gb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC);

            if ((sp & 0xf) + (i8_32 & 0xf)) & 0x10 > 0xf {
                set_bits!(gb.cpu.f, BIT_FLAGH);
            }

            if (sp ^ i8_32 ^ r) & 0x100 == 0x100 {
                set_bits!(gb.cpu.f, BIT_FLAGC);
            }

            gb.cpu.h = ((r >> 8) & 0xFF) as u8;
            gb.cpu.l = (r & 0xFF) as u8;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(2);

            CYCLE_RETURN_12
        },

        // LD SP, HL
        0xF9u8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.sp = ((gb.cpu.h as u16) << 8) | gb.cpu.l as u16;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (u16)
        0xFAu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let addr = value;
            gb.cpu.a = gb.memory_read(addr);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_16
        },

        // EI
        0xFBu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.ime = true;
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // CP u8
        0xFEu8 => |gb: &mut Gameboy, value: u16| -> OpCycles {
            let v = value as u8;
            compare_register_with_value!(gb, a, v);
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 38H
        0xFFu8 => |gb: &mut Gameboy, _value: u16| -> OpCycles {
            gb.cpu.pc = gb.cpu.pc.wrapping_add(1);

            let sp1 = gb.cpu.sp.wrapping_sub(1);
            let sp2 = gb.cpu.sp.wrapping_sub(2);
            gb.memory_write(sp1, ((gb.cpu.pc >> 8) & 0xFF) as u8);
            gb.memory_write(sp2, (gb.cpu.pc & 0xFF) as u8);
            gb.cpu.sp = sp2;
            gb.cpu.pc = 0x38;
            CYCLE_RETURN_16
        },

    }
}

pub fn op_code_names(addr: u8, cb_mode: bool) -> &'static str {
    let mut addr = addr as u16;
    if cb_mode {
        addr += 0x100;
    }

    let opcodes = [
        "NOP",
        "LD BC, d16",
        "LD (BC), A",
        "INC BC",
        "INC B",
        "DEC B",
        "LD B, d8",
        "RLCA",
        "LD (a16), SP",
        "ADD HL, BC",
        "LD A, (BC)",
        "DEC BC",
        "INC C",
        "DEC C",
        "LD C, d8",
        "RRCA",
        "STOP 0",
        "LD DE, d16",
        "LD (DE), A",
        "INC DE",
        "INC D",
        "DEC D",
        "LD D, d8",
        "RLA",
        "JR r8",
        "ADD HL, DE",
        "LD A, (DE)",
        "DEC DE",
        "INC E",
        "DEC E",
        "LD E, d8",
        "RRA",
        "JR NZ, r8",
        "LD HL, d16",
        "LD (HL+), A",
        "INC HL",
        "INC H",
        "DEC H",
        "LD H, d8",
        "DAA",
        "JR Z, r8",
        "ADD HL, HL",
        "LD A, (HL+)",
        "DEC HL",
        "INC L",
        "DEC L",
        "LD L, d8",
        "CPL",
        "JR NC, r8",
        "LD SP, d16",
        "LD (HL-), A",
        "INC SP",
        "INC (HL)",
        "DEC (HL)",
        "LD (HL), d8",
        "SCF",
        "JR C, r8",
        "ADD HL, SP",
        "LD A, (HL-)",
        "DEC SP",
        "INC A",
        "DEC A",
        "LD A, d8",
        "CCF",
        "LD B, B",
        "LD B, C",
        "LD B, D",
        "LD B, E",
        "LD B, H",
        "LD B, L",
        "LD B, (HL)",
        "LD B, A",
        "LD C, B",
        "LD C, C",
        "LD C, D",
        "LD C, E",
        "LD C, H",
        "LD C, L",
        "LD C, (HL)",
        "LD C, A",
        "LD D, B",
        "LD D, C",
        "LD D, D",
        "LD D, E",
        "LD D, H",
        "LD D, L",
        "LD D, (HL)",
        "LD D, A",
        "LD E, B",
        "LD E, C",
        "LD E, D",
        "LD E, E",
        "LD E, H",
        "LD E, L",
        "LD E, (HL)",
        "LD E, A",
        "LD H, B",
        "LD H, C",
        "LD H, D",
        "LD H, E",
        "LD H, H",
        "LD H, L",
        "LD H, (HL)",
        "LD H, A",
        "LD L, B",
        "LD L, C",
        "LD L, D",
        "LD L, E",
        "LD L, H",
        "LD L, L",
        "LD L, (HL)",
        "LD L, A",
        "LD (HL), B",
        "LD (HL), C",
        "LD (HL), D",
        "LD (HL), E",
        "LD (HL), H",
        "LD (HL), L",
        "HALT",
        "LD (HL), A",
        "LD A, B",
        "LD A, C",
        "LD A, D",
        "LD A, E",
        "LD A, H",
        "LD A, L",
        "LD A, (HL)",
        "LD A, A",
        "ADD A, B",
        "ADD A, C",
        "ADD A, D",
        "ADD A, E",
        "ADD A, H",
        "ADD A, L",
        "ADD A, (HL)",
        "ADD A, A",
        "ADC A, B",
        "ADC A, C",
        "ADC A, D",
        "ADC A, E",
        "ADC A, H",
        "ADC A, L",
        "ADC A, (HL)",
        "ADC A, A",
        "SUB B",
        "SUB C",
        "SUB D",
        "SUB E",
        "SUB H",
        "SUB L",
        "SUB (HL)",
        "SUB A",
        "SBC A, B",
        "SBC A, C",
        "SBC A, D",
        "SBC A, E",
        "SBC A, H",
        "SBC A, L",
        "SBC A, (HL)",
        "SBC A, A",
        "AND B",
        "AND C",
        "AND D",
        "AND E",
        "AND H",
        "AND L",
        "AND (HL)",
        "AND A",
        "XOR B",
        "XOR C",
        "XOR D",
        "XOR E",
        "XOR H",
        "XOR L",
        "XOR (HL)",
        "XOR A",
        "OR B",
        "OR C",
        "OR D",
        "OR E",
        "OR H",
        "OR L",
        "OR (HL)",
        "OR A",
        "CP B",
        "CP C",
        "CP D",
        "CP E",
        "CP H",
        "CP L",
        "CP (HL)",
        "CP A",
        "RET NZ",
        "POP BC",
        "JP NZ, a16",
        "JP a16",
        "CALL NZ, a16",
        "PUSH BC",
        "ADD A, d8",
        "RST 00H",
        "RET Z",
        "RET",
        "JP Z, a16",
        "PREFIX CB",
        "CALL Z, a16",
        "CALL a16",
        "ADC A, d8",
        "RST 08H",
        "RET NC",
        "POP DE",
        "JP NC, a16",
        "ILLEGAL",
        "CALL NC, a16",
        "PUSH DE",
        "SUB d8",
        "RST 10H",
        "RET C",
        "RETI",
        "JP C, a16",
        "ILLEGAL",
        "CALL C, a16",
        "ILLEGAL",
        "SBC A, d8",
        "RST 18H",
        "LDH (a8), A",
        "POP HL",
        "LD (C), A",
        "ILLEGAL",
        "ILLEGAL",
        "PUSH HL",
        "AND d8",
        "RST 20H",
        "ADD SP, r8",
        "JP (HL)",
        "LD (a16), A",
        "ILLEGAL",
        "ILLEGAL",
        "ILLEGAL",
        "XOR d8",
        "RST 28H",
        "LDH A, (a8)",
        "POP AF",
        "LD A, (C)",
        "DI",
        "ILLEGAL",
        "PUSH AF",
        "OR d8",
        "RST 30H",
        "LD HL, SP+r8",
        "LD SP, HL",
        "LD A, (a16)",
        "EI",
        "ILLEGAL",
        "ILLEGAL",
        "CP d8",
        "RST 38H",
        // CB prefix instructions do not take any arguments
        "RLC B",
        "RLC C",
        "RLC D",
        "RLC E",
        "RLC H",
        "RLC L",
        "RLC (HL)",
        "RLC A",
        "RRC B",
        "RRC C",
        "RRC D",
        "RRC E",
        "RRC H",
        "RRC L",
        "RRC (HL)",
        "RRC A",
        "RL B",
        "RL C",
        "RL D",
        "RL E",
        "RL H",
        "RL L",
        "RL (HL)",
        "RL A",
        "RR B",
        "RR C",
        "RR D",
        "RR E",
        "RR H",
        "RR L",
        "RR (HL)",
        "RR A",
        "SLA B",
        "SLA C",
        "SLA D",
        "SLA E",
        "SLA H",
        "SLA L",
        "SLA (HL)",
        "SLA A",
        "SRA B",
        "SRA C",
        "SRA D",
        "SRA E",
        "SRA H",
        "SRA L",
        "SRA (HL)",
        "SRA A",
        "SWAP B",
        "SWAP C",
        "SWAP D",
        "SWAP E",
        "SWAP H",
        "SWAP L",
        "SWAP (HL)",
        "SWAP A",
        "SRL B",
        "SRL C",
        "SRL D",
        "SRL E",
        "SRL H",
        "SRL L",
        "SRL (HL)",
        "SRL A",
        "BIT 0, B",
        "BIT 0, C",
        "BIT 0, D",
        "BIT 0, E",
        "BIT 0, H",
        "BIT 0, L",
        "BIT 0, (HL)",
        "BIT 0, A",
        "BIT 1, B",
        "BIT 1, C",
        "BIT 1, D",
        "BIT 1, E",
        "BIT 1, H",
        "BIT 1, L",
        "BIT 1, (HL)",
        "BIT 1, A",
        "BIT 2, B",
        "BIT 2, C",
        "BIT 2, D",
        "BIT 2, E",
        "BIT 2, H",
        "BIT 2, L",
        "BIT 2, (HL)",
        "BIT 2, A",
        "BIT 3, B",
        "BIT 3, C",
        "BIT 3, D",
        "BIT 3, E",
        "BIT 3, H",
        "BIT 3, L",
        "BIT 3, (HL)",
        "BIT 3, A",
        "BIT 4, B",
        "BIT 4, C",
        "BIT 4, D",
        "BIT 4, E",
        "BIT 4, H",
        "BIT 4, L",
        "BIT 4, (HL)",
        "BIT 4, A",
        "BIT 5, B",
        "BIT 5, C",
        "BIT 5, D",
        "BIT 5, E",
        "BIT 5, H",
        "BIT 5, L",
        "BIT 5, (HL)",
        "BIT 5, A",
        "BIT 6, B",
        "BIT 6, C",
        "BIT 6, D",
        "BIT 6, E",
        "BIT 6, H",
        "BIT 6, L",
        "BIT 6, (HL)",
        "BIT 6, A",
        "BIT 7, B",
        "BIT 7, C",
        "BIT 7, D",
        "BIT 7, E",
        "BIT 7, H",
        "BIT 7, L",
        "BIT 7, (HL)",
        "BIT 7, A",
        "RES 0, B",
        "RES 0, C",
        "RES 0, D",
        "RES 0, E",
        "RES 0, H",
        "RES 0, L",
        "RES 0, (HL)",
        "RES 0, A",
        "RES 1, B",
        "RES 1, C",
        "RES 1, D",
        "RES 1, E",
        "RES 1, H",
        "RES 1, L",
        "RES 1, (HL)",
        "RES 1, A",
        "RES 2, B",
        "RES 2, C",
        "RES 2, D",
        "RES 2, E",
        "RES 2, H",
        "RES 2, L",
        "RES 2, (HL)",
        "RES 2, A",
        "RES 3, B",
        "RES 3, C",
        "RES 3, D",
        "RES 3, E",
        "RES 3, H",
        "RES 3, L",
        "RES 3, (HL)",
        "RES 3, A",
        "RES 4, B",
        "RES 4, C",
        "RES 4, D",
        "RES 4, E",
        "RES 4, H",
        "RES 4, L",
        "RES 4, (HL)",
        "RES 4, A",
        "RES 5, B",
        "RES 5, C",
        "RES 5, D",
        "RES 5, E",
        "RES 5, H",
        "RES 5, L",
        "RES 5, (HL)",
        "RES 5, A",
        "RES 6, B",
        "RES 6, C",
        "RES 6, D",
        "RES 6, E",
        "RES 6, H",
        "RES 6, L",
        "RES 6, (HL)",
        "RES 6, A",
        "RES 7, B",
        "RES 7, C",
        "RES 7, D",
        "RES 7, E",
        "RES 7, H",
        "RES 7, L",
        "RES 7, (HL)",
        "RES 7, A",
        "SET 0, B",
        "SET 0, C",
        "SET 0, D",
        "SET 0, E",
        "SET 0, H",
        "SET 0, L",
        "SET 0, (HL)",
        "SET 0, A",
        "SET 1, B",
        "SET 1, C",
        "SET 1, D",
        "SET 1, E",
        "SET 1, H",
        "SET 1, L",
        "SET 1, (HL)",
        "SET 1, A",
        "SET 2, B",
        "SET 2, C",
        "SET 2, D",
        "SET 2, E",
        "SET 2, H",
        "SET 2, L",
        "SET 2, (HL)",
        "SET 2, A",
        "SET 3, B",
        "SET 3, C",
        "SET 3, D",
        "SET 3, E",
        "SET 3, H",
        "SET 3, L",
        "SET 3, (HL)",
        "SET 3, A",
        "SET 4, B",
        "SET 4, C",
        "SET 4, D",
        "SET 4, E",
        "SET 4, H",
        "SET 4, L",
        "SET 4, (HL)",
        "SET 4, A",
        "SET 5, B",
        "SET 5, C",
        "SET 5, D",
        "SET 5, E",
        "SET 5, H",
        "SET 5, L",
        "SET 5, (HL)",
        "SET 5, A",
        "SET 6, B",
        "SET 6, C",
        "SET 6, D",
        "SET 6, E",
        "SET 6, H",
        "SET 6, L",
        "SET 6, (HL)",
        "SET 6, A",
        "SET 7, B",
        "SET 7, C",
        "SET 7, D",
        "SET 7, E",
        "SET 7, H",
        "SET 7, L",
        "SET 7, (HL)",
        "SET 7, A",
    ];

    opcodes[addr as usize]
}
