use crate::time::Time;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CpuBusIntent {
    ReadSample { addr: u16 },
    WriteDrive { addr: u16, value: u8 },
    Idle,
    IntrPoll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct IntentOutcome {
    pub intent: CpuBusIntent,
    pub apply_at: Time,
}

pub trait CpuIntentSource {
    fn next_intent(&mut self) -> CpuBusIntent;
}
