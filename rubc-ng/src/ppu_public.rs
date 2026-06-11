use crate::golden::{GoldenSelection, GoldenV2Reader, ObservableSample, ObservableValue};
use crate::model::GbModel;
use crate::time::Time;
use crate::timing::{Observable, TimingTable};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PpuRegisterWrite {
    pub time: Time,
    pub addr: u16,
    pub value: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PpuPublicEvent {
    Mode2IrqPrepare,
    Mode2Enter,
    Mode3Enter,
    Mode0Enter,
    FrameVBlank,
}

impl PpuPublicEvent {
    pub fn name(self) -> &'static str {
        match self {
            Self::Mode2IrqPrepare => "mode2_irq_prepare",
            Self::Mode2Enter => "mode2_enter",
            Self::Mode3Enter => "mode3_enter",
            Self::Mode0Enter => "mode0_enter",
            Self::FrameVBlank => "frame_vblank",
        }
    }

    fn parse(name: &str) -> Result<Self, String> {
        match name {
            "mode2_irq_prepare" => Ok(Self::Mode2IrqPrepare),
            "mode2_enter" => Ok(Self::Mode2Enter),
            "mode3_enter" => Ok(Self::Mode3Enter),
            "mode0_enter" => Ok(Self::Mode0Enter),
            "frame_vblank" => Ok(Self::FrameVBlank),
            _ => Err(format!("unsupported PPU-public event for slice 1: {name}")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PpuPublic {
    model: GbModel,
    table: TimingTable,
    origin: Time,
    base_frame: u64,
    lcdc: u8,
    stat: u8,
    lyc: u8,
}

impl PpuPublic {
    pub fn new(model: GbModel, origin: Time, base_frame: u64) -> Self {
        Self {
            model,
            table: TimingTable::for_model(model),
            origin,
            base_frame,
            lcdc: 0x91,
            stat: 0,
            lyc: 0,
        }
    }

    pub fn model(&self) -> GbModel {
        self.model
    }

    pub fn write_register(&mut self, write: PpuRegisterWrite) {
        match write.addr {
            0xFF40 => self.lcdc = write.value,
            0xFF41 => self.stat = write.value,
            0xFF45 => self.lyc = write.value,
            _ => {}
        }
    }

    pub fn lcdc(&self) -> u8 {
        self.lcdc
    }

    pub fn stat(&self) -> u8 {
        self.stat
    }

    pub fn lyc(&self) -> u8 {
        self.lyc
    }

    pub fn sample_event(
        &self,
        now: Time,
        event: PpuPublicEvent,
        observable: Observable,
    ) -> Option<ObservableSample> {
        if self.lcdc & 0x80 == 0 {
            return None;
        }

        let position = self.position(now)?;
        let mode = self.mode_for(event, position.ly, position.line_tick)?;
        let value = match observable {
            Observable::PpuModeEdge => ObservableValue::U8(mode),
            Observable::PpuLy => ObservableValue::U16(position.ly),
            _ => return None,
        };

        Some(ObservableSample {
            time: now,
            observable,
            value,
        })
    }

    fn position(&self, now: Time) -> Option<PpuPosition> {
        let elapsed = now.subphases().checked_sub(self.origin.subphases())?;
        let line_ticks = self.entry("dmg_b_line_ticks")?;
        let lines_per_frame = self.entry("dmg_b_lines_per_frame")?;
        let frame_ticks = line_ticks.checked_mul(lines_per_frame)?;
        let frame_elapsed = elapsed % frame_ticks;
        let line = frame_elapsed / line_ticks;
        let line_tick = frame_elapsed % line_ticks;

        Some(PpuPosition {
            frame: self.base_frame + elapsed / frame_ticks,
            ly: line as u16,
            line_tick,
        })
    }

    fn mode_for(&self, event: PpuPublicEvent, ly: u16, line_tick: u64) -> Option<u8> {
        let vblank_line = self.entry("dmg_b_vblank_line")? as u16;
        match event {
            PpuPublicEvent::Mode2IrqPrepare => (ly < vblank_line
                && line_tick == self.entry("dmg_b_mode2_irq_prepare_tick")?)
            .then_some(0),
            PpuPublicEvent::Mode2Enter => (ly < vblank_line
                && line_tick == self.entry("dmg_b_mode2_enter_tick")?)
            .then_some(2),
            PpuPublicEvent::Mode3Enter => (ly < vblank_line
                && line_tick == self.entry("dmg_b_mode3_enter_tick")?)
            .then_some(3),
            PpuPublicEvent::Mode0Enter => (ly < vblank_line
                && line_tick == self.entry("dmg_b_mode0_enter_tick")?)
            .then_some(0),
            PpuPublicEvent::FrameVBlank => (ly == vblank_line
                && line_tick == self.entry("dmg_b_vblank_irq_tick")?)
            .then_some(1),
        }
    }

    fn entry(&self, name: &str) -> Option<u64> {
        self.table.ppu_public_offset(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PpuPosition {
    frame: u64,
    ly: u16,
    line_tick: u64,
}

pub fn replay_ppu_public_observable(
    path: impl AsRef<Path>,
    model: GbModel,
    event: &str,
    observable: Observable,
    selection: GoldenSelection,
) -> Result<Vec<ObservableSample>, String> {
    let event = PpuPublicEvent::parse(event)?;
    let path = path.as_ref();
    let selected = GoldenV2Reader::open(path)?
        .filter_selection(selection)
        .filter_map(|row| match row {
            Ok(row) if row.event.as_deref() == Some(event.name()) => Some(Ok(row)),
            Ok(_) => None,
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let first = selected
        .first()
        .ok_or_else(|| format!("no selected PPU-public rows for {}", event.name()))?;
    let table = TimingTable::for_model(model);
    let line_ticks = table
        .ppu_public_offset("dmg_b_line_ticks")
        .ok_or_else(|| "missing dmg_b_line_ticks timing entry".to_owned())?;
    let first_ly = first
        .ly
        .ok_or_else(|| "selected row is missing LY".to_owned())? as u64;
    let first_line_tick = first
        .line_tick
        .ok_or_else(|| "selected row is missing line_tick".to_owned())?;
    let origin = Time::from_subphases(
        first
            .raw_tick
            .saturating_sub(first_ly * line_ticks + first_line_tick),
    );
    let mut ppu = PpuPublic::new(model, origin, first.frame);
    let writes = extract_register_writes(path)?;
    let mut next_write = 0;
    let mut actual = Vec::with_capacity(selected.len());

    for row in selected {
        let now = Time::from_subphases(row.raw_tick);
        while writes
            .get(next_write)
            .is_some_and(|write| write.time <= now)
        {
            ppu.write_register(writes[next_write]);
            next_write += 1;
        }
        if let Some(sample) = ppu.sample_event(now, event, observable) {
            actual.push(sample);
        }
    }

    Ok(actual)
}

fn extract_register_writes(path: &Path) -> Result<Vec<PpuRegisterWrite>, String> {
    let mut writes = GoldenV2Reader::open(path)?
        .filter_map(|row| match row {
            Ok(row) => match (row.addr, row.byte) {
                (Some(addr @ (0xFF40 | 0xFF41 | 0xFF45)), Some(value)) => {
                    Some(Ok(PpuRegisterWrite {
                        time: Time::from_subphases(row.raw_tick),
                        addr,
                        value,
                    }))
                }
                _ => None,
            },
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    writes.sort_by_key(|write| write.time);
    Ok(writes)
}
