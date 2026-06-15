use crate::model::GbModel;
use crate::time::Time;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Observable {
    BootRomExit,
    CpuReadSample,
    CpuWriteDrive,
    CpuIdle,
    CpuIntrPoll,
    PpuLy,
    PpuModeEdge,
    PpuStat,
    PpuStatSources,
    PpuIrqEdge,
    PpuLcdOn,
    PpuLyc,
    PpuFetchSample,
    PpuMemoryLock,
    OutputPixelLatch,
    BusConflict,
    DmaBeat,
    TimerEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Anchor {
    PowerOn,
    CpuMStart,
    CpuTStart,
    PpuLineStart,
    PpuMode3Start,
    OutputColumn,
    BusWriteStart,
    DmaStart,
    TimerDivEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PhaseRule {
    AtAnchor,
    OffsetSubphases(i64),
    EveryCpuT { divisor: u8 },
    BeforeAnchor { subphases: u64 },
    AfterAnchor { subphases: u64 },
}

impl PhaseRule {
    pub fn resolve(self, anchor: Time) -> Time {
        match self {
            PhaseRule::AtAnchor | PhaseRule::EveryCpuT { .. } => anchor,
            PhaseRule::OffsetSubphases(offset) if offset >= 0 => {
                Time::from_subphases(anchor.subphases() + offset as u64)
            }
            PhaseRule::OffsetSubphases(offset) => {
                Time::from_subphases(anchor.subphases().saturating_sub(offset.unsigned_abs()))
            }
            PhaseRule::BeforeAnchor { subphases } => {
                Time::from_subphases(anchor.subphases().saturating_sub(subphases))
            }
            PhaseRule::AfterAnchor { subphases } => {
                Time::from_subphases(anchor.subphases() + subphases)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimingDomain {
    Boot,
    Cpu,
    PpuPublic,
    PpuInternal,
    Output,
    BusConflicts,
    Dma,
    Timer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimingEntry {
    pub name: &'static str,
    pub observable: Observable,
    pub anchor: Anchor,
    pub offset: Time,
    pub phase: PhaseRule,
}

impl TimingEntry {
    pub const fn new(
        name: &'static str,
        observable: Observable,
        anchor: Anchor,
        offset_subphases: u64,
        phase: PhaseRule,
    ) -> Self {
        Self {
            name,
            observable,
            anchor,
            offset: Time::from_subphases(offset_subphases),
            phase,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimingProfile {
    pub name: &'static str,
    pub entries: Vec<TimingEntry>,
}

impl TimingProfile {
    pub fn lookup(&self, name: &str) -> Option<&TimingEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimingTable {
    pub model: GbModel,
    pub boot: TimingProfile,
    pub cpu: TimingProfile,
    pub ppu_public: TimingProfile,
    pub ppu_internal: TimingProfile,
    pub output: TimingProfile,
    pub bus_conflicts: TimingProfile,
    pub dma: TimingProfile,
    pub timer: TimingProfile,
}

impl TimingTable {
    pub fn for_model(model: GbModel) -> Self {
        let ppu_dot_phase = PhaseRule::EveryCpuT { divisor: 1 };

        Self {
            model,
            boot: TimingProfile {
                name: "boot",
                entries: vec![TimingEntry::new(
                    "boot_rom_exit",
                    Observable::BootRomExit,
                    Anchor::PowerOn,
                    0,
                    PhaseRule::AtAnchor,
                )],
            },
            cpu: TimingProfile {
                name: "cpu",
                entries: vec![
                    TimingEntry::new(
                        "cpu_read_sample_end_m",
                        Observable::CpuReadSample,
                        Anchor::CpuMStart,
                        16,
                        PhaseRule::AfterAnchor { subphases: 16 },
                    ),
                    TimingEntry::new(
                        "cpu_write_drive_t2",
                        Observable::CpuWriteDrive,
                        Anchor::CpuMStart,
                        8,
                        PhaseRule::AfterAnchor { subphases: 8 },
                    ),
                    TimingEntry::new(
                        "cpu_intr_poll_boundary",
                        Observable::CpuIntrPoll,
                        Anchor::CpuMStart,
                        0,
                        PhaseRule::AtAnchor,
                    ),
                ],
            },
            ppu_public: TimingProfile {
                name: "ppu_public",
                entries: vec![
                    // DMG-B v2 public goldens: consecutive LY0/LY1 mode2-prepare rows in
                    // v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv:23 and :32 are
                    // 912 SameBoy 8MHz ticks apart, i.e. one 456-dot DMG scanline.
                    TimingEntry::new(
                        "dmg_b_line_ticks",
                        Observable::PpuLy,
                        Anchor::PpuLineStart,
                        912,
                        PhaseRule::AfterAnchor { subphases: 912 },
                    ),
                    // DMG-B v2 public goldens: LY ranges 0..=153 before returning to LY0;
                    // see v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv:1340-1346.
                    TimingEntry::new(
                        "dmg_b_lines_per_frame",
                        Observable::PpuLy,
                        Anchor::PpuLineStart,
                        154,
                        PhaseRule::AfterAnchor { subphases: 154 },
                    ),
                    TimingEntry::new(
                        "ppu_dot",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        0,
                        ppu_dot_phase,
                    ),
                    // DMG-B v2 public goldens: mode2 IRQ prepare/stat sample at line_tick=2;
                    // see v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv:23.
                    TimingEntry::new(
                        "dmg_b_mode2_irq_prepare_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        2,
                        PhaseRule::AfterAnchor { subphases: 2 },
                    ),
                    // DMG-B v2.1 public goldens: hblank_ly_scx_timing-GS shows the
                    // STAT-write-visible mode2 IRQ prepare sample at line_tick=10.
                    TimingEntry::new(
                        "dmg_b_mode2_irq_prepare_after_stat_write_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        10,
                        PhaseRule::AfterAnchor { subphases: 10 },
                    ),
                    // DMG-B v2 public goldens: mode2_enter at line_tick=8;
                    // see v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv:25-26.
                    TimingEntry::new(
                        "dmg_b_mode2_enter_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        8,
                        PhaseRule::AfterAnchor { subphases: 8 },
                    ),
                    // DMG-B v2.1 public goldens: intr_2_0_timing includes an early
                    // mode2 enter sample at line_tick=4.
                    TimingEntry::new(
                        "dmg_b_mode2_enter_intr_early_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        4,
                        PhaseRule::AfterAnchor { subphases: 4 },
                    ),
                    // DMG-B v2.1 public goldens: hblank_ly_scx_timing-GS shows the
                    // STAT-write-visible mode2 enter sample at line_tick=16.
                    TimingEntry::new(
                        "dmg_b_mode2_enter_after_stat_write_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        16,
                        PhaseRule::AfterAnchor { subphases: 16 },
                    ),
                    TimingEntry::new(
                        "mode3_public_start",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        176,
                        PhaseRule::AfterAnchor { subphases: 176 },
                    ),
                    // DMG-B v2 public goldens: mode3_enter at line_tick=176;
                    // see v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv:28-29.
                    TimingEntry::new(
                        "dmg_b_mode3_enter_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        176,
                        PhaseRule::AfterAnchor { subphases: 176 },
                    ),
                    // DMG-B v2.1 public goldens: hblank_ly_scx_timing-GS SCX/STAT-write
                    // windows include a shortened mode3 public entry at line_tick=172.
                    TimingEntry::new(
                        "dmg_b_mode3_enter_scx_short_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        172,
                        PhaseRule::AfterAnchor { subphases: 172 },
                    ),
                    // DMG-B v2 public goldens: mode0_enter at line_tick=520;
                    // see v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv:30-31.
                    TimingEntry::new(
                        "dmg_b_mode0_enter_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        520,
                        PhaseRule::AfterAnchor { subphases: 520 },
                    ),
                    // DMG-B v2.1 public goldens: hblank_ly_scx_timing-GS variable
                    // mode3 length reaches HBlank at line_tick=516.
                    TimingEntry::new(
                        "dmg_b_mode0_enter_scx_short_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        516,
                        PhaseRule::AfterAnchor { subphases: 516 },
                    ),
                    // DMG-B v2.1 public goldens: hblank_ly_scx_timing-GS variable
                    // mode3 length also reaches HBlank at line_tick=524.
                    TimingEntry::new(
                        "dmg_b_mode0_enter_scx_mid_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        524,
                        PhaseRule::AfterAnchor { subphases: 524 },
                    ),
                    // DMG-B v2.1 public goldens: hblank_ly_scx_timing-GS variable
                    // mode3 length can delay HBlank to line_tick=528.
                    TimingEntry::new(
                        "dmg_b_mode0_enter_scx_long_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        528,
                        PhaseRule::AfterAnchor { subphases: 528 },
                    ),
                    // DMG-B v2.1 public goldens: hblank_ly_scx_timing-GS variable
                    // mode3 length also reaches HBlank at line_tick=532.
                    TimingEntry::new(
                        "dmg_b_mode0_enter_scx_longer_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        532,
                        PhaseRule::AfterAnchor { subphases: 532 },
                    ),
                    // DMG-B v2.1 public goldens: hblank_ly_scx_timing-GS variable
                    // mode3 length maxes out at line_tick=536 in this W1 capture.
                    TimingEntry::new(
                        "dmg_b_mode0_enter_scx_longest_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        536,
                        PhaseRule::AfterAnchor { subphases: 536 },
                    ),
                    // DMG-B v2 public goldens: frame_vblank/vblank_irq_edge at LY144,
                    // line_tick=6; see v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv:1321-1323.
                    TimingEntry::new(
                        "dmg_b_vblank_line",
                        Observable::PpuLy,
                        Anchor::PpuLineStart,
                        144,
                        PhaseRule::AfterAnchor { subphases: 144 },
                    ),
                    TimingEntry::new(
                        "dmg_b_vblank_irq_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        6,
                        PhaseRule::AfterAnchor { subphases: 6 },
                    ),
                    // DMG-B v2.1 public goldens: intr_2_0_timing captures one VBlank
                    // IRQ/public frame edge at line_tick=14.
                    TimingEntry::new(
                        "dmg_b_vblank_irq_late_tick",
                        Observable::PpuIrqEdge,
                        Anchor::PpuLineStart,
                        14,
                        PhaseRule::AfterAnchor { subphases: 14 },
                    ),
                    // DMG-B v2.1 public goldens: LCD-disable observation rows appear at
                    // LY144 line_tick=0 in lcdon_timing-GS/lcdon_write_timing-GS.
                    TimingEntry::new(
                        "dmg_b_lcd_off_line_tick",
                        Observable::PpuLcdOn,
                        Anchor::PpuLineStart,
                        0,
                        PhaseRule::AfterAnchor { subphases: 0 },
                    ),
                    // DMG-B v2.1 public goldens: first enabled line's OAM prelude is
                    // line_tick=156 in lcdon_timing-GS/lcdon_write_timing-GS.
                    TimingEntry::new(
                        "dmg_b_lcd_on_line0_oam_prelude_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        156,
                        PhaseRule::AfterAnchor { subphases: 156 },
                    ),
                    // DMG-B v2.1 public goldens: lcdon_write_timing-GS has one delayed
                    // first line-0 mode3 edge at line_tick=184.
                    TimingEntry::new(
                        "dmg_b_lcdon_write_first_mode3_enter_tick",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        184,
                        PhaseRule::AfterAnchor { subphases: 184 },
                    ),
                    // DMG-B v2 public goldens show the LY153 early sample before LY0 reset;
                    // see v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv:1340-1346.
                    TimingEntry::new(
                        "dmg_b_ly153_early_sample_tick",
                        Observable::PpuLy,
                        Anchor::PpuLineStart,
                        4,
                        PhaseRule::AfterAnchor { subphases: 4 },
                    ),
                ],
            },
            ppu_internal: TimingProfile {
                name: "ppu_internal",
                entries: vec![
                    TimingEntry::new(
                        "bg_fetch_tile_no_t1",
                        Observable::PpuFetchSample,
                        Anchor::PpuMode3Start,
                        14 * 4,
                        PhaseRule::AfterAnchor { subphases: 14 * 4 },
                    ),
                    TimingEntry::new(
                        "bg_fetch_tile_high_t1",
                        Observable::PpuFetchSample,
                        Anchor::PpuMode3Start,
                        22 * 4,
                        PhaseRule::AfterAnchor { subphases: 22 * 4 },
                    ),
                ],
            },
            output: TimingProfile {
                name: "output",
                entries: vec![TimingEntry::new(
                    "lcd_column_latch",
                    Observable::OutputPixelLatch,
                    Anchor::OutputColumn,
                    0,
                    PhaseRule::AtAnchor,
                )],
            },
            bus_conflicts: TimingProfile {
                name: "bus_conflicts",
                entries: vec![TimingEntry::new(
                    "io_write_conflict_visible",
                    Observable::BusConflict,
                    Anchor::BusWriteStart,
                    0,
                    PhaseRule::AtAnchor,
                )],
            },
            dma: TimingProfile {
                name: "dma",
                entries: vec![TimingEntry::new(
                    "oam_dma_beat",
                    Observable::DmaBeat,
                    Anchor::DmaStart,
                    4,
                    PhaseRule::AfterAnchor { subphases: 4 },
                )],
            },
            timer: TimingProfile {
                name: "timer",
                entries: vec![TimingEntry::new(
                    "div_falling_edge",
                    Observable::TimerEdge,
                    Anchor::TimerDivEdge,
                    0,
                    PhaseRule::AtAnchor,
                )],
            },
        }
    }

    pub fn lookup(&self, domain: TimingDomain, name: &str) -> Option<&TimingEntry> {
        self.profile(domain).lookup(name)
    }

    pub fn profile(&self, domain: TimingDomain) -> &TimingProfile {
        match domain {
            TimingDomain::Boot => &self.boot,
            TimingDomain::Cpu => &self.cpu,
            TimingDomain::PpuPublic => &self.ppu_public,
            TimingDomain::PpuInternal => &self.ppu_internal,
            TimingDomain::Output => &self.output,
            TimingDomain::BusConflicts => &self.bus_conflicts,
            TimingDomain::Dma => &self.dma,
            TimingDomain::Timer => &self.timer,
        }
    }

    pub fn ppu_dot_phase(&self) -> PhaseRule {
        self.lookup(TimingDomain::PpuPublic, "ppu_dot")
            .map(|entry| entry.phase)
            .unwrap_or(PhaseRule::EveryCpuT { divisor: 1 })
    }

    pub fn cpu_read_phase(&self) -> PhaseRule {
        self.lookup(TimingDomain::Cpu, "cpu_read_sample_end_m")
            .map(|entry| entry.phase)
            .unwrap_or(PhaseRule::AfterAnchor { subphases: 16 })
    }

    pub fn cpu_write_phase(&self) -> PhaseRule {
        self.lookup(TimingDomain::Cpu, "cpu_write_drive_t2")
            .map(|entry| entry.phase)
            .unwrap_or(PhaseRule::AfterAnchor { subphases: 8 })
    }

    pub fn cpu_intr_poll_phase(&self) -> PhaseRule {
        self.lookup(TimingDomain::Cpu, "cpu_intr_poll_boundary")
            .map(|entry| entry.phase)
            .unwrap_or(PhaseRule::AtAnchor)
    }

    pub fn ppu_public_offset(&self, name: &str) -> Option<u64> {
        self.lookup(TimingDomain::PpuPublic, name)
            .map(|entry| entry.offset.subphases())
    }
}
