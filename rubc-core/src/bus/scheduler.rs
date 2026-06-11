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

/// CPU write scheduled to become bus-visible at `at`; `seq` breaks ties FIFO.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CpuWriteEvent {
    pub at: Time,
    pub seq: u64,
    pub addr: u16,
    pub value: u8,
}

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

/// The Game Boy model class that selects which write-conflict map applies.
///
/// SameBoy keys its conflict maps on the full model; for rubc's mid-mode-3
/// timing port only the DMG vs CGB (single/double-speed) distinction matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictModel {
    /// Original DMG (and MGB/SGB share the relevant entries we port).
    Dmg,
    /// CGB at normal speed.
    Cgb,
    /// CGB in double-speed mode.
    CgbDouble,
}

/// Per-register CPU-write-vs-PPU-fetch conflict timing class, transcribed 1:1
/// from SameBoy `Core/sm83_cpu.c:31-83` (`dmg_conflict_map`/`cgb_conflict_map`/
/// `cgb_double_conflict_map`) and the `cycle_write` switch (`:113-319`).
///
/// The variant names mirror SameBoy's `GB_CONFLICT_*` constants. This is pure
/// data: [`conflict_type`] maps an IO address to its class; the producer-side
/// flush timing that consumes these is wired in later stages (ADR 0001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictType {
    /// `GB_CONFLICT_READ_OLD`: normal write, PPU sees it at M-cycle end.
    ReadOld,
    /// `GB_CONFLICT_READ_NEW`: commit 1 T-cycle early, borrowing 1 T into the
    /// next M-cycle (SCY on DMG/CGB).
    ReadNew,
    /// `GB_CONFLICT_WRITE_CPU`: commit 1 T-cycle late (IF, LYC/WX on CGB).
    WriteCpu,
    /// `GB_CONFLICT_PALETTE_DMG`: two-phase `old|new` then `new` transient
    /// (BGP/OBP0/OBP1 on DMG).
    PaletteDmg,
    /// `GB_CONFLICT_PALETTE_CGB`: CGB palette two-phase variant.
    PaletteCgb,
    /// `GB_CONFLICT_STAT_DMG`: STAT reads as `0xFF` for one T-cycle (DMG).
    StatDmg,
    /// `GB_CONFLICT_STAT_CGB`: CGB STAT two-phase variant.
    StatCgb,
    /// `GB_CONFLICT_STAT_CGB_DOUBLE`: CGB double-speed STAT variant.
    StatCgbDouble,
    /// `GB_CONFLICT_DMG_LCDC`: LCDC LCD-interacting two-phase write (DMG).
    LcdcDmg,
    /// `GB_CONFLICT_LCDC_CGB`: CGB LCDC (TILE_SEL glitch path).
    LcdcCgb,
    /// `GB_CONFLICT_LCDC_CGB_DOUBLE`: CGB double-speed LCDC variant.
    LcdcCgbDouble,
    /// `GB_CONFLICT_WX_DMG`: WX `wx_just_changed` one-T transient (DMG).
    WxDmg,
    /// `GB_CONFLICT_SCX_DMG_AND_CGB_DOUBLE`: SCX fine-scroll latency.
    ScxDmgAndCgbDouble,
    /// `GB_CONFLICT_NR10_CGB_DOUBLE`: APU sweep stepping (CGB double-speed).
    Nr10CgbDouble,
}

/// The write-conflict class for an IO write, or `None` for addresses with no
/// PPU/timing conflict (which flush as a plain `ReadOld` write).
///
/// Transcribed 1:1 from SameBoy's per-model conflict maps. Only the `0xFF00..`
/// IO page carries conflicts; SameBoy gates on `(addr & 0xFF80) == 0xFF00` and
/// indexes `map[addr & 0x7F]` (`sm83_cpu.c:117-128`).
pub fn conflict_type(model: ConflictModel, addr: u16) -> Option<ConflictType> {
    if addr & 0xFF80 != 0xFF00 {
        return None;
    }
    let reg = addr & 0x7F;
    match model {
        ConflictModel::Dmg => dmg_conflict_type(reg),
        ConflictModel::Cgb => cgb_conflict_type(reg),
        ConflictModel::CgbDouble => cgb_double_conflict_type(reg),
    }
}

/// DMG map, from `dmg_conflict_map` (`sm83_cpu.c:56-68`).
fn dmg_conflict_type(reg: u16) -> Option<ConflictType> {
    Some(match reg {
        0x0F => ConflictType::WriteCpu,           // IF
        0x45 => ConflictType::ReadOld,            // LYC
        0x40 => ConflictType::LcdcDmg,            // LCDC
        0x42 => ConflictType::ReadNew,            // SCY
        0x41 => ConflictType::StatDmg,            // STAT
        0x47 => ConflictType::PaletteDmg,         // BGP
        0x48 => ConflictType::PaletteDmg,         // OBP0
        0x49 => ConflictType::PaletteDmg,         // OBP1
        0x4A => ConflictType::ReadOld,            // WY
        0x4B => ConflictType::WxDmg,              // WX
        0x43 => ConflictType::ScxDmgAndCgbDouble, // SCX
        _ => return None,
    })
}

/// CGB map, from `cgb_conflict_map` (`sm83_cpu.c:31-42`).
fn cgb_conflict_type(reg: u16) -> Option<ConflictType> {
    Some(match reg {
        0x40 => ConflictType::LcdcCgb,    // LCDC
        0x0F => ConflictType::WriteCpu,   // IF
        0x45 => ConflictType::WriteCpu,   // LYC
        0x4A => ConflictType::ReadOld,    // WY
        0x41 => ConflictType::StatCgb,    // STAT
        0x47 => ConflictType::PaletteCgb, // BGP
        0x48 => ConflictType::PaletteCgb, // OBP0
        0x49 => ConflictType::PaletteCgb, // OBP1
        0x43 => ConflictType::ReadOld,    // SCX
        0x4B => ConflictType::WriteCpu,   // WX
        _ => return None,
    })
}

/// CGB double-speed map, from `cgb_double_conflict_map` (`sm83_cpu.c:44-53`).
fn cgb_double_conflict_type(reg: u16) -> Option<ConflictType> {
    Some(match reg {
        0x40 => ConflictType::LcdcCgbDouble,      // LCDC
        0x0F => ConflictType::WriteCpu,           // IF
        0x45 => ConflictType::ReadOld,            // LYC
        0x4A => ConflictType::ReadOld,            // WY
        0x41 => ConflictType::StatCgbDouble,      // STAT
        0x10 => ConflictType::Nr10CgbDouble,      // NR10
        0x43 => ConflictType::ScxDmgAndCgbDouble, // SCX
        0x4B => ConflictType::ReadOld,            // WX
        _ => return None,
    })
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

    #[test]
    fn conflict_map_dmg_matches_sameboy_table() {
        use ConflictType::*;
        let dmg = ConflictModel::Dmg;
        // 1:1 with SameBoy dmg_conflict_map (sm83_cpu.c:56-68).
        assert_eq!(conflict_type(dmg, 0xFF42), Some(ReadNew)); // SCY
        assert_eq!(conflict_type(dmg, 0xFF47), Some(PaletteDmg)); // BGP
        assert_eq!(conflict_type(dmg, 0xFF48), Some(PaletteDmg)); // OBP0
        assert_eq!(conflict_type(dmg, 0xFF49), Some(PaletteDmg)); // OBP1
        assert_eq!(conflict_type(dmg, 0xFF40), Some(LcdcDmg)); // LCDC
        assert_eq!(conflict_type(dmg, 0xFF41), Some(StatDmg)); // STAT
        assert_eq!(conflict_type(dmg, 0xFF4A), Some(ReadOld)); // WY
        assert_eq!(conflict_type(dmg, 0xFF45), Some(ReadOld)); // LYC
        assert_eq!(conflict_type(dmg, 0xFF4B), Some(WxDmg)); // WX
        assert_eq!(conflict_type(dmg, 0xFF43), Some(ScxDmgAndCgbDouble)); // SCX
        assert_eq!(conflict_type(dmg, 0xFF0F), Some(WriteCpu)); // IF
                                                                // Non-conflict IO + non-IO addresses flush as plain ReadOld (None).
        assert_eq!(conflict_type(dmg, 0xFF46), None); // DMA
        assert_eq!(conflict_type(dmg, 0xFF00), None); // P1/JOYP
        assert_eq!(conflict_type(dmg, 0xC000), None); // WRAM (not IO page)
        assert_eq!(conflict_type(dmg, 0x8000), None); // VRAM
    }

    #[test]
    fn conflict_map_cgb_differs_from_dmg() {
        use ConflictType::*;
        let cgb = ConflictModel::Cgb;
        // 1:1 with SameBoy cgb_conflict_map (sm83_cpu.c:31-42).
        assert_eq!(conflict_type(cgb, 0xFF40), Some(LcdcCgb)); // LCDC
        assert_eq!(conflict_type(cgb, 0xFF41), Some(StatCgb)); // STAT
        assert_eq!(conflict_type(cgb, 0xFF47), Some(PaletteCgb)); // BGP
        assert_eq!(conflict_type(cgb, 0xFF45), Some(WriteCpu)); // LYC (differs from DMG ReadOld)
        assert_eq!(conflict_type(cgb, 0xFF4B), Some(WriteCpu)); // WX (differs from DMG WxDmg)
        assert_eq!(conflict_type(cgb, 0xFF43), Some(ReadOld)); // SCX (differs from DMG ScxDmg)
                                                               // SCY is absent from the CGB map (flushes ReadOld).
        assert_eq!(conflict_type(cgb, 0xFF42), None);
    }

    #[test]
    fn conflict_map_cgb_double_matches_sameboy_table() {
        use ConflictType::*;
        let dbl = ConflictModel::CgbDouble;
        // 1:1 with SameBoy cgb_double_conflict_map (sm83_cpu.c:44-53).
        assert_eq!(conflict_type(dbl, 0xFF40), Some(LcdcCgbDouble)); // LCDC
        assert_eq!(conflict_type(dbl, 0xFF41), Some(StatCgbDouble)); // STAT
        assert_eq!(conflict_type(dbl, 0xFF10), Some(Nr10CgbDouble)); // NR10
        assert_eq!(conflict_type(dbl, 0xFF43), Some(ScxDmgAndCgbDouble)); // SCX
        assert_eq!(conflict_type(dbl, 0xFF45), Some(ReadOld)); // LYC
        assert_eq!(conflict_type(dbl, 0xFF4B), Some(ReadOld)); // WX
        assert_eq!(conflict_type(dbl, 0xFF0F), Some(WriteCpu)); // IF
    }
}
