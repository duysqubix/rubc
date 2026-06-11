use crate::model::GbModel;
use crate::time::Time;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Observable {
    BootRomExit,
    CpuReadSample,
    CpuWriteDrive,
    CpuIdle,
    CpuIntrPoll,
    PpuModeEdge,
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
        let ppu_dot_phase = if model.is_cgb() {
            PhaseRule::EveryCpuT { divisor: 1 }
        } else {
            PhaseRule::EveryCpuT { divisor: 1 }
        };

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
                    TimingEntry::new(
                        "ppu_dot",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        0,
                        ppu_dot_phase,
                    ),
                    TimingEntry::new(
                        "mode3_public_start",
                        Observable::PpuModeEdge,
                        Anchor::PpuLineStart,
                        80 * 4,
                        PhaseRule::AfterAnchor { subphases: 80 * 4 },
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
}
