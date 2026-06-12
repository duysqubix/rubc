use crate::bus_intent::{CpuBusIntent, IntentOutcome};
use crate::timing::{PhaseRule, TimingTable};

pub const SUBPHASES_PER_T: u64 = 4;
pub const CPU_T_PER_M: u64 = 4;
pub const SUBPHASES_PER_M: u64 = SUBPHASES_PER_T * CPU_T_PER_M;
pub const DMG_DOTS_PER_LINE: u64 = 456;
pub const DMG_LINES_PER_FRAME: u64 = 154;
pub const DMG_DOTS_PER_FRAME: u64 = DMG_DOTS_PER_LINE * DMG_LINES_PER_FRAME;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(u64);

impl Time {
    pub const ZERO: Self = Self(0);

    pub const fn from_subphases(subphases: u64) -> Self {
        Self(subphases)
    }

    pub const fn from_t(cpu_t: u64) -> Self {
        Self(cpu_t * SUBPHASES_PER_T)
    }

    pub const fn subphases(self) -> u64 {
        self.0
    }

    pub const fn cpu_t(self) -> u64 {
        self.0 / SUBPHASES_PER_T
    }

    pub const fn subphase_in_t(self) -> u8 {
        (self.0 % SUBPHASES_PER_T) as u8
    }

    pub fn advance(&mut self, subphases: u64) {
        self.0 += subphases;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ClockPhase {
    #[default]
    CpuT0,
    CpuT1,
    CpuT2,
    CpuT3,
}

impl ClockPhase {
    const fn from_subphase(subphase: u8) -> Self {
        match subphase {
            0 => Self::CpuT0,
            1 => Self::CpuT1,
            2 => Self::CpuT2,
            _ => Self::CpuT3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockSpine {
    pub now: Time,
    pub cpu_t: u64,
    pub ppu_dot: u64,
    pub line_dot: u16,
    pub frame_dot: u32,
    pub phase: ClockPhase,
}

impl Default for ClockSpine {
    fn default() -> Self {
        Self {
            now: Time::ZERO,
            cpu_t: 0,
            ppu_dot: 0,
            line_dot: 0,
            frame_dot: 0,
            phase: ClockPhase::CpuT0,
        }
    }
}

impl ClockSpine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn step_subphase(&mut self, table: &TimingTable) {
        self.step_subphase_with_ppu_divisor(table, 1);
    }

    pub fn step_subphase_with_ppu_divisor(&mut self, table: &TimingTable, ppu_divisor: u8) {
        let old_t = self.now.cpu_t();
        self.now.advance(1);
        self.cpu_t = self.now.cpu_t();
        self.phase = ClockPhase::from_subphase(self.now.subphase_in_t());

        let phase = match table.ppu_dot_phase() {
            PhaseRule::EveryCpuT { divisor } => PhaseRule::EveryCpuT {
                divisor: divisor.saturating_mul(ppu_divisor).max(1),
            },
            phase => phase,
        };

        if self.cpu_t != old_t && Self::phase_fires(phase, self.cpu_t) {
            self.ppu_dot += 1;
            self.line_dot = (self.ppu_dot % DMG_DOTS_PER_LINE) as u16;
            self.frame_dot = (self.ppu_dot % DMG_DOTS_PER_FRAME) as u32;
        }
    }

    pub fn apply_cpu_intent(&self, intent: CpuBusIntent, table: &TimingTable) -> IntentOutcome {
        let phase = match intent {
            CpuBusIntent::ReadSample { .. } => table.cpu_read_phase(),
            CpuBusIntent::WriteDrive { .. } => table.cpu_write_phase(),
            CpuBusIntent::Idle => PhaseRule::AtAnchor,
            CpuBusIntent::IntrPoll => table.cpu_intr_poll_phase(),
        };

        IntentOutcome {
            intent,
            apply_at: phase.resolve(self.now),
        }
    }

    const fn phase_fires(phase: PhaseRule, cpu_t: u64) -> bool {
        match phase {
            PhaseRule::EveryCpuT { divisor } => {
                divisor != 0 && cpu_t.is_multiple_of(divisor as u64)
            }
            _ => true,
        }
    }
}
