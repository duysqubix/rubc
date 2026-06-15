//! SM83 register file.
//!
//! Eight 8-bit registers (A, F, B, C, D, E, H, L) plus the 16-bit SP and PC.
//! The four register pairs AF/BC/DE/HL are exposed as 16-bit views. The low
//! nibble of F is always zero (only Z/N/H/C exist), enforced on every write.

use super::alu::Flags;

/// The SM83 register file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Regs {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Regs {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- 16-bit pair views -------------------------------------------------

    pub fn af(&self) -> u16 {
        (self.a as u16) << 8 | self.f as u16
    }
    pub fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }
    pub fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }
    pub fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    /// Set AF. The low nibble of F is masked off (it cannot hold bits).
    pub fn set_af(&mut self, v: u16) {
        self.a = (v >> 8) as u8;
        self.f = (v as u8) & 0xF0;
    }
    pub fn set_bc(&mut self, v: u16) {
        self.b = (v >> 8) as u8;
        self.c = v as u8;
    }
    pub fn set_de(&mut self, v: u16) {
        self.d = (v >> 8) as u8;
        self.e = v as u8;
    }
    pub fn set_hl(&mut self, v: u16) {
        self.h = (v >> 8) as u8;
        self.l = v as u8;
    }

    // ---- flags -------------------------------------------------------------

    /// Read the condition flags out of F.
    pub fn flags(&self) -> Flags {
        Flags::from_byte(self.f)
    }

    /// Write the condition flags into F (low nibble forced to zero).
    pub fn set_flags(&mut self, flags: Flags) {
        self.f = flags.to_byte();
    }

    /// Set F directly, masking the low nibble.
    pub fn set_f(&mut self, f: u8) {
        self.f = f & 0xF0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_views_compose_halves() {
        let mut r = Regs::new();
        r.b = 0x12;
        r.c = 0x34;
        assert_eq!(r.bc(), 0x1234);
        r.set_de(0xBEEF);
        assert_eq!(r.d, 0xBE);
        assert_eq!(r.e, 0xEF);
        assert_eq!(r.de(), 0xBEEF);
    }

    #[test]
    fn af_masks_low_nibble_of_f() {
        let mut r = Regs::new();
        r.set_af(0x12FF); // F low nibble should be dropped.
        assert_eq!(r.a, 0x12);
        assert_eq!(r.f, 0xF0, "low nibble of F is always zero");
        assert_eq!(r.af(), 0x12F0);
    }

    #[test]
    fn set_f_masks_low_nibble() {
        let mut r = Regs::new();
        r.set_f(0xFF);
        assert_eq!(r.f, 0xF0);
    }

    #[test]
    fn flags_roundtrip_through_f() {
        let mut r = Regs::new();
        r.set_flags(Flags::new(true, false, true, false));
        assert_eq!(r.f, 0xA0); // Z + H
        let fl = r.flags();
        assert!(fl.z && !fl.n && fl.h && !fl.c);
    }
}
