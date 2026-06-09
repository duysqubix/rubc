//! Sub-dot timing scheduler — the foundation of the timestamped CPU/PPU phase
//! model (see `docs/adr/0001-sub-dot-cpu-ppu-event-scheduler.md`).
//!
//! # Why this exists
//!
//! The mealybug-tearoom mid-mode-3 tests write `SCY`/`BGP`/`LCDC`/`WX` while the
//! background fetcher is mid-fetch. Reproducing them faithfully requires
//! expressing the exact *sub-dot* phase at which a CPU register write becomes
//! visible to a concurrent PPU fetch — something the old strict-lockstep
//! `1 dot = 1 T` model cannot represent.
//!
//! # The model
//!
//! Time is measured in **CPU-T subphases**, not PPU dots:
//!
//! ```text
//!   1 T-cycle            = SUBPHASES_PER_T (4) subphases
//!   1 M-cycle            = 4 T-cycles      = 16 subphases
//!   1 PPU dot (normal)   = 1 T-cycle       = 4 subphases
//!   1 PPU dot (2x speed) = 1/2 T-cycle     = 2 subphases
//! ```
//!
//! Measuring in T-subphases (not dots) is what keeps CGB double-speed parity:
//! the PPU advances one dot every T at normal speed and every *second* T at
//! double speed, exactly mirroring the existing `t_phase` toggle.
//!
//! # Stage 1 (this file, behavior-preserving)
//!
//! This introduces the [`Time`] type and the phase constants only. It is wired
//! into the bus as a monotonic clock that advances in lockstep with the existing
//! `t_tick_count`, so it can be asserted equal in tests *before* any timing
//! behavior changes (stages 5+). Nothing here alters emulation.

/// Sub-divisions of one CPU T-cycle. Four lets us name the four canonical
/// write-drive positions the current model already uses (T0/T1/T2/T3) without
/// fractional arithmetic.
pub const SUBPHASES_PER_T: u64 = 4;

/// Subphases per M-cycle (4 T-cycles).
pub const SUBPHASES_PER_M: u64 = SUBPHASES_PER_T * 4;

/// A monotonic timestamp measured in CPU-T subphases since power-on.
///
/// `Time` is a thin newtype over `u64`; at ~4.19 MHz * 4 subphases it would take
/// roughly 35,000 years to overflow, so wrapping is not a concern.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Time(pub u64);

impl Time {
    /// The power-on instant.
    pub const ZERO: Time = Time(0);

    /// Construct from a whole T-cycle count (subphase 0 of that T).
    #[inline]
    pub const fn from_t(t: u64) -> Time {
        Time(t * SUBPHASES_PER_T)
    }

    /// Whole T-cycles elapsed (truncating any partial subphase).
    #[inline]
    pub const fn t(self) -> u64 {
        self.0 / SUBPHASES_PER_T
    }

    /// Whole M-cycles elapsed (truncating).
    #[inline]
    pub const fn m(self) -> u64 {
        self.0 / SUBPHASES_PER_M
    }

    /// The subphase index within the current T-cycle (`0..SUBPHASES_PER_T`).
    #[inline]
    pub const fn subphase_in_t(self) -> u64 {
        self.0 % SUBPHASES_PER_T
    }

    /// Advance by `subphases`.
    #[inline]
    pub fn advance(&mut self, subphases: u64) {
        self.0 += subphases;
    }

    /// Advance by one whole T-cycle.
    #[inline]
    pub fn advance_t(&mut self) {
        self.0 += SUBPHASES_PER_T;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_t_and_m_conversions() {
        assert_eq!(Time::ZERO.t(), 0);
        assert_eq!(Time::from_t(7).t(), 7);
        assert_eq!(Time::from_t(8).m(), 2); // 8 T = 2 M
        assert_eq!(Time::from_t(8).subphase_in_t(), 0);
    }

    #[test]
    fn advancing_one_t_is_four_subphases() {
        let mut now = Time::ZERO;
        now.advance_t();
        assert_eq!(now.0, SUBPHASES_PER_T);
        assert_eq!(now.t(), 1);
    }

    #[test]
    fn subphase_wraps_within_t() {
        let mut now = Time::ZERO;
        for expected in 0..SUBPHASES_PER_T {
            assert_eq!(now.subphase_in_t(), expected);
            now.advance(1);
        }
        // After SUBPHASES_PER_T subphases we are at the next whole T, subphase 0.
        assert_eq!(now.subphase_in_t(), 0);
        assert_eq!(now.t(), 1);
    }

    #[test]
    fn m_cycle_is_sixteen_subphases() {
        assert_eq!(SUBPHASES_PER_M, 16);
        assert_eq!(Time::from_t(4).0, SUBPHASES_PER_M);
    }
}
