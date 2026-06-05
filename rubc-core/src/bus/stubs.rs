//! Tickable peripheral stubs for the N4 bus skeleton.
//!
//! These are intentionally minimal: just enough state so the bus invariant
//! (OAM-DMA beat -> 4 T-cycle ticks -> latched access) can be built and proven
//! BEFORE the real PPU/APU land in later waves. Each stub counts its ticks so
//! tests can assert the ordering of the invariant.

/// Interrupt enable / flag registers + a pending queue.
///
/// IRQs requested during a tick are queued and only merged into `if_` at the
/// next instruction boundary, modelling "IRQs raised mid-M-cycle become visible
/// next boundary".
#[derive(Default)]
pub struct Interrupts {
    pub ie: u8,
    pub if_: u8,
    /// Bits requested during the current M-cycle, surfaced at the next boundary.
    pending: u8,
}

impl Interrupts {
    /// Request an interrupt (by bit 0..=4). Queued until the next boundary.
    pub fn request(&mut self, bit: u8) {
        self.pending |= 1 << bit;
    }

    /// Merge queued requests into `if_`. Called at the instruction boundary.
    pub fn settle_boundary(&mut self) {
        self.if_ |= self.pending | 0xE0; // top 3 bits read as 1
        self.pending = 0;
    }

    /// Currently visible pending+enabled interrupts (IE & IF, low 5 bits).
    pub fn pending_mask(&self) -> u8 {
        self.ie & self.if_ & 0x1F
    }

    pub fn clear_bit(&mut self, bit: u8) {
        self.if_ &= !(1 << bit);
    }
}

/// Stub APU: counts ticks. Real channels land in the APU wave.
#[derive(Default)]
pub struct ApuStub {
    pub t_ticks: u64,
}

impl ApuStub {
    pub fn tick_t(&mut self) {
        self.t_ticks += 1;
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
