use crate::bus_intent::{CpuBusIntent, IntentOutcome};
use crate::model::GbModel;
use crate::time::{ClockSpine, Time};
use crate::timing::TimingTable;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRecord {
    pub time: Time,
    pub cpu_t: u64,
    pub ppu_dot: u64,
    pub intent: CpuBusIntent,
    pub outcome: IntentOutcome,
}

#[derive(Clone, Debug)]
pub struct MachineNg {
    model: GbModel,
    rom: Vec<u8>,
    pc: u16,
    spine: ClockSpine,
    table: TimingTable,
}

impl MachineNg {
    pub fn from_rom(model: GbModel, rom: &[u8]) -> Result<Self, String> {
        if rom.is_empty() {
            return Err("ROM must contain at least one byte".to_owned());
        }

        Ok(Self {
            model,
            rom: rom.to_vec(),
            pc: 0,
            spine: ClockSpine::new(),
            table: TimingTable::for_model(model),
        })
    }

    pub fn model(&self) -> GbModel {
        self.model
    }

    pub fn spine(&self) -> &ClockSpine {
        &self.spine
    }

    pub fn run_steps(&mut self, steps: usize) -> Vec<StepRecord> {
        (0..steps).map(|_| self.step()).collect()
    }

    pub fn step(&mut self) -> StepRecord {
        let intent = self.next_intent();
        let outcome = self.spine.apply_cpu_intent(intent, &self.table);
        let record = StepRecord {
            time: self.spine.now,
            cpu_t: self.spine.cpu_t,
            ppu_dot: self.spine.ppu_dot,
            intent,
            outcome,
        };
        self.spine.step_subphase(&self.table);
        record
    }

    fn next_intent(&mut self) -> CpuBusIntent {
        let addr = self.pc;
        let opcode = self.rom.get(addr as usize).copied().unwrap_or(0x00);
        self.pc = self.pc.wrapping_add(1);

        match opcode {
            0x00 => CpuBusIntent::ReadSample { addr },
            _ => CpuBusIntent::Idle,
        }
    }
}
