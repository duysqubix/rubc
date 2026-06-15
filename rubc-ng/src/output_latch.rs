use crate::golden::{GoldenRow, GoldenTrace};
use crate::time::Time;
use crate::timing::{TimingDomain, TimingTable};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum LcdPaletteSource {
    Bg,
    Obp0,
    Obp1,
}

impl LcdPaletteSource {
    fn from_golden(kind: &str) -> Result<Self, String> {
        match kind {
            "BG" => Ok(Self::Bg),
            "OBJ0" => Ok(Self::Obp0),
            "OBJ1" => Ok(Self::Obp1),
            other => Err(format!("unsupported palette source {other}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PaletteWrite {
    pub time: Time,
    pub source: LcdPaletteSource,
    pub value: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutputRawPixel {
    pub time: Time,
    pub ly: u16,
    pub x: usize,
    pub source: LcdPaletteSource,
    pub raw_color: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LatchedPixel {
    pub latch_time: Time,
    pub ly: u16,
    pub x: usize,
    pub source: LcdPaletteSource,
    pub raw_color: u8,
    pub sampled_palette_value: u8,
    pub final_color: u8,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LcdOutputLatch {
    palette: BTreeMap<LcdPaletteSource, Vec<PaletteWrite>>,
    latch_perturbation: i64,
    forced_source: Option<LcdPaletteSource>,
    #[serde(skip, default = "dmg_b_timing_table")]
    table: TimingTable,
}

fn dmg_b_timing_table() -> TimingTable {
    TimingTable::for_model(crate::model::GbModel::DmgB)
}

impl LcdOutputLatch {
    pub fn dmg_default() -> Self {
        Self {
            palette: BTreeMap::new(),
            latch_perturbation: 0,
            forced_source: None,
            table: TimingTable::for_model(crate::model::GbModel::DmgB),
        }
    }

    fn with_latch_perturbation(mut self, perturbation: i64) -> Self {
        self.latch_perturbation = perturbation;
        self
    }

    fn with_forced_source(mut self, source: LcdPaletteSource) -> Self {
        self.forced_source = Some(source);
        self
    }

    pub fn apply_write(&mut self, write: PaletteWrite) {
        let writes = self.palette.entry(write.source).or_default();
        writes.push(write);
        writes.sort_by_key(|write| write.time);
    }

    pub(crate) fn rebuild_timing_table(&mut self, model: crate::model::GbModel) {
        self.table = TimingTable::for_model(model);
    }

    pub fn latch_pixel(&self, pixel: OutputRawPixel) -> Result<LatchedPixel, String> {
        let source = self.forced_source.unwrap_or(pixel.source);
        let latch_time = self.latch_time(pixel.time);
        let sampled_palette_value = self.sample_palette(source, latch_time).unwrap_or(0);
        let final_color = (sampled_palette_value >> (u16::from(pixel.raw_color.min(3)) * 2)) & 0x03;

        Ok(LatchedPixel {
            latch_time,
            ly: pixel.ly,
            x: pixel.x,
            source,
            raw_color: pixel.raw_color,
            sampled_palette_value,
            final_color,
        })
    }

    fn latch_time(&self, column_time: Time) -> Time {
        let table_offset = self
            .table
            .lookup(TimingDomain::Output, "lcd_column_latch")
            .map(|entry| entry.offset.subphases())
            .unwrap_or(0);
        let base = column_time.subphases().saturating_add(table_offset);
        if self.latch_perturbation >= 0 {
            Time::from_subphases(base.saturating_add(self.latch_perturbation as u64))
        } else {
            Time::from_subphases(base.saturating_sub(self.latch_perturbation.unsigned_abs()))
        }
    }

    fn sample_palette(&self, source: LcdPaletteSource, time: Time) -> Option<u8> {
        self.palette.get(&source).and_then(|writes| {
            writes
                .iter()
                .rev()
                .find(|write| write.time <= time)
                .map(|write| write.value)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LcdOutputPaletteDivergence {
    rom: String,
    index: usize,
    raw_tick: u64,
    ly: u16,
    x: usize,
    machine_source: LcdPaletteSource,
    golden_source: LcdPaletteSource,
    raw_color: u8,
    machine_palette: u8,
    golden_palette: u8,
    machine_final_color: u8,
}

impl fmt::Display for LcdOutputPaletteDivergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "first LCD output palette divergence for {} at output #{} tick {} LY {} x {}: machine source {:?}, golden source {:?}, raw {}, machine palette {:02X}, golden palette {:02X}, machine final color {}",
            self.rom,
            self.index,
            self.raw_tick,
            self.ly,
            self.x,
            self.machine_source,
            self.golden_source,
            self.raw_color,
            self.machine_palette,
            self.golden_palette,
            self.machine_final_color,
        )
    }
}

impl std::error::Error for LcdOutputPaletteDivergence {}

pub fn assert_lcd_output_palette_golden(
    path: impl AsRef<Path>,
) -> Result<(), LcdOutputPaletteDivergence> {
    assert_lcd_output_palette_golden_inner(path.as_ref(), 0, None)
}

pub fn assert_lcd_output_palette_golden_with_perturbation(
    path: impl AsRef<Path>,
    perturbation: i64,
) -> Result<(), LcdOutputPaletteDivergence> {
    assert_lcd_output_palette_golden_inner(path.as_ref(), perturbation, None)
}

pub fn assert_lcd_output_palette_golden_with_wrong_register(
    path: impl AsRef<Path>,
) -> Result<(), LcdOutputPaletteDivergence> {
    assert_lcd_output_palette_golden_inner(path.as_ref(), 0, Some(LcdPaletteSource::Obp0))
}

fn assert_lcd_output_palette_golden_inner(
    path: &Path,
    perturbation: i64,
    forced_source: Option<LcdPaletteSource>,
) -> Result<(), LcdOutputPaletteDivergence> {
    let rom = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<golden>")
        .to_owned();
    let trace = GoldenTrace::read_tsv(path).map_err(|err| error(&rom, 0, err))?;
    let mut latch = LcdOutputLatch::dmg_default().with_latch_perturbation(perturbation);
    if let Some(source) = forced_source {
        latch = latch.with_forced_source(source);
    }

    let rows = output_rows(&trace).map_err(|err| error(&rom, 0, err))?;
    let writes = palette_transition_writes(&rows);
    for write in writes {
        latch.apply_write(write);
    }

    for (index, row) in rows.iter().enumerate() {
        let pixel = OutputRawPixel {
            time: Time::from_subphases(row.raw_tick),
            ly: row.ly,
            x: row.x as usize,
            source: row.source,
            raw_color: row.raw_color,
        };
        let machine = latch
            .latch_pixel(pixel)
            .map_err(|err| error(&rom, index, err))?;
        if machine.source != row.source || machine.sampled_palette_value != row.palette_value {
            return Err(LcdOutputPaletteDivergence {
                rom,
                index,
                raw_tick: row.raw_tick,
                ly: row.ly,
                x: row.x as usize,
                machine_source: machine.source,
                golden_source: row.source,
                raw_color: row.raw_color,
                machine_palette: machine.sampled_palette_value,
                golden_palette: row.palette_value,
                machine_final_color: machine.final_color,
            });
        }
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GoldenOutputRow {
    raw_tick: u64,
    ly: u16,
    x: i32,
    source: LcdPaletteSource,
    raw_color: u8,
    palette_value: u8,
}

fn output_rows(trace: &GoldenTrace) -> Result<Vec<GoldenOutputRow>, String> {
    trace
        .rows
        .iter()
        .filter(|row| row.kind == "pixel")
        .map(output_row)
        .collect()
}

fn output_row(row: &GoldenRow) -> Result<GoldenOutputRow, String> {
    Ok(GoldenOutputRow {
        raw_tick: row.raw_tick,
        ly: row.ly,
        x: row
            .screen_x
            .ok_or_else(|| "pixel row missing screen_x".to_owned())?,
        source: LcdPaletteSource::from_golden(
            row.palette_kind
                .as_deref()
                .ok_or_else(|| "pixel row missing palette_kind".to_owned())?,
        )?,
        raw_color: row
            .raw_color
            .ok_or_else(|| "pixel row missing raw_color".to_owned())?,
        palette_value: row
            .palette_value
            .ok_or_else(|| "pixel row missing palette_value".to_owned())?,
    })
}

fn palette_transition_writes(rows: &[GoldenOutputRow]) -> Vec<PaletteWrite> {
    let mut last = BTreeMap::<LcdPaletteSource, u8>::new();
    let mut writes = Vec::new();
    for row in rows {
        if last.insert(row.source, row.palette_value) != Some(row.palette_value) {
            writes.push(PaletteWrite {
                time: Time::from_subphases(row.raw_tick),
                source: row.source,
                value: row.palette_value,
            });
        }
    }
    writes
}

fn error(rom: &str, index: usize, message: String) -> LcdOutputPaletteDivergence {
    LcdOutputPaletteDivergence {
        rom: rom.to_owned(),
        index,
        raw_tick: 0,
        ly: 0,
        x: 0,
        machine_source: LcdPaletteSource::Bg,
        golden_source: LcdPaletteSource::Bg,
        raw_color: 0,
        machine_palette: 0,
        golden_palette: 0,
        machine_final_color: message.len() as u8,
    }
}
