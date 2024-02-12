use crate::{bits::*, gameboy::Motherboard, globals::*, utils::*};

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
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGZ, BIT_FLAGH);


            match is_bit_set(mb.cpu.a, 7){
                true => {
                    set_bit(&mut mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a = (mb.cpu.a << 1) + 1;
                },
                false => {
                    clear_bit(&mut mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a <<= 1;
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
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

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
            clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            match is_bit_set(mb.cpu.a, 0){
                true => {
                    set_bit(&mut mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a = (mb.cpu.a >> 1) | 0x80;
                },
                false => {
                    clear_bit(&mut mb.cpu.f, BIT_FLAGC);
                    mb.cpu.a >>= 1;
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
            clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);

            match is_bit_set(mb.cpu.a, 7){
                true => set_bit(&mut mb.cpu.f, BIT_FLAGC),
                false => clear_bit(&mut mb.cpu.f, BIT_FLAGC),
            }

            mb.cpu.a <<= 1;
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
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

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
            clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH);

            let carry = is_bit_set(mb.cpu.f, BIT_FLAGC);

            match is_bit_set(mb.cpu.a, 0){
                true => set_bit(&mut mb.cpu.f, BIT_FLAGC),
                false => clear_bit(&mut mb.cpu.f, BIT_FLAGC),
            }

            mb.cpu.a >>= 1;
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

        // LD HL, u16
        0x21u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.h = (value >> 8) as u8;
            mb.cpu.l = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_12
        },

        // LD (HL+), A
        0x22u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.a);
            let result = addr.wrapping_add(1);
            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC HL
        0x23u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let value = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let result = value.wrapping_add(1);
            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC H
        0x24u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            increment_register!(mb, h);
            CYCLE_RETURN_4
        },

        // DEC H
        0x25u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            decrement_register!(mb, h);
            CYCLE_RETURN_4
        },

        // LD H, u8
        0x26u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.h = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // DAA
        0x27u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let mut corr: u8 = 0;

            if is_bit_set(mb.cpu.f, BIT_FLAGH){
                corr |= 0x06;
            }

            if is_bit_set(mb.cpu.f, BIT_FLAGC){
                corr |= 0x60;
            }

            if is_bit_set(mb.cpu.f, BIT_FLAGN){
               mb.cpu.a = mb.cpu.a.wrapping_sub(corr);
            }else{
                if (mb.cpu.a & 0x0F) > 9 {
                    corr |= 0x06;
                }

                if mb.cpu.a > 0x99 {
                    corr |= 0x60;
                }

                mb.cpu.a = mb.cpu.a.wrapping_add(corr);
            }

            let mut flag: u8 = 0;
            if mb.cpu.a == 0 {
                set_bit(&mut flag, BIT_FLAGZ);
            }

            if corr & 0x60 != 0 {
                set_bit(&mut flag, BIT_FLAGC);
            }

            mb.cpu.f &= 0x40;
            mb.cpu.f |= flag;

            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // JR Z, r8 - Relative jump if last result was zero
        0x28u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                mb.cpu.pc = (mb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
                CYCLE_RETURN_12
            }else {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN_8
            }
        },

        // ADD HL, HL
        0x29u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

            let hl = ((mb.cpu.h as u16) << 8 | mb.cpu.l as u16) as u32;
            let result = hl + hl;

            if result & 0x10000 != 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGC);
            }

            if (hl ^ result ^ hl) & 0x1000 != 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGH);
            }

            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;

            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (HL+)
        0x2Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.a = memory_read(addr);
            let result = addr.wrapping_add(1);
            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DEC HL
        0x2Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let value = (mb.cpu.h as u16) << 8 | mb.cpu.l as u16;
            let result = value.wrapping_sub(1);
            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC L
        0x2Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            increment_register!(mb, l);
            CYCLE_RETURN_4
        },

        // DEC L
        0x2Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            decrement_register!(mb, l);
            CYCLE_RETURN_4
        },

        // LD L, u8
        0x2Eu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.l = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // CPL - Complement A
        0x2Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.a = !mb.cpu.a;
            set_bit(&mut mb.cpu.f, BIT_FLAGN);
            set_bit(&mut mb.cpu.f, BIT_FLAGH);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // JR NC, r8 - Relative jump if last result was not carry
        0x30u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if !is_bit_set(mb.cpu.f, BIT_FLAGC) {
                mb.cpu.pc = (mb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
                CYCLE_RETURN_12
            }else {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN_8
            }
        },

        // LD SP, u16
        0x31u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.sp = value;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_12
        },

        // LD (HL-), A
        0x32u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.a);
            let result = addr.wrapping_sub(1);
            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC SP
        0x33u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC (HL)
        0x34u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(hl);
            let result = value.wrapping_add(1);

            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGZ, BIT_FLAGH);

            if result == 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGZ);
            }

            if (value & 0x0F) == 0x0F {
                set_bit(&mut mb.cpu.f, BIT_FLAGH);
            }

            memory_write(hl, result);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            CYCLE_RETURN_12
        },

        // DEC (HL)
        0x35u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let hl = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(hl);
            let result = value.wrapping_sub(1);

            set_bit(&mut mb.cpu.f, BIT_FLAGN);
            clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGH);

            if result == 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGZ);
            }

            if (value & 0x0F) == 0x0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGH);
            }

            memory_write(hl, result);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            CYCLE_RETURN_12
        },

        // LD (HL), u8
        0x36u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let hl = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(hl, value as u8);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_12
        },

        // SCF - Set carry flag
        0x37u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH);
            set_bit(&mut mb.cpu.f, BIT_FLAGC);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // JR C, r8 - Relative jump if last result was carry
        0x38u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if is_bit_set(mb.cpu.f, BIT_FLAGC) {
                mb.cpu.pc = (mb.cpu.pc.wrapping_add((value as i8) as u16)).wrapping_add(2);
                CYCLE_RETURN_12
            }else {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
                CYCLE_RETURN_8
            }
        },

        // ADD HL, SP
        0x39u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            clear_bits!(mb.cpu.f, BIT_FLAGC, BIT_FLAGN, BIT_FLAGH);

            let hl = ((mb.cpu.h as u16) << 8 | mb.cpu.l as u16) as u32;
            let result = hl + mb.cpu.sp as u32;

            if result & 0x10000 != 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGC);
            }

            if (hl ^ mb.cpu.sp as u32 ^ result) & 0x1000 != 0 {
                set_bit(&mut mb.cpu.f, BIT_FLAGH);
            }

            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;

            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (HL-)
        0x3Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.a = memory_read(addr);
            let result = addr.wrapping_sub(1);
            mb.cpu.h = (result >> 8) as u8;
            mb.cpu.l = result as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DEC SP
        0x3Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.sp = mb.cpu.sp.wrapping_sub(1);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // INC A
        0x3Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            increment_register!(mb, a);
            CYCLE_RETURN_4
        },

        // DEC A
        0x3Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            decrement_register!(mb, a);
            CYCLE_RETURN_4
        },

        // LD A, u8
        0x3Eu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.a = value as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_8
        },

        // CCF - Complement carry flag
        0x3Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {

            clear_bits!(mb.cpu.f, BIT_FLAGN, BIT_FLAGH);
            if is_bit_set(mb.cpu.f, BIT_FLAGC) {
                clear_bit(&mut mb.cpu.f, BIT_FLAGC);
            } else {
                set_bit(&mut mb.cpu.f, BIT_FLAGC);
            }
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, B
        0x40u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, C
        0x41u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.b = mb.cpu.c;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, D
        0x42u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.b = mb.cpu.d;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, E
        0x43u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.b = mb.cpu.e;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, H
        0x44u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.b = mb.cpu.h;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, L
        0x45u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.b = mb.cpu.l;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD B, (HL)
        0x46u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.b = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD B, A
        0x47u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.b = mb.cpu.a;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, B
        0x48u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.c = mb.cpu.b;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, C
        0x49u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, D
        0x4Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.c = mb.cpu.d;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, E
        0x4Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.c = mb.cpu.e;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, H
        0x4Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.c = mb.cpu.h;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, L
        0x4Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.c = mb.cpu.l;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD C, (HL)
        0x4Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.c = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD C, A
        0x4Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.c = mb.cpu.a;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, B
        0x50u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.d = mb.cpu.b;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, C
        0x51u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.d = mb.cpu.c;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, D
        0x52u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, E
        0x53u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.d = mb.cpu.e;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, H
        0x54u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.d = mb.cpu.h;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, L
        0x55u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.d = mb.cpu.l;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD D, (HL)
        0x56u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.d = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD D, A
        0x57u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.d = mb.cpu.a;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, B
        0x58u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.e = mb.cpu.b;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, C
        0x59u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.e = mb.cpu.c;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, D
        0x5Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.e = mb.cpu.d;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, E
        0x5Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, H
        0x5Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.e = mb.cpu.h;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, L
        0x5Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.e = mb.cpu.l;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD E, (HL)
        0x5Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.e = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD E, A
        0x5Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.e = mb.cpu.a;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, B
        0x60u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.h = mb.cpu.b;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, C
        0x61u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.h = mb.cpu.c;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, D
        0x62u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.h = mb.cpu.d;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, E
        0x63u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.h = mb.cpu.e;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, H
        0x64u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, L
        0x65u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.h = mb.cpu.l;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD H, (HL)
        0x66u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.h = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD H, A
        0x67u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.h = mb.cpu.a;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, B
        0x68u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.l = mb.cpu.b;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, C
        0x69u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.l = mb.cpu.c;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, D
        0x6Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.l = mb.cpu.d;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, E
        0x6Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.l = mb.cpu.e;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, H
        0x6Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.l = mb.cpu.h;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, L
        0x6Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD L, (HL)
        0x6Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.l = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD L, A
        0x6Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.l = mb.cpu.a;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD (HL), B
        0x70u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.b);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), C
        0x71u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.c);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), D
        0x72u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.d);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), E
        0x73u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.e);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), H
        0x74u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.h);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD (HL), L
        0x75u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.l);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // HALT
        0x76u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.halted = true;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD (HL), A
        0x77u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            memory_write(addr, mb.cpu.a);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, B
        0x78u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.a = mb.cpu.b;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, C
        0x79u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.a = mb.cpu.c;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, D
        0x7Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.a = mb.cpu.d;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, E
        0x7Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.a = mb.cpu.e;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, H
        0x7Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.a = mb.cpu.h;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, L
        0x7Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.a = mb.cpu.l;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // LD A, (HL)
        0x7Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.a = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, A
        0x7Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // ADD A, B
        0x80u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_register!(mb, a, b);
            CYCLE_RETURN_4
        },

        // ADD A, C
        0x81u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_register!(mb, a, c);
            CYCLE_RETURN_4
        },

        // ADD A, D
        0x82u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_register!(mb, a, d);
            CYCLE_RETURN_4
        },

        // ADD A, E
        0x83u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_register!(mb, a, e);
            CYCLE_RETURN_4
        },

        // ADD A, H
        0x84u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_register!(mb, a, h);
            CYCLE_RETURN_4
        },

        // ADD A, L
        0x85u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_register!(mb, a, l);
            CYCLE_RETURN_4
        },

        // ADD A, (HL)
        0x86u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(addr);
            add_register_from_value!(mb, a, value);
            CYCLE_RETURN_8
        },

        // ADD A, A
        0x87u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_register!(mb, a, a);
            CYCLE_RETURN_4
        },

        // ADC A, B
        0x88u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_carry_register!(mb, a, b);
            CYCLE_RETURN_4
        },

        // ADC A, C
        0x89u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_carry_register!(mb, a, c);
            CYCLE_RETURN_4
        },

        // ADC A, D
        0x8Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_carry_register!(mb, a, d);
            CYCLE_RETURN_4
        },

        // ADC A, E
        0x8Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_carry_register!(mb, a, e);
            CYCLE_RETURN_4
        },

        // ADC A, H
        0x8Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_carry_register!(mb, a, h);
            CYCLE_RETURN_4
        },

        // ADC A, L
        0x8Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_carry_register!(mb, a, l);
            CYCLE_RETURN_4
        },

        // ADC A, (HL)
        0x8Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(addr);
            add_carry_register_from_value!(mb, a, value);
            CYCLE_RETURN_8
        },

        // ADC A, A
        0x8Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            add_carry_register!(mb, a, a);
            CYCLE_RETURN_4
        },

        // SUB B
        0x90u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_register!(mb, a, b);
            CYCLE_RETURN_4
        },

        // SUB C
        0x91u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_register!(mb, a, c);
            CYCLE_RETURN_4
        },

        // SUB D
        0x92u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_register!(mb, a, d);
            CYCLE_RETURN_4
        },

        // SUB E
        0x93u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_register!(mb, a, e);
            CYCLE_RETURN_4
        },

        // SUB H
        0x94u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_register!(mb, a, h);
            CYCLE_RETURN_4
        },

        // SUB L
        0x95u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_register!(mb, a, l);
            CYCLE_RETURN_4
        },

        // SUB (HL)
        0x96u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(addr);
            sub_register_from_value!(mb, a, value);
            CYCLE_RETURN_8
        },

        // SUB A
        0x97u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_register!(mb, a, a);
            CYCLE_RETURN_4
        },

        // SBC A, B
        0x98u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_carry_register!(mb, a, b);
            CYCLE_RETURN_4
        },

        // SBC A, C
        0x99u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_carry_register!(mb, a, c);
            CYCLE_RETURN_4
        },

        // SBC A, D
        0x9Au8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_carry_register!(mb, a, d);
            CYCLE_RETURN_4
        },

        // SBC A, E
        0x9Bu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_carry_register!(mb, a, e);
            CYCLE_RETURN_4
        },

        // SBC A, H
        0x9Cu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_carry_register!(mb, a, h);
            CYCLE_RETURN_4
        },

        // SBC A, L
        0x9Du8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_carry_register!(mb, a, l);
            CYCLE_RETURN_4
        },

        // SBC A, (HL)
        0x9Eu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(addr);
            sub_carry_register_from_value!(mb, a, value);
            CYCLE_RETURN_8
        },

        // SBC A, A
        0x9Fu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            sub_carry_register!(mb, a, a);
            CYCLE_RETURN_4
        },

        // AND B
        0xA0u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            and_register!(mb, a, b);
            CYCLE_RETURN_4
        },

        // AND C
        0xA1u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            and_register!(mb, a, c);
            CYCLE_RETURN_4
        },

        // AND D
        0xA2u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            and_register!(mb, a, d);
            CYCLE_RETURN_4
        },

        // AND E
        0xA3u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            and_register!(mb, a, e);
            CYCLE_RETURN_4
        },

        // AND H
        0xA4u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            and_register!(mb, a, h);
            CYCLE_RETURN_4
        },

        // AND L
        0xA5u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            and_register!(mb, a, l);
            CYCLE_RETURN_4
        },

        // AND (HL)
        0xA6u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(addr);
            and_register_with_value!(mb, a, value);
            CYCLE_RETURN_8
        },

        // AND A
        0xA7u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            and_register!(mb, a, a);
            CYCLE_RETURN_4
        },

        // XOR B
        0xA8u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            xor_register!(mb, a, b);
            CYCLE_RETURN_4
        },

        // XOR C
        0xA9u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            xor_register!(mb, a, c);
            CYCLE_RETURN_4
        },

        // XOR D
        0xAAu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            xor_register!(mb, a, d);
            CYCLE_RETURN_4
        },

        // XOR E
        0xABu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            xor_register!(mb, a, e);
            CYCLE_RETURN_4
        },

        // XOR H
        0xACu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            xor_register!(mb, a, h);
            CYCLE_RETURN_4
        },

        // XOR L
        0xADu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            xor_register!(mb, a, l);
            CYCLE_RETURN_4
        },

        // XOR (HL)
        0xAEu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(addr);
            xor_register_with_value!(mb, a, value);
            CYCLE_RETURN_8
        },

        // XOR A
        0xAFu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            xor_register!(mb, a, a);
            CYCLE_RETURN_4
        },

        // OR B
        0xB0u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            or_register!(mb, a, b);
            CYCLE_RETURN_4
        },

        // OR C
        0xB1u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            or_register!(mb, a, c);
            CYCLE_RETURN_4
        },

        // OR D
        0xB2u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            or_register!(mb, a, d);
            CYCLE_RETURN_4
        },

        // OR E
        0xB3u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            or_register!(mb, a, e);
            CYCLE_RETURN_4
        },

        // OR H
        0xB4u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            or_register!(mb, a, h);
            CYCLE_RETURN_4
        },

        // OR L
        0xB5u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            or_register!(mb, a, l);
            CYCLE_RETURN_4
        },

        // OR (HL)
        0xB6u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(addr);
            or_register_with_value!(mb, a, value);
            CYCLE_RETURN_8
        },

        // OR A
        0xB7u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            or_register!(mb, a, a);
            CYCLE_RETURN_4
        },

        // CP B
        0xB8u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            compare_register!(mb, a, b);
            CYCLE_RETURN_4
        },

        // CP C
        0xB9u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            compare_register!(mb, a, c);
            CYCLE_RETURN_4
        },

        // CP D
        0xBAu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            compare_register!(mb, a, d);
            CYCLE_RETURN_4
        },

        // CP E
        0xBBu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            compare_register!(mb, a, e);
            CYCLE_RETURN_4
        },

        // CP H
        0xBCu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            compare_register!(mb, a, h);
            CYCLE_RETURN_4
        },

        // CP L
        0xBDu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            compare_register!(mb, a, l);
            CYCLE_RETURN_4
        },

        // CP (HL)
        0xBEu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            let value = memory_read(addr);
            compare_register_with_value!(mb, a, value);
            CYCLE_RETURN_8
        },

        // CP A
        0xBFu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            compare_register!(mb, a, a);
            CYCLE_RETURN_4
        },

        // RET NZ
        0xC0u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            if !is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                let lo = memory_read(mb.cpu.sp);
                mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
                let hi = memory_read(mb.cpu.sp);
                mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
                mb.cpu.pc = ((hi as u16) << 8) | lo as u16;
                CYCLE_RETURN_20
            } else {
                mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
                CYCLE_RETURN_8
            }
        },

        // POP BC
        0xC1u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let lo = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            let hi = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            mb.cpu.b = hi;
            mb.cpu.c = lo;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_12
        },

        // JP NZ, u16
        0xC2u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if !is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                mb.cpu.pc = value;
                CYCLE_RETURN_16
            } else {
                mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
                CYCLE_RETURN_12
            }
        },

        // JP, u16 - Absolute jump
        0xC3u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.pc = value;
            CYCLE_RETURN_16
        },

        // CALL NZ, u16
        0xC4u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);

            if !is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                let sp1 = mb.cpu.sp.wrapping_sub(1);
                let sp2 = mb.cpu.sp.wrapping_sub(2);

                let pch = ((mb.cpu.pc >> 8) & 0xFF) as u8;
                let pcl = (mb.cpu.pc & 0xFF) as u8;
                memory_write(sp1, pch);
                memory_write(sp2, pcl);
                mb.cpu.sp = sp2;
                mb.cpu.pc = value;
                CYCLE_RETURN_24
            }else {
                CYCLE_RETURN_12
            }
        },

        // PUSH BC
        0xC5u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, mb.cpu.b);
            memory_write(sp2, mb.cpu.c);
            mb.cpu.sp = sp2;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_16
        },

        // ADD A, u8
        0xC6u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let v = value as u8;
            add_register_from_value!(mb, a, v);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 00H
        0xC7u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, ((mb.cpu.pc >> 8) & 0xFF) as u8);
            memory_write(sp2, (mb.cpu.pc & 0xFF) as u8);
            mb.cpu.sp = sp2;
            mb.cpu.pc = 0x00;
            CYCLE_RETURN_16
        },

        // RET Z
        0xC8u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            if is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                let lo = memory_read(mb.cpu.sp);
                mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
                let hi = memory_read(mb.cpu.sp);
                mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
                mb.cpu.pc = ((hi as u16) << 8) | lo as u16;
                CYCLE_RETURN_20
            } else {
                mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
                CYCLE_RETURN_8
            }
        },

        // RET
        0xC9u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let lo = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            let hi = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            mb.cpu.pc = ((hi as u16) << 8) | lo as u16;
            CYCLE_RETURN_16
        },

        // JP Z, u16
        0xCAu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                mb.cpu.pc = value;
                CYCLE_RETURN_16
            } else {
                mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
                CYCLE_RETURN_12
            }
        },

        // PREFIX CB
        0xCBu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            // let opcode = memory_read(mb.cpu.pc);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            let cb_opcode = memory_read(mb.cpu.pc);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            mb.execute_op_code_cb(cb_opcode).expect("Failed to execute CB opcode")
        },

        // CALL Z, u16
        0xCCu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);

            if is_bit_set(mb.cpu.f, BIT_FLAGZ) {
                let sp1 = mb.cpu.sp.wrapping_sub(1);
                let sp2 = mb.cpu.sp.wrapping_sub(2);

                let pch = ((mb.cpu.pc >> 8) & 0xFF) as u8;
                let pcl = (mb.cpu.pc & 0xFF) as u8;
                memory_write(sp1, pch);
                memory_write(sp2, pcl);
                mb.cpu.sp = sp2;
                mb.cpu.pc = value;
                CYCLE_RETURN_24
            } else {
                CYCLE_RETURN_12
            }
        },

        // CALL u16
        0xCDu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);

            let pch = ((mb.cpu.pc >> 8) & 0xFF) as u8;
            let pcl = (mb.cpu.pc & 0xFF) as u8;
            memory_write(sp1, pch);
            memory_write(sp2, pcl);
            mb.cpu.sp = sp2;
            mb.cpu.pc = value;
            CYCLE_RETURN_24
        },

        // ADC A, u8
        0xCEu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let v = value as u8;
            add_carry_register_from_value!(mb, a, v);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 08H
        0xCFu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, ((mb.cpu.pc >> 8) & 0xFF) as u8);
            memory_write(sp2, (mb.cpu.pc & 0xFF) as u8);
            mb.cpu.sp = sp2;
            mb.cpu.pc = 0x08;
            CYCLE_RETURN_16
        },

        // RET NC
        0xD0u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            if !is_bit_set(mb.cpu.f, BIT_FLAGC) {
                let lo = memory_read(mb.cpu.sp);
                mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
                let hi = memory_read(mb.cpu.sp);
                mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
                mb.cpu.pc = ((hi as u16) << 8) | lo as u16;
                CYCLE_RETURN_20
            } else {
                mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
                CYCLE_RETURN_8
            }
        },

        // POP DE
        0xD1u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let lo = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            let hi = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            mb.cpu.d = hi;
            mb.cpu.e = lo;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_12
        },

        // JP NC, u16
        0xD2u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if !is_bit_set(mb.cpu.f, BIT_FLAGC) {
                mb.cpu.pc = value;
                CYCLE_RETURN_16
            } else {
                mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
                CYCLE_RETURN_12
            }
        },

        // CALL NC, u16
        0xD4u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);

            if !is_bit_set(mb.cpu.f, BIT_FLAGC) {
                let sp1 = mb.cpu.sp.wrapping_sub(1);
                let sp2 = mb.cpu.sp.wrapping_sub(2);

                let pch = ((mb.cpu.pc >> 8) & 0xFF) as u8;
                let pcl = (mb.cpu.pc & 0xFF) as u8;
                memory_write(sp1, pch);
                memory_write(sp2, pcl);
                mb.cpu.sp = sp2;
                mb.cpu.pc = value;
                CYCLE_RETURN_24
            } else {
                CYCLE_RETURN_12
            }
        },

        // PUSH DE
        0xD5u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, mb.cpu.d);
            memory_write(sp2, mb.cpu.e);
            mb.cpu.sp = sp2;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_16
        },

        // SUB u8
        0xD6u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let v = value as u8;
            sub_register_from_value!(mb, a, v);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 10H
        0xD7u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, ((mb.cpu.pc >> 8) & 0xFF) as u8);
            memory_write(sp2, (mb.cpu.pc & 0xFF) as u8);
            mb.cpu.sp = sp2;
            mb.cpu.pc = 0x10;
            CYCLE_RETURN_16
        },

        // RET C
        0xD8u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            if is_bit_set(mb.cpu.f, BIT_FLAGC) {
                let lo = memory_read(mb.cpu.sp);
                mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
                let hi = memory_read(mb.cpu.sp);
                mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
                mb.cpu.pc = ((hi as u16) << 8) | lo as u16;
                CYCLE_RETURN_20
            } else {
                mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
                CYCLE_RETURN_8
            }
        },

        // RETI
        0xD9u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let lo = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            let hi = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            mb.cpu.pc = ((hi as u16) << 8) | lo as u16;
            mb.ime = true;
            CYCLE_RETURN_16
        },

        // JP C, u16
        0xDAu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            if is_bit_set(mb.cpu.f, BIT_FLAGC) {
                mb.cpu.pc = value;
                CYCLE_RETURN_16
            } else {
                mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
                CYCLE_RETURN_12
            }
        },

        // CALL C, u16
        0xDCu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);

            if is_bit_set(mb.cpu.f, BIT_FLAGC) {
                let sp1 = mb.cpu.sp.wrapping_sub(1);
                let sp2 = mb.cpu.sp.wrapping_sub(2);

                let pch = ((mb.cpu.pc >> 8) & 0xFF) as u8;
                let pcl = (mb.cpu.pc & 0xFF) as u8;
                memory_write(sp1, pch);
                memory_write(sp2, pcl);
                mb.cpu.sp = sp2;
                mb.cpu.pc = value;
                CYCLE_RETURN_24
            } else {
                CYCLE_RETURN_12
            }
        },

        // SBC A, u8
        0xDEu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let v = value as u8;
            sub_carry_register_from_value!(mb, a, v);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 18H
        0xDFu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, ((mb.cpu.pc >> 8) & 0xFF) as u8);
            memory_write(sp2, (mb.cpu.pc & 0xFF) as u8);
            mb.cpu.sp = sp2;
            mb.cpu.pc = 0x18;
            CYCLE_RETURN_16
        },

        // LDH (u8), A
        0xE0u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let addr = 0xFF00 | (value as u16);
            memory_write(addr, mb.cpu.a);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_12
        },

        // POP HL
        0xE1u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let lo = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            let hi = memory_read(mb.cpu.sp);
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            mb.cpu.h = hi;
            mb.cpu.l = lo;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_12
        },

        // LD (C), A
        0xE2u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = 0xFF00 | (mb.cpu.c as u16);
            memory_write(addr, mb.cpu.a);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // PUSH HL
        0xE5u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, mb.cpu.h);
            memory_write(sp2, mb.cpu.l);
            mb.cpu.sp = sp2;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_16
        },

        // AND u8
        0xE6u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let v = value as u8;
            and_register_with_value!(mb, a, v);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 20H
        0xE7u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, ((mb.cpu.pc >> 8) & 0xFF) as u8);
            memory_write(sp2, (mb.cpu.pc & 0xFF) as u8);
            mb.cpu.sp = sp2;
            mb.cpu.pc = 0x20;
            CYCLE_RETURN_16
        },


        // ADD SP, i8
        0xE8u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let value = (value as u8) as i8;
            let sp = mb.cpu.sp as i32;
            let r = sp + value as i32;
            let i8_32 = value as i32;

            clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC);

            if ((sp & 0xf) + (i8_32 & 0xf)) & 0x10 > 0xf {
                set_bits!(mb.cpu.f, BIT_FLAGH);
            }

            if (sp ^ i8_32 ^ r) & 0x100 == 0x100 {
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.sp = r as u16;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);

            CYCLE_RETURN_16
        },

        // JP (HL)
        0xE9u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            CYCLE_RETURN_4
        },

        // LD (u16), A
        0xEAu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let addr = value;
            memory_write(addr, mb.cpu.a);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_16
        },

        // XOR u8
        0xEEu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let v = value as u8;
            xor_register_with_value!(mb, a, v);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 28H
        0xEFu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, ((mb.cpu.pc >> 8) & 0xFF) as u8);
            memory_write(sp2, (mb.cpu.pc & 0xFF) as u8);
            mb.cpu.sp = sp2;
            mb.cpu.pc = 0x28;
            CYCLE_RETURN_16
        },

        // LDH A, (u8)
        0xF0u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let addr = 0xFF00 | (value as u16);
            mb.cpu.a = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);
            CYCLE_RETURN_12
        },

        // POP AF
        0xF1u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.f = memory_read(mb.cpu.sp) & 0xF0 & 0xF0;
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            mb.cpu.a = memory_read(mb.cpu.sp) ;
            mb.cpu.sp = mb.cpu.sp.wrapping_add(1);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_12
        },

        // LD A, (C)
        0xF2u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let addr = 0xFF00 | (mb.cpu.c as u16);
            mb.cpu.a = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // DI
        0xF3u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.ime = false;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // PUSH AF
        0xF5u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, mb.cpu.a);
            memory_write(sp2, mb.cpu.f);
            mb.cpu.sp = sp2;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_16
        },

        // OR u8
        0xF6u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let v = value as u8;
            or_register_with_value!(mb, a, v);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 30H
        0xF7u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, ((mb.cpu.pc >> 8) & 0xFF) as u8);
            memory_write(sp2, (mb.cpu.pc & 0xFF) as u8);
            mb.cpu.sp = sp2;
            mb.cpu.pc = 0x30;
            CYCLE_RETURN_16
        },

        // LD HL, SP + i8
        0xF8u8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let value = (value as u8) as i8;
            let sp = mb.cpu.sp as i32;
            let r = sp + value as i32;
            let i8_32 = value as i32;

            clear_bits!(mb.cpu.f, BIT_FLAGZ, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC);

            if ((sp & 0xf) + (i8_32 & 0xf)) & 0x10 > 0xf {
                set_bits!(mb.cpu.f, BIT_FLAGH);
            }

            if (sp ^ i8_32 ^ r) & 0x100 == 0x100 {
                set_bits!(mb.cpu.f, BIT_FLAGC);
            }

            mb.cpu.h = ((r >> 8) & 0xFF) as u8;
            mb.cpu.l = (r & 0xFF) as u8;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(2);

            CYCLE_RETURN_12
        },

        // LD SP, HL
        0xF9u8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.sp = ((mb.cpu.h as u16) << 8) | mb.cpu.l as u16;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // LD A, (u16)
        0xFAu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let addr = value;
            mb.cpu.a = memory_read(addr);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(3);
            CYCLE_RETURN_16
        },

        // EI
        0xFBu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.ime = true;
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_4
        },

        // CP u8
        0xFEu8 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            let v = value as u8;
            compare_register_with_value!(mb, a, v);
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);
            CYCLE_RETURN_8
        },

        // RST 38H
        0xFFu8 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.pc = mb.cpu.pc.wrapping_add(1);

            let sp1 = mb.cpu.sp.wrapping_sub(1);
            let sp2 = mb.cpu.sp.wrapping_sub(2);
            memory_write(sp1, ((mb.cpu.pc >> 8) & 0xFF) as u8);
            memory_write(sp2, (mb.cpu.pc & 0xFF) as u8);
            mb.cpu.sp = sp2;
            mb.cpu.pc = 0x38;
            CYCLE_RETURN_16
        },

    }
}
