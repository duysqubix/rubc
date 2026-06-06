//! Tickable peripheral stubs for the N4 bus skeleton.
//!
//! These are intentionally minimal: just enough state so the bus invariant
//! (OAM-DMA beat -> 4 T-cycle ticks -> latched access) can be built and proven
//! BEFORE the real PPU/APU land in later waves. Each stub counts its ticks so
//! tests can assert the ordering of the invariant.

/// Interrupt enable / flag registers.
///
/// IRQ sources latch IF immediately on the T-cycle that requests them. The CPU
/// still decides whether to dispatch only at an instruction boundary.
#[derive(Default)]
pub struct Interrupts {
    pub ie: u8,
    pub if_: u8,
}

impl Interrupts {
    /// Request an interrupt (by bit 0..=4). Latches IF immediately.
    pub fn request(&mut self, bit: u8) {
        self.if_ |= 1 << bit;
    }

    /// Normalize IF's unused high bits at an instruction boundary.
    pub fn settle_boundary(&mut self) {
        self.if_ |= 0xE0; // top 3 bits read as 1
    }

    /// Currently visible pending+enabled interrupts (IE & IF, low 5 bits).
    pub fn pending_mask(&self) -> u8 {
        self.ie & self.if_ & 0x1F
    }

    pub fn clear_bit(&mut self, bit: u8) {
        self.if_ &= !(1 << bit);
    }
}

/// CGB clock/speed state.
#[derive(Default)]
pub struct CgbState {
    /// True when running as a Game Boy Color. DMG has no speed switch, so KEY1
    /// writes are inert and STOP always halts. Set by `Machine::boot_cgb`.
    pub cgb_mode: bool,
    pub double_speed: bool,
    /// Toggles each T so PPU/APU advance every 2nd T in double-speed mode.
    pub t_phase: bool,
}
