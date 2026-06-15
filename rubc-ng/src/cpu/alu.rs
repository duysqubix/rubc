//! Pure SM83 ALU helpers.
//!
//! Every function here is SIDE-EFFECT-FREE: it takes operands (and the carry-in
//! where relevant) and returns the `(result, Flags)` pair. No PC increment, no
//! cycle counting, no register mutation — that all lives in the M-cycle state
//! machine. This makes the flag logic exhaustively unit-testable against the
//! SM83 truth tables, independent of the CPU plumbing.
//!
//! Flag layout in the `F` register: Z = bit 7, N = bit 6, H = bit 5, C = bit 4;
//! the low nibble is always zero.

/// The four SM83 condition flags, as a small value type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

impl Flags {
    pub const fn new(z: bool, n: bool, h: bool, c: bool) -> Self {
        Self { z, n, h, c }
    }

    /// Pack into the `F` register byte (low nibble always zero).
    pub const fn to_byte(self) -> u8 {
        (self.z as u8) << 7 | (self.n as u8) << 6 | (self.h as u8) << 5 | (self.c as u8) << 4
    }

    /// Unpack from the `F` register byte.
    pub const fn from_byte(f: u8) -> Self {
        Self {
            z: f & 0x80 != 0,
            n: f & 0x40 != 0,
            h: f & 0x20 != 0,
            c: f & 0x10 != 0,
        }
    }
}

// ---- 8-bit arithmetic ------------------------------------------------------

/// `a + b + carry_in`. Half-carry from bit 3, carry from bit 7.
pub fn add8(a: u8, b: u8, carry_in: bool) -> (u8, Flags) {
    let cin = carry_in as u16;
    let sum = a as u16 + b as u16 + cin;
    let result = sum as u8;
    let h = ((a & 0x0F) + (b & 0x0F) + cin as u8) > 0x0F;
    (result, Flags::new(result == 0, false, h, sum > 0xFF))
}

/// `a - b - carry_in`. Half-borrow from bit 4, borrow from bit 8. N set.
pub fn sub8(a: u8, b: u8, carry_in: bool) -> (u8, Flags) {
    let cin = carry_in as u16;
    let diff = (a as u16).wrapping_sub(b as u16).wrapping_sub(cin);
    let result = diff as u8;
    let h = (a & 0x0F) < ((b & 0x0F) + cin as u8);
    let c = (a as u16) < (b as u16 + cin);
    (result, Flags::new(result == 0, true, h, c))
}

/// `a & b`. H always set; N, C clear.
pub fn and8(a: u8, b: u8) -> (u8, Flags) {
    let r = a & b;
    (r, Flags::new(r == 0, false, true, false))
}

/// `a | b`. All of N, H, C clear.
pub fn or8(a: u8, b: u8) -> (u8, Flags) {
    let r = a | b;
    (r, Flags::new(r == 0, false, false, false))
}

/// `a ^ b`. All of N, H, C clear.
pub fn xor8(a: u8, b: u8) -> (u8, Flags) {
    let r = a ^ b;
    (r, Flags::new(r == 0, false, false, false))
}

/// `a - b` discarding the result (CP). Same flags as [`sub8`] with carry 0.
pub fn cp8(a: u8, b: u8) -> Flags {
    sub8(a, b, false).1
}

/// `value + 1`. C is PRESERVED by the caller (INC does not touch C).
pub fn inc8(value: u8) -> (u8, Flags) {
    let result = value.wrapping_add(1);
    let h = (value & 0x0F) == 0x0F;
    // C is left to the caller; we report it false here and the caller keeps old C.
    (result, Flags::new(result == 0, false, h, false))
}

/// `value - 1`. C is PRESERVED by the caller (DEC does not touch C).
pub fn dec8(value: u8) -> (u8, Flags) {
    let result = value.wrapping_sub(1);
    let h = (value & 0x0F) == 0x00;
    (result, Flags::new(result == 0, true, h, false))
}

// ---- 16-bit arithmetic -----------------------------------------------------

/// `hl + rr` (ADD HL,rr). Z is PRESERVED by the caller; N clear; H from bit 11,
/// C from bit 15.
pub fn add16(hl: u16, rr: u16) -> (u16, Flags) {
    let result = hl.wrapping_add(rr);
    let h = (hl & 0x0FFF) + (rr & 0x0FFF) > 0x0FFF;
    let c = (hl as u32) + (rr as u32) > 0xFFFF;
    // Z preserved by caller; reported false here.
    (result, Flags::new(false, false, h, c))
}

/// `sp + e8` (ADD SP,e8 / LD HL,SP+e8). Z=0, N=0; H/C computed from the LOW byte
/// addition (8-bit boundaries), per SM83.
pub fn add_sp_e8(sp: u16, e8: i8) -> (u16, Flags) {
    let off = e8 as u16; // sign-extended
    let result = sp.wrapping_add(off);
    let h = (sp & 0x0F) + (off & 0x0F) > 0x0F;
    let c = (sp & 0xFF) + (off & 0xFF) > 0xFF;
    (result, Flags::new(false, false, h, c))
}

// ---- rotates / shifts (CB + the A-register variants) -----------------------

/// RLC: rotate left, bit 7 -> C and -> bit 0.
pub fn rlc(value: u8) -> (u8, Flags) {
    let c = value & 0x80 != 0;
    let result = value.rotate_left(1);
    (result, Flags::new(result == 0, false, false, c))
}

/// RRC: rotate right, bit 0 -> C and -> bit 7.
pub fn rrc(value: u8) -> (u8, Flags) {
    let c = value & 0x01 != 0;
    let result = value.rotate_right(1);
    (result, Flags::new(result == 0, false, false, c))
}

/// RL: rotate left through carry.
pub fn rl(value: u8, carry_in: bool) -> (u8, Flags) {
    let c = value & 0x80 != 0;
    let result = (value << 1) | carry_in as u8;
    (result, Flags::new(result == 0, false, false, c))
}

/// RR: rotate right through carry.
pub fn rr(value: u8, carry_in: bool) -> (u8, Flags) {
    let c = value & 0x01 != 0;
    let result = (value >> 1) | ((carry_in as u8) << 7);
    (result, Flags::new(result == 0, false, false, c))
}

/// SLA: arithmetic shift left (bit 0 = 0).
pub fn sla(value: u8) -> (u8, Flags) {
    let c = value & 0x80 != 0;
    let result = value << 1;
    (result, Flags::new(result == 0, false, false, c))
}

/// SRA: arithmetic shift right (bit 7 preserved).
pub fn sra(value: u8) -> (u8, Flags) {
    let c = value & 0x01 != 0;
    let result = (value >> 1) | (value & 0x80);
    (result, Flags::new(result == 0, false, false, c))
}

/// SRL: logical shift right (bit 7 = 0).
pub fn srl(value: u8) -> (u8, Flags) {
    let c = value & 0x01 != 0;
    let result = value >> 1;
    (result, Flags::new(result == 0, false, false, c))
}

/// SWAP: swap nibbles. All of N, H, C clear.
pub fn swap(value: u8) -> (u8, Flags) {
    let result = value.rotate_left(4);
    (result, Flags::new(result == 0, false, false, false))
}

/// BIT n,value: test bit. Z = !bit; N=0, H=1; C PRESERVED by caller.
pub fn bit(value: u8, n: u8) -> Flags {
    let z = value & (1 << n) == 0;
    Flags::new(z, false, true, false)
}

/// RES n,value: clear bit (no flags).
pub fn res(value: u8, n: u8) -> u8 {
    value & !(1 << n)
}

/// SET n,value: set bit (no flags).
pub fn set(value: u8, n: u8) -> u8 {
    value | (1 << n)
}

// ---- misc accumulator ops --------------------------------------------------

/// DAA: decimal-adjust A after add/sub, using the current N/H/C flags.
pub fn daa(a: u8, n: bool, h: bool, c: bool) -> (u8, Flags) {
    let mut result = a;
    let mut carry = c;
    if !n {
        if c || a > 0x99 {
            result = result.wrapping_add(0x60);
            carry = true;
        }
        if h || (a & 0x0F) > 0x09 {
            result = result.wrapping_add(0x06);
        }
    } else {
        if c {
            result = result.wrapping_sub(0x60);
        }
        if h {
            result = result.wrapping_sub(0x06);
        }
    }
    (result, Flags::new(result == 0, n, false, carry))
}

/// CPL: complement A. N=1, H=1; Z, C PRESERVED by caller.
pub fn cpl(a: u8) -> u8 {
    !a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add8_half_and_full_carry() {
        // 0x3A + 0xC6 = 0x100 -> 0x00, Z+H+C set, N clear.
        let (r, f) = add8(0x3A, 0xC6, false);
        assert_eq!(r, 0x00);
        assert_eq!(f, Flags::new(true, false, true, true));
        // 0x0F + 0x01 = 0x10 -> H only.
        let (r, f) = add8(0x0F, 0x01, false);
        assert_eq!(r, 0x10);
        assert_eq!(f, Flags::new(false, false, true, false));
        // carry-in: 0xFF + 0x00 + 1 = 0x100.
        let (r, f) = add8(0xFF, 0x00, true);
        assert_eq!(r, 0x00);
        assert!(f.z && f.h && f.c && !f.n);
    }

    #[test]
    fn sub8_borrow_flags() {
        // 0x3E - 0x3E = 0 -> Z+N.
        let (r, f) = sub8(0x3E, 0x3E, false);
        assert_eq!(r, 0x00);
        assert_eq!(f, Flags::new(true, true, false, false));
        // 0x3E - 0x0F: half-borrow.
        let (r, f) = sub8(0x3E, 0x0F, false);
        assert_eq!(r, 0x2F);
        assert!(f.n && f.h && !f.c);
        // 0x3E - 0x40: full borrow.
        let (_r, f) = sub8(0x3E, 0x40, false);
        assert!(f.n && f.c);
    }

    #[test]
    fn logic_ops_flags() {
        assert_eq!(and8(0x5A, 0x3F).1, Flags::new(false, false, true, false));
        assert_eq!(and8(0x00, 0xFF).1, Flags::new(true, false, true, false));
        assert_eq!(or8(0x00, 0x00).1, Flags::new(true, false, false, false));
        assert_eq!(xor8(0xFF, 0xFF).1, Flags::new(true, false, false, false));
    }

    #[test]
    fn inc_dec_half_carry() {
        assert_eq!(inc8(0x0F), (0x10, Flags::new(false, false, true, false)));
        assert_eq!(inc8(0xFF), (0x00, Flags::new(true, false, true, false)));
        assert_eq!(dec8(0x10), (0x0F, Flags::new(false, true, true, false)));
        assert_eq!(dec8(0x01), (0x00, Flags::new(true, true, false, false)));
    }

    #[test]
    fn add16_bit11_carry() {
        // 0x8A23 + 0x0605 = 0x9028, H from bit 11.
        let (r, f) = add16(0x8A23, 0x0605);
        assert_eq!(r, 0x9028);
        assert!(f.h && !f.c && !f.n);
        // overflow past 0xFFFF -> C.
        let (_r, f) = add16(0xFFFF, 0x0001);
        assert!(f.c && f.h);
    }

    #[test]
    fn add_sp_e8_low_byte_flags() {
        // SP=0xFFFF, +1 -> H and C from low-byte math.
        let (r, f) = add_sp_e8(0xFFFF, 1);
        assert_eq!(r, 0x0000);
        assert!(f.h && f.c && !f.z && !f.n);
        // negative offset.
        let (r, _f) = add_sp_e8(0x0005, -2);
        assert_eq!(r, 0x0003);
    }

    #[test]
    fn rotates_through_and_circular() {
        assert_eq!(rlc(0x85), (0x0B, Flags::new(false, false, false, true)));
        assert_eq!(rrc(0x01), (0x80, Flags::new(false, false, false, true)));
        assert_eq!(
            rl(0x80, false),
            (0x00, Flags::new(true, false, false, true))
        );
        assert_eq!(
            rr(0x01, true),
            (0x80, Flags::new(false, false, false, true))
        );
        assert_eq!(sla(0x80), (0x00, Flags::new(true, false, false, true)));
        assert_eq!(sra(0x81), (0xC0, Flags::new(false, false, false, true)));
        assert_eq!(srl(0x01), (0x00, Flags::new(true, false, false, true)));
        assert_eq!(swap(0xAB), (0xBA, Flags::new(false, false, false, false)));
    }

    #[test]
    fn bit_res_set() {
        assert_eq!(bit(0x80, 7), Flags::new(false, false, true, false));
        assert_eq!(bit(0x00, 7), Flags::new(true, false, true, false));
        assert_eq!(res(0xFF, 3), 0xF7);
        assert_eq!(set(0x00, 3), 0x08);
    }

    #[test]
    fn daa_after_add_and_sub() {
        // 0x09 + 0x08 = 0x11 (raw), DAA after add with H -> 0x17.
        let (r, _f) = daa(0x11, false, true, false);
        assert_eq!(r, 0x17);
        // 0x0A -> 0x10 (BCD adjust after add).
        let (r, _f) = daa(0x0A, false, false, false);
        assert_eq!(r, 0x10);
    }

    #[test]
    fn flags_byte_roundtrip() {
        for f in 0u8..=0xFF {
            let masked = f & 0xF0;
            assert_eq!(Flags::from_byte(f).to_byte(), masked);
        }
    }
}
