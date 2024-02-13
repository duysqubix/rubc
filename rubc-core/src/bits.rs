#[inline(always)]
pub const fn is_bit_set(value: u8, bit: u8) -> bool {
    (value & (1 << bit)) != 0
}

#[inline(always)]
pub fn set_bit(value: &mut u8, bit: u8) {
    *value |= 1 << bit;
}

#[inline(always)]
pub fn clear_bit(value: &mut u8, bit: u8) {
    *value &= !(1 << bit);
}

#[inline(always)]
pub fn toggle_bit(value: &mut u8, bit: u8) {
    *value ^= 1 << bit;
}

#[inline(always)]
pub fn get_bit(value: u8, bit: u8) -> u8 {
    (value >> bit) & 1
}

macro_rules! clear_bits {
     ($val:expr, $($bit:expr),+) => {{
         // Start with all bits set
         let mut mask: u8 = 0xFF;
         // Combine masks for each bit to clear
         $(
             mask &= !(1 << $bit);
         )+
         // Clear the bits by ANDing with the negated mask
         $val &= mask
     }};
 }

macro_rules! set_bits {
     ($val:expr, $($bit:expr),+) => {{
         // Start with all bits cleared
         let mut mask: u8 = 0x00;
         // Combine masks for each bit to set
         $(
             mask |= 1 << $bit;
         )+
         // Set the bits by ORing with the mask
         $val |= mask
     }};
 }

macro_rules! compare_register {
    ($mb:ident, $reg1:ident, $reg2:ident) => {
        let reg_a = $mb.cpu.$reg1;
        let reg_b = $mb.cpu.$reg2;

        let (result, overflow) = reg_a.overflowing_sub(reg_b);

        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);
        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        set_bit(&mut $mb.cpu.f, BIT_FLAGN);

        if (reg_a as u16 ^ reg_b as u16 ^ result as u16) & 0x10 != 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
    };
}

macro_rules! compare_register_with_value {
    ($mb:ident, $reg:ident, $value:ident) => {
        let reg_a = $mb.cpu.$reg;
        let reg_b = $value;

        let (result, overflow) = reg_a.overflowing_sub(reg_b);

        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);
        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        set_bit(&mut $mb.cpu.f, BIT_FLAGN);

        if (reg_a as u16 ^ reg_b as u16 ^ result as u16) & 0x10 != 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
    };
}

macro_rules! or_register {
    ($mb:ident, $reg1:ident, $reg2:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGC, BIT_FLAGH, BIT_FLAGZ);
        let result = $mb.cpu.$reg1 | $mb.cpu.$reg2;

        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg1 = result;
    };
}

macro_rules! or_register_with_value {
    ($mb:ident, $reg:ident, $value:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGC, BIT_FLAGH, BIT_FLAGZ);
        let result = $mb.cpu.$reg | $value;

        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg = result;
    };
}

macro_rules! xor_register {
    ($mb:ident, $reg1:ident, $reg2:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGC, BIT_FLAGH, BIT_FLAGZ);
        let result = $mb.cpu.$reg1 ^ $mb.cpu.$reg2;

        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg1 = result;
    };
}

macro_rules! xor_register_with_value {
    ($mb:ident, $reg:ident, $value:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGC, BIT_FLAGH, BIT_FLAGZ);
        let result = $mb.cpu.$reg ^ $value;

        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg = result;
    };
}

macro_rules! and_register {
    ($mb:ident, $reg1:ident, $reg2:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGC, BIT_FLAGZ);
        let result = $mb.cpu.$reg1 & $mb.cpu.$reg2;

        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg1 = result;
    };
}

macro_rules! and_register_with_value {
    ($mb:ident, $reg:ident, $value:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGC, BIT_FLAGZ);
        let result = $mb.cpu.$reg & $value;

        if result == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg = result;
    };
}

macro_rules! sub_carry_register {
    ($mb:ident, $reg1:ident, $reg2:ident) => {
        let a = $mb.cpu.$reg1;
        let b = $mb.cpu.$reg2;

        let carry = if is_bit_set($mb.cpu.f, BIT_FLAGC) {
            1
        } else {
            0
        };
        // let result = a.wrapping_sub(b).wrapping_sub(carry);

        let (result, overflow1) = a.overflowing_sub(b);
        let (result, overflow2) = result.overflowing_sub(carry);
        let overflow = overflow1 || overflow2;

        clear_bits!($mb.cpu.f, BIT_FLAGZ, BIT_FLAGH, BIT_FLAGC, BIT_FLAGN);

        if result & 0xFF == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if (a & 0x0f) < (b & 0x0f) + carry {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        set_bit(&mut $mb.cpu.f, BIT_FLAGN);
        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg1 = result as u8;
    };
}

macro_rules! sub_carry_register_from_value {
    ($mb:ident, $reg:ident, $value:ident) => {
        let a = $mb.cpu.$reg;
        let b = $value as u8;

        let carry = if is_bit_set($mb.cpu.f, BIT_FLAGC) {
            1
        } else {
            0
        };
        let (result, overflow1) = a.overflowing_sub(b);
        let (result, overflow2) = result.overflowing_sub(carry);
        let overflow = overflow1 || overflow2;

        clear_bits!($mb.cpu.f, BIT_FLAGZ, BIT_FLAGH, BIT_FLAGC, BIT_FLAGN);

        if result & 0xFF == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if (a & 0x0f) < (b & 0x0f) + carry {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        set_bit(&mut $mb.cpu.f, BIT_FLAGN);
        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg = result as u8;
    };
}

macro_rules! sub_register {
    ($mb:ident, $reg1:ident, $reg2:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);
        let a = $mb.cpu.$reg1;
        let b = $mb.cpu.$reg2;

        let (result, overflow) = a.overflowing_sub(b);

        if (result & 0xFF) == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if (a & 0x0F) < (b & 0x0F) {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        set_bit(&mut $mb.cpu.f, BIT_FLAGN);
        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg1 = result as u8;
    };
}

macro_rules! sub_register_from_value {
    ($mb:ident, $reg:ident, $value:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);
        let a = $mb.cpu.$reg as u16;
        let b = $value as u16;

        let (result, overflow) = a.overflowing_sub(b);

        if (result & 0xFF) == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if (a & 0x0F) < (b & 0x0F) {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        set_bit(&mut $mb.cpu.f, BIT_FLAGN);
        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg = result as u8;
    };
}

macro_rules! add_register {
    ($mb:ident, $reg1:ident, $reg2:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);
        let a = $mb.cpu.$reg1;
        let b = $mb.cpu.$reg2;

        let (result, overflow) = a.overflowing_add(b);

        if (result & 0xFF) == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if (a ^ b ^ result) & 0x10 != 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg1 = result as u8;
    };
}

macro_rules! add_carry_register {
    ($mb:ident, $reg1:ident, $reg2:ident) => {
        let a = $mb.cpu.$reg1;
        let b = $mb.cpu.$reg2;

        let carry = if is_bit_set($mb.cpu.f, BIT_FLAGC) {
            1
        } else {
            0
        };
        let (result, overflow1) = a.overflowing_add(b);
        let (result, overflow2) = result.overflowing_add(carry);
        let overflow = overflow1 || overflow2;

        clear_bits!($mb.cpu.f, BIT_FLAGZ, BIT_FLAGH, BIT_FLAGC, BIT_FLAGN);

        if result & 0xFF == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        if (a & 0x0f) + (b & 0x0f) + carry > 0x0f {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg1 = (result & 0xff) as u8;
    };
}

macro_rules! add_register_from_value {
    ($mb:ident, $reg:ident, $value:ident) => {
        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGH, BIT_FLAGC, BIT_FLAGZ);
        let a = $mb.cpu.$reg;
        let b = $value;

        let (result, overflow) = a.overflowing_add(b);

        if (result & 0xFF) == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if (a ^ b ^ result) & 0x10 != 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg = result as u8;
    };
}

macro_rules! add_carry_register_from_value {
    ($mb:ident, $reg:ident, $value:ident) => {
        let a = $mb.cpu.$reg;
        let b = $value;

        let carry = if is_bit_set($mb.cpu.f, BIT_FLAGC) {
            1
        } else {
            0
        };
        let (result, overflow1) = a.overflowing_add(b);
        let (result, overflow2) = result.overflowing_add(carry);
        let overflow = overflow1 || overflow2;
        clear_bits!($mb.cpu.f, BIT_FLAGZ, BIT_FLAGH, BIT_FLAGC, BIT_FLAGN);

        if result & 0xFF == 0 {
            set_bit(&mut $mb.cpu.f, BIT_FLAGZ);
        }

        if overflow {
            set_bit(&mut $mb.cpu.f, BIT_FLAGC);
        }

        if (a & 0x0f) + (b & 0x0f) + carry > 0x0f {
            set_bit(&mut $mb.cpu.f, BIT_FLAGH);
        }

        $mb.cpu.pc = $mb.cpu.pc.wrapping_add(1);
        $mb.cpu.$reg = (result & 0xff) as u8;
    };
}

macro_rules! increment_register {
    ($mb:ident, $reg:ident) => {
        let result = $mb.cpu.$reg.wrapping_add(1);

        clear_bits!($mb.cpu.f, BIT_FLAGN, BIT_FLAGZ, BIT_FLAGH);

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
        clear_bits!($mb.cpu.f, BIT_FLAGZ, BIT_FLAGH);

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
