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
//!   1 PPU dot (2x speed) = 2 T-cycles      = 8 subphases
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

/// Subphase period of one PPU dot at normal CPU speed: one dot per CPU T-cycle.
///
/// Stage 1 names this period only; `tick_cpu_t` still uses today's behavior.
pub const PPU_DOT_SUBPHASES_NORMAL: u64 = SUBPHASES_PER_T;

/// Subphase period of one PPU dot at CGB double-speed: one dot every second CPU
/// T-cycle, so a dot spans two T-cycles.
///
/// Stage 1 names this period only; `tick_cpu_t` still uses today's behavior.
pub const PPU_DOT_SUBPHASES_DOUBLE: u64 = SUBPHASES_PER_T * 2;

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

/// `SUBPHASES_PER_T` as a `u8`, for the small relative offsets a single
/// M-cycle access plan deals in (0..=16).
pub const SUBPHASES_PER_T_U8: u8 = SUBPHASES_PER_T as u8;

/// T-cycles in one CPU M-cycle.
pub const CPU_M_CYCLE_TICKS: u8 = 4;

/// The end-of-M-cycle offset, in subphases (4 T * 4 subphases).
pub const CPU_ACCESS_END_OFFSET: u8 = SUBPHASES_PER_T_U8 * CPU_M_CYCLE_TICKS;

/// The explicit sub-dot timing of one CPU bus M-cycle (ADR 0001 stage 2).
///
/// Offsets are **relative to the start of the M-cycle**, in CPU-T subphases
/// (`0..=CPU_ACCESS_END_OFFSET`). This makes the timing that was previously
/// implicit in `write_drive_ticks` / `step_t` into inspectable data, so stage 5
/// can shift a phase by editing a plan rather than scattered tick counting.
///
/// Stage 2 is behavior-preserving: the offsets encode exactly today's timing
/// (writes commit after the Nth `tick_cpu_t`; reads sample at end-of-M).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CpuAccessPlan {
    /// When the access begins (always 0 today: the M-cycle start).
    pub start: u8,
    /// Subphase at which a write commits, or `None` for non-writes.
    pub write_visible_at: Option<u8>,
    /// Subphase at which a read samples its byte, or `None` for non-reads.
    pub read_sample_at: Option<u8>,
    /// When the access ends (end-of-M).
    pub end: u8,
}

impl CpuAccessPlan {
    /// An internal-work M-cycle: no memory access, just four ticks.
    pub const fn idle() -> Self {
        Self {
            start: 0,
            write_visible_at: None,
            read_sample_at: None,
            end: CPU_ACCESS_END_OFFSET,
        }
    }

    /// A read/fetch M-cycle: the byte samples at end-of-M (after 4 ticks),
    /// matching today's `read_latched` placement.
    pub const fn read_like() -> Self {
        Self {
            start: 0,
            write_visible_at: None,
            read_sample_at: Some(CPU_ACCESS_END_OFFSET),
            end: CPU_ACCESS_END_OFFSET,
        }
    }

    /// A write M-cycle whose commit lands after `write_drive_ticks` ticks,
    /// encoding today's `write_drive_ticks(addr)` exactly (T0 for BGP, T2 for
    /// SCY/STAT/etc., T3 for other PPU-visible, T4/end for the rest).
    pub const fn write(write_drive_ticks: u8) -> Self {
        debug_assert!(write_drive_ticks <= CPU_M_CYCLE_TICKS);
        Self {
            start: 0,
            write_visible_at: Some(write_drive_ticks * SUBPHASES_PER_T_U8),
            read_sample_at: None,
            end: CPU_ACCESS_END_OFFSET,
        }
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

    #[test]
    fn ppu_dot_periods_match_cpu_speed_modes() {
        assert_eq!(PPU_DOT_SUBPHASES_NORMAL, 4);
        assert_eq!(PPU_DOT_SUBPHASES_DOUBLE, 8);
    }

    #[test]
    fn access_plan_write_offsets_encode_drive_ticks() {
        // The four canonical write-drive positions today: BGP at T0, the
        // SCY/STAT class at T2, other PPU-visible at T3, the rest at end-of-M.
        assert_eq!(CpuAccessPlan::write(0).write_visible_at, Some(0));
        assert_eq!(CpuAccessPlan::write(2).write_visible_at, Some(8));
        assert_eq!(CpuAccessPlan::write(3).write_visible_at, Some(12));
        assert_eq!(
            CpuAccessPlan::write(4).write_visible_at,
            Some(CPU_ACCESS_END_OFFSET)
        );
    }

    #[test]
    fn access_plan_read_and_idle_shapes() {
        let r = CpuAccessPlan::read_like();
        assert_eq!(r.read_sample_at, Some(CPU_ACCESS_END_OFFSET));
        assert_eq!(r.write_visible_at, None);

        let i = CpuAccessPlan::idle();
        assert_eq!(i.read_sample_at, None);
        assert_eq!(i.write_visible_at, None);
        assert_eq!(i.end, CPU_ACCESS_END_OFFSET);
    }

    #[test]
    fn access_plan_offsets_round_trip_to_t() {
        for drive in 0..=CPU_M_CYCLE_TICKS {
            let off = CpuAccessPlan::write(drive).write_visible_at.unwrap();
            assert_eq!(off / SUBPHASES_PER_T_U8, drive);
        }
    }
}
