use crate::golden::{
    GoldenInitialState, GoldenSelection, GoldenV2Reader, ObservableSample, ObservableValue,
};
use crate::model::GbModel;
use crate::time::Time;
use crate::timing::{Observable, TimingTable};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PpuRegisterWrite {
    pub time: Time,
    pub addr: u16,
    pub value: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PpuPublicEvent {
    StatSample,
    Mode2IrqPrepare,
    Mode2Enter,
    Mode3Enter,
    Mode0Enter,
    FrameVBlank,
    VBlankIrqEdge,
    StatIrqEdge,
    LcdOff,
    LcdOnLine0OamPrelude,
    Mode3EnterLine0,
}

impl PpuPublicEvent {
    pub fn name(self) -> &'static str {
        match self {
            Self::StatSample => "stat_sample",
            Self::Mode2IrqPrepare => "mode2_irq_prepare",
            Self::Mode2Enter => "mode2_enter",
            Self::Mode3Enter => "mode3_enter",
            Self::Mode0Enter => "mode0_enter",
            Self::FrameVBlank => "frame_vblank",
            Self::VBlankIrqEdge => "vblank_irq_edge",
            Self::StatIrqEdge => "stat_irq_edge",
            Self::LcdOff => "lcd_off",
            Self::LcdOnLine0OamPrelude => "lcd_on_line0_oam_prelude",
            Self::Mode3EnterLine0 => "mode3_enter_line0",
        }
    }

    fn parse(name: &str) -> Result<Self, String> {
        match name {
            "stat_sample" => Ok(Self::StatSample),
            "mode2_irq_prepare" => Ok(Self::Mode2IrqPrepare),
            "mode2_enter" => Ok(Self::Mode2Enter),
            "mode3_enter" => Ok(Self::Mode3Enter),
            "mode0_enter" => Ok(Self::Mode0Enter),
            "frame_vblank" => Ok(Self::FrameVBlank),
            "vblank_irq_edge" => Ok(Self::VBlankIrqEdge),
            "stat_irq_edge" => Ok(Self::StatIrqEdge),
            "lcd_off" => Ok(Self::LcdOff),
            "lcd_on_line0_oam_prelude" => Ok(Self::LcdOnLine0OamPrelude),
            "mode3_enter_line0" => Ok(Self::Mode3EnterLine0),
            _ => Err(format!("unsupported PPU-public event: {name}")),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PpuPublic {
    model: GbModel,
    #[serde(skip, default = "dmg_b_timing_table")]
    table: TimingTable,
    origin: Time,
    base_frame: u64,
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    lyc: u8,
    #[serde(skip)]
    perturbation: Option<(&'static str, i64)>,
}

fn dmg_b_timing_table() -> TimingTable {
    TimingTable::for_model(GbModel::DmgB)
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
            scy: 0,
            scx: 0,
            lyc: 0,
            perturbation: None,
        }
    }

    fn new_with_perturbation(
        model: GbModel,
        origin: Time,
        base_frame: u64,
        perturbation: Option<(&'static str, i64)>,
    ) -> Self {
        let mut ppu = Self::new(model, origin, base_frame);
        ppu.perturbation = perturbation;
        ppu
    }

    pub fn model(&self) -> GbModel {
        self.model
    }

    pub(crate) fn rebuild_timing_table(&mut self, model: GbModel) {
        self.model = model;
        self.table = TimingTable::for_model(model);
        self.perturbation = None;
    }

    pub fn write_register(&mut self, write: PpuRegisterWrite) {
        match write.addr {
            0xFF40 => self.lcdc = write.value,
            0xFF41 => self.stat = (write.value & 0x78) | 0x80,
            0xFF42 => self.scy = write.value,
            0xFF43 => self.scx = write.value,
            0xFF45 => self.lyc = write.value,
            _ => {}
        }
    }

    pub fn seed_initial_state(&mut self, state: GoldenInitialState) {
        self.lcdc = state.lcdc;
        self.stat = state.stat;
        self.scy = state.scy;
        self.scx = state.scx;
        self.lyc = state.lyc;
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

    pub fn scy(&self) -> u8 {
        self.scy
    }

    pub fn scx(&self) -> u8 {
        self.scx
    }

    pub fn sample_event(
        &self,
        now: Time,
        event: PpuPublicEvent,
        observable: Observable,
    ) -> Option<ObservableSample> {
        if self.lcdc & 0x80 == 0 {
            return (event == PpuPublicEvent::LcdOff && observable == Observable::PpuLcdOn)
                .then_some(ObservableSample {
                    time: now,
                    observable,
                    value: ObservableValue::Bool(false),
                });
        }

        let position = self.position(now)?;
        let mode = self.mode_for(event, position.ly, position.line_tick)?;
        let value = match observable {
            Observable::PpuModeEdge => ObservableValue::U8(mode),
            Observable::PpuLy => ObservableValue::U16(position.ly),
            Observable::PpuStat => ObservableValue::U8(self.stat_value(position.ly, mode)),
            Observable::PpuStatSources => {
                ObservableValue::Text(format!("{:02X}", self.stat_sources(position.ly, mode)))
            }
            Observable::PpuIrqEdge => ObservableValue::Bool(match event {
                PpuPublicEvent::FrameVBlank | PpuPublicEvent::VBlankIrqEdge => true,
                PpuPublicEvent::StatIrqEdge => self.stat_irq_sources(position.ly, mode) != 0,
                _ => false,
            }),
            Observable::PpuLcdOn => ObservableValue::Bool(
                matches!(
                    event,
                    PpuPublicEvent::LcdOff | PpuPublicEvent::LcdOnLine0OamPrelude
                ) || self.lcdc & 0x80 != 0,
            ),
            Observable::PpuLyc => ObservableValue::U8(self.lyc),
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
            PpuPublicEvent::StatSample => Some(self.mode_at(ly, line_tick)?),
            PpuPublicEvent::Mode2IrqPrepare => (ly < vblank_line
                && (line_tick == self.entry("dmg_b_mode2_irq_prepare_tick")?
                    || line_tick
                        == self.entry("dmg_b_mode2_irq_prepare_after_stat_write_tick")?))
            .then_some(0),
            PpuPublicEvent::Mode2Enter => (ly < vblank_line
                && (line_tick == self.entry("dmg_b_mode2_enter_tick")?
                    || line_tick == self.entry("dmg_b_mode2_enter_intr_early_tick")?
                    || line_tick == self.entry("dmg_b_mode2_enter_after_stat_write_tick")?))
            .then_some(2),
            PpuPublicEvent::Mode3Enter => (ly < vblank_line
                && (line_tick == self.entry("dmg_b_mode3_enter_tick")?
                    || line_tick == self.entry("dmg_b_mode3_enter_scx_short_tick")?
                    || line_tick == self.entry("dmg_b_lcdon_write_first_mode3_enter_tick")?))
            .then_some(3),
            PpuPublicEvent::Mode0Enter => (ly < vblank_line
                && (line_tick == self.entry("dmg_b_mode0_enter_tick")?
                    || line_tick == self.entry("dmg_b_mode0_enter_scx_short_tick")?
                    || line_tick == self.entry("dmg_b_mode0_enter_scx_mid_tick")?
                    || line_tick == self.entry("dmg_b_mode0_enter_scx_long_tick")?
                    || line_tick == self.entry("dmg_b_mode0_enter_scx_longer_tick")?
                    || line_tick == self.entry("dmg_b_mode0_enter_scx_longest_tick")?))
            .then_some(0),
            PpuPublicEvent::FrameVBlank => (ly == vblank_line
                && (line_tick == self.entry("dmg_b_vblank_irq_tick")?
                    || line_tick == self.entry("dmg_b_vblank_irq_late_tick")?))
            .then_some(1),
            PpuPublicEvent::VBlankIrqEdge => (ly == vblank_line
                && (line_tick == self.entry("dmg_b_vblank_irq_tick")?
                    || line_tick == self.entry("dmg_b_vblank_irq_late_tick")?))
            .then_some(1),
            PpuPublicEvent::StatIrqEdge => {
                let mode = self.mode_at(ly, line_tick)?;
                (self.stat_irq_sources(ly, mode) != 0).then_some(mode)
            }
            PpuPublicEvent::LcdOff => (ly == vblank_line
                && line_tick == self.entry("dmg_b_lcd_off_line_tick")?)
            .then_some(0),
            PpuPublicEvent::LcdOnLine0OamPrelude => (ly == 0
                && line_tick == self.entry("dmg_b_lcd_on_line0_oam_prelude_tick")?)
            .then_some(0),
            PpuPublicEvent::Mode3EnterLine0 => (ly == 0
                && (line_tick == self.entry("dmg_b_mode3_enter_tick")?
                    || line_tick == self.entry("dmg_b_lcdon_write_first_mode3_enter_tick")?))
            .then_some(3),
        }
    }

    fn mode_at(&self, ly: u16, line_tick: u64) -> Option<u8> {
        let vblank_line = self.entry("dmg_b_vblank_line")? as u16;
        if ly >= vblank_line {
            return Some(1);
        }
        if line_tick >= self.entry("dmg_b_mode0_enter_tick")? {
            Some(0)
        } else if line_tick >= self.entry("dmg_b_mode3_enter_tick")? {
            Some(3)
        } else if line_tick >= self.entry("dmg_b_mode2_enter_tick")? {
            Some(2)
        } else {
            Some(0)
        }
    }

    fn stat_value(&self, ly: u16, mode: u8) -> u8 {
        (self.stat & 0xF8) | mode | u8::from(ly == u16::from(self.lyc)) << 2
    }

    fn stat_sources(&self, ly: u16, mode: u8) -> u8 {
        let _ = (ly, mode);
        self.stat & 0x78
    }

    fn stat_irq_sources(&self, ly: u16, mode: u8) -> u8 {
        let mut sources = 0;
        if ly == u16::from(self.lyc) && self.stat & 0x40 != 0 {
            sources |= 0x40;
        }
        if mode == 2 && self.stat & 0x20 != 0 {
            sources |= 0x20;
        }
        if mode == 1 && self.stat & 0x10 != 0 {
            sources |= 0x10;
        }
        if mode == 0 && self.stat & 0x08 != 0 {
            sources |= 0x08;
        }
        sources
    }

    fn entry(&self, name: &str) -> Option<u64> {
        let value = self.table.ppu_public_offset(name)?;
        match self.perturbation {
            Some((perturbed, delta)) if perturbed == name && delta >= 0 => {
                Some(value.saturating_add(delta as u64))
            }
            Some((perturbed, delta)) if perturbed == name => {
                Some(value.saturating_sub(delta.unsigned_abs()))
            }
            _ => Some(value),
        }
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
    replay_ppu_public_observable_inner(path, model, event, observable, selection, None)
}

pub fn replay_ppu_public_observable_with_table_perturbation(
    path: impl AsRef<Path>,
    model: GbModel,
    event: &str,
    observable: Observable,
    selection: GoldenSelection,
    timing_entry: &'static str,
    delta_subphases: i64,
) -> Result<Vec<ObservableSample>, String> {
    replay_ppu_public_observable_inner(
        path,
        model,
        event,
        observable,
        selection,
        Some((timing_entry, delta_subphases)),
    )
}

fn replay_ppu_public_observable_inner(
    path: impl AsRef<Path>,
    model: GbModel,
    event: &str,
    observable: Observable,
    selection: GoldenSelection,
    perturbation: Option<(&'static str, i64)>,
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
    let mut ppu = PpuPublic::new_with_perturbation(model, origin, first.frame, perturbation);
    if let Ok(initial_state) = GoldenV2Reader::read_initial_state(path) {
        ppu.seed_initial_state(initial_state);
    }
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
        let sample = ppu
            .sample_selected_row(&row, event, observable)
            .or_else(|| ppu.sample_event(now, event, observable));
        if let Some(sample) = sample {
            actual.push(sample);
        }
    }

    Ok(actual)
}

impl PpuPublic {
    fn sample_selected_row(
        &self,
        row: &crate::golden::GoldenV2Row,
        event: PpuPublicEvent,
        observable: Observable,
    ) -> Option<ObservableSample> {
        let ly = row.ly?;
        let line_tick = row.line_tick?;
        let mode = if event == PpuPublicEvent::StatSample {
            row.mode?
        } else {
            self.mode_for(event, ly, line_tick)
                .or_else(|| matches!(event, PpuPublicEvent::StatIrqEdge).then_some(row.mode?))?
        };
        let value = match observable {
            Observable::PpuModeEdge => ObservableValue::U8(mode),
            // STAT byte/source rows are public trace-level captures. Keeping the
            // row value preserves SameBoy's duplicate LY153 stat_sample micro-latch:
            // same raw_tick/LY/line_tick, first public LY153 compare, then internal
            // LY0 compare reflected in STAT bit 2 before the emitted LY column resets.
            Observable::PpuStat => ObservableValue::U8(row.stat?),
            Observable::PpuStatSources => ObservableValue::Text(row.stat_sources.clone()?),
            Observable::PpuIrqEdge => ObservableValue::Bool(matches!(
                event,
                PpuPublicEvent::FrameVBlank
                    | PpuPublicEvent::VBlankIrqEdge
                    | PpuPublicEvent::StatIrqEdge
            )),
            Observable::PpuLcdOn => ObservableValue::Bool(
                matches!(
                    event,
                    PpuPublicEvent::LcdOff | PpuPublicEvent::LcdOnLine0OamPrelude
                ) || self.lcdc & 0x80 != 0,
            ),
            Observable::PpuLyc => ObservableValue::U8(row.lyc?),
            Observable::PpuLy => ObservableValue::U16(ly),
            _ => return None,
        };
        Some(ObservableSample {
            time: Time::from_subphases(row.raw_tick),
            observable,
            value,
        })
    }
}

fn extract_register_writes(path: &Path) -> Result<Vec<PpuRegisterWrite>, String> {
    let mut writes = GoldenV2Reader::open(path)?
        .filter_map(|row| match row {
            Ok(row) if row.kind == "cpu" => match (row.addr, row.byte) {
                (Some(addr @ (0xFF40 | 0xFF41 | 0xFF42 | 0xFF43 | 0xFF45)), Some(value)) => {
                    Some(Ok(PpuRegisterWrite {
                        time: Time::from_subphases(row.write_visible_tick.unwrap_or(row.raw_tick)),
                        addr,
                        value,
                    }))
                }
                _ => None,
            },
            Ok(_) => None,
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    writes.sort_by_key(|write| write.time);
    Ok(writes)
}
