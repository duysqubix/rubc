use crate::time::Time;
use crate::timing::Observable;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::ops::RangeInclusive;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct GoldenTrace {
    pub rows: Vec<GoldenRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GoldenRow {
    pub kind: String,
    pub frame: u64,
    pub raw_tick: u64,
    pub ly: u16,
    pub line_tick: u64,
    pub dot: f64,
    pub state: Option<String>,
    pub x: Option<i32>,
    pub screen_x: Option<i32>,
    pub addr: Option<u16>,
    pub byte: Option<u8>,
    pub raw_color: Option<u8>,
    pub palette_kind: Option<String>,
    pub palette_reg: Option<String>,
    pub palette_value: Option<u8>,
    pub io_scy: Option<u8>,
    pub io_lcdc: Option<u8>,
    pub io_wx: Option<u8>,
    pub pos: Option<i32>,
    pub conflict: Option<String>,
    pub norm_dot: Option<f64>,
}

impl GoldenTrace {
    pub fn read_tsv(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path.as_ref()).map_err(|err| err.to_string())?;
        Self::from_tsv_str(&text)
    }

    pub fn from_tsv_str(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header = lines
            .next()
            .ok_or_else(|| "missing TSV header".to_owned())?;
        let expected = [
            "kind",
            "frame",
            "raw_tick",
            "ly",
            "line_tick",
            "dot",
            "state",
            "x",
            "screen_x",
            "addr",
            "byte",
            "raw_color",
            "palette_kind",
            "palette_reg",
            "palette_value",
            "io_scy",
            "io_lcdc",
            "io_wx",
            "pos",
            "conflict",
            "norm_dot",
        ];
        let columns: Vec<_> = header.split('\t').collect();
        if columns != expected {
            return Err(format!("unexpected golden TSV header: {header}"));
        }

        let mut rows = Vec::new();
        for (index, line) in lines.enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            rows.push(GoldenRow::parse(line).map_err(|err| format!("row {}: {err}", index + 2))?);
        }

        Ok(Self { rows })
    }
}

impl GoldenRow {
    fn parse(line: &str) -> Result<Self, String> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 21 {
            return Err(format!("expected 21 fields, got {}", fields.len()));
        }

        Ok(Self {
            kind: req_string(fields[0], "kind")?,
            frame: parse_dec(fields[1], "frame")?,
            raw_tick: parse_dec(fields[2], "raw_tick")?,
            ly: parse_dec(fields[3], "ly")?,
            line_tick: parse_dec(fields[4], "line_tick")?,
            dot: parse_float(fields[5], "dot")?,
            state: opt_string(fields[6]),
            x: parse_opt_dec(fields[7], "x")?,
            screen_x: parse_opt_dec(fields[8], "screen_x")?,
            addr: parse_opt_hex_u16(fields[9], "addr")?,
            byte: parse_opt_hex_u8(fields[10], "byte")?,
            raw_color: parse_opt_hex_u8(fields[11], "raw_color")?,
            palette_kind: opt_string(fields[12]),
            palette_reg: opt_string(fields[13]),
            palette_value: parse_opt_hex_u8(fields[14], "palette_value")?,
            io_scy: parse_opt_hex_u8(fields[15], "io_scy")?,
            io_lcdc: parse_opt_hex_u8(fields[16], "io_lcdc")?,
            io_wx: parse_opt_hex_u8(fields[17], "io_wx")?,
            pos: parse_opt_dec(fields[18], "pos")?,
            conflict: opt_string(fields[19]),
            norm_dot: parse_opt_float(fields[20], "norm_dot")?,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoldenSelection {
    kind: Option<String>,
    event: Option<String>,
    frames: Option<RangeInclusive<u64>>,
    lines: Option<RangeInclusive<u16>>,
    limit: Option<usize>,
}

impl GoldenSelection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    pub fn event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    pub fn frames(mut self, frames: RangeInclusive<u64>) -> Self {
        self.frames = Some(frames);
        self
    }

    pub fn lines(mut self, lines: RangeInclusive<u16>) -> Self {
        self.lines = Some(lines);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    fn accepts(&self, row: &GoldenV2Row) -> bool {
        self.kind.as_ref().is_none_or(|kind| row.kind == *kind)
            && self
                .event
                .as_ref()
                .is_none_or(|event| row.event.as_deref() == Some(event.as_str()))
            && self
                .frames
                .as_ref()
                .is_none_or(|frames| frames.contains(&row.frame))
            && self
                .lines
                .as_ref()
                .is_none_or(|lines| row.ly.is_some_and(|ly| lines.contains(&ly)))
    }
}

#[derive(Debug)]
pub struct GoldenV2Reader<R: BufRead = BufReader<File>> {
    lines: Lines<R>,
    columns: Vec<String>,
    next_line_number: usize,
}

impl GoldenV2Reader<BufReader<File>> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let file = File::open(path.as_ref()).map_err(|err| err.to_string())?;
        Self::new(BufReader::new(file))
    }
}

impl<R: BufRead> GoldenV2Reader<R> {
    pub fn new(reader: R) -> Result<Self, String> {
        let mut lines = reader.lines();
        let header = lines
            .next()
            .ok_or_else(|| "missing v2 TSV header".to_owned())?
            .map_err(|err| err.to_string())?;
        let columns: Vec<String> = header.split('\t').map(str::to_owned).collect();
        if columns != V2_COLUMNS && columns != V21_COLUMNS {
            return Err(format!("unexpected v2 golden TSV header: {header}"));
        }
        Ok(Self {
            lines,
            columns,
            next_line_number: 2,
        })
    }

    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    pub fn filter_selection(self, selection: GoldenSelection) -> GoldenV2Selection<R> {
        GoldenV2Selection {
            reader: self,
            selection,
            yielded: 0,
        }
    }
}

impl<R: BufRead> Iterator for GoldenV2Reader<R> {
    type Item = Result<GoldenV2Row, String>;

    fn next(&mut self) -> Option<Self::Item> {
        for line in self.lines.by_ref() {
            let line_number = self.next_line_number;
            self.next_line_number += 1;
            match line {
                Ok(line) if line.trim().is_empty() => continue,
                Ok(line) => {
                    return Some(
                        GoldenV2Row::parse(&line)
                            .map_err(|err| format!("row {line_number}: {err}")),
                    );
                }
                Err(err) => return Some(Err(err.to_string())),
            }
        }
        None
    }
}

pub struct GoldenV2Selection<R: BufRead> {
    reader: GoldenV2Reader<R>,
    selection: GoldenSelection,
    yielded: usize,
}

impl<R: BufRead> Iterator for GoldenV2Selection<R> {
    type Item = Result<GoldenV2Row, String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self
            .selection
            .limit
            .is_some_and(|limit| self.yielded >= limit)
        {
            return None;
        }
        for row in self.reader.by_ref() {
            match row {
                Ok(row) if self.selection.accepts(&row) => {
                    self.yielded += 1;
                    return Some(Ok(row));
                }
                Ok(_) => continue,
                Err(err) => return Some(Err(err)),
            }
        }
        None
    }
}

const V2_COLUMNS: [&str; 35] = [
    "schema",
    "kind",
    "frame",
    "raw_tick",
    "ly",
    "line_tick",
    "dot",
    "event",
    "mode",
    "stat",
    "stat_sources",
    "if",
    "ie",
    "lyc",
    "line_dot",
    "lcd_on",
    "irq_edge",
    "model",
    "double_speed",
    "state",
    "x",
    "screen_x",
    "addr",
    "byte",
    "raw_color",
    "palette_kind",
    "palette_reg",
    "palette_value",
    "io_scy",
    "io_scx",
    "io_lcdc",
    "io_wx",
    "io_wy",
    "pos",
    "conflict",
];

const V21_COLUMNS: [&str; 37] = [
    "schema",
    "kind",
    "frame",
    "raw_tick",
    "ly",
    "line_tick",
    "dot",
    "event",
    "mode",
    "stat",
    "stat_sources",
    "if",
    "ie",
    "lyc",
    "line_dot",
    "lcd_on",
    "irq_edge",
    "model",
    "double_speed",
    "state",
    "x",
    "screen_x",
    "addr",
    "byte",
    "raw_color",
    "palette_kind",
    "palette_reg",
    "palette_value",
    "io_scy",
    "io_scx",
    "io_lcdc",
    "io_wx",
    "io_wy",
    "pos",
    "conflict",
    "write_visible_tick",
    "write_visible_dot",
];

#[derive(Clone, Debug, PartialEq)]
pub struct GoldenV2Row {
    pub schema: u8,
    pub kind: String,
    pub frame: u64,
    pub raw_tick: u64,
    pub ly: Option<u16>,
    pub line_tick: Option<u64>,
    pub dot: Option<f64>,
    pub event: Option<String>,
    pub mode: Option<u8>,
    pub stat: Option<u8>,
    pub stat_sources: Option<String>,
    pub interrupt_flag: Option<u8>,
    pub interrupt_enable: Option<u8>,
    pub lyc: Option<u8>,
    pub line_dot: Option<f64>,
    pub lcd_on: Option<bool>,
    pub irq_edge: Option<bool>,
    pub model: Option<u8>,
    pub double_speed: Option<bool>,
    pub state: Option<String>,
    pub x: Option<i32>,
    pub screen_x: Option<i32>,
    pub addr: Option<u16>,
    pub byte: Option<u8>,
    pub raw_color: Option<u8>,
    pub palette_kind: Option<String>,
    pub palette_reg: Option<String>,
    pub palette_value: Option<u8>,
    pub io_scy: Option<u8>,
    pub io_scx: Option<u8>,
    pub io_lcdc: Option<u8>,
    pub io_wx: Option<u8>,
    pub io_wy: Option<u8>,
    pub pos: Option<i32>,
    pub conflict: Option<String>,
    pub write_visible_tick: Option<u64>,
    pub write_visible_dot: Option<f64>,
}

impl GoldenV2Row {
    fn parse(line: &str) -> Result<Self, String> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != V2_COLUMNS.len() && fields.len() != V21_COLUMNS.len() {
            return Err(format!(
                "expected {} or {} fields, got {}",
                V2_COLUMNS.len(),
                V21_COLUMNS.len(),
                fields.len()
            ));
        }
        let schema = parse_dec(fields[0], "schema")?;
        if schema != 2 {
            return Err(format!("unsupported schema: {schema}"));
        }
        Ok(Self {
            schema,
            kind: req_string(fields[1], "kind")?,
            frame: parse_dec(fields[2], "frame")?,
            raw_tick: parse_dec(fields[3], "raw_tick")?,
            ly: parse_opt_dec(fields[4], "ly")?,
            line_tick: parse_opt_nonnegative_i64_as_u64(fields[5], "line_tick")?,
            dot: parse_opt_float(fields[6], "dot")?,
            event: opt_string(fields[7]),
            mode: parse_opt_dec(fields[8], "mode")?,
            stat: parse_opt_hex_u8(fields[9], "stat")?,
            stat_sources: opt_string(fields[10]),
            interrupt_flag: parse_opt_hex_u8(fields[11], "if")?,
            interrupt_enable: parse_opt_hex_u8(fields[12], "ie")?,
            lyc: parse_opt_hex_u8(fields[13], "lyc")?,
            line_dot: parse_opt_float(fields[14], "line_dot")?,
            lcd_on: parse_opt_bool_u8(fields[15], "lcd_on")?,
            irq_edge: parse_opt_bool_u8(fields[16], "irq_edge")?,
            model: parse_opt_dec(fields[17], "model")?,
            double_speed: parse_opt_bool_u8(fields[18], "double_speed")?,
            state: opt_string(fields[19]),
            x: parse_opt_dec(fields[20], "x")?,
            screen_x: parse_opt_dec(fields[21], "screen_x")?,
            addr: parse_opt_hex_u16(fields[22], "addr")?,
            byte: parse_opt_hex_u8(fields[23], "byte")?,
            raw_color: parse_opt_hex_u8(fields[24], "raw_color")?,
            palette_kind: opt_string(fields[25]),
            palette_reg: opt_string(fields[26]),
            palette_value: parse_opt_hex_u8(fields[27], "palette_value")?,
            io_scy: parse_opt_hex_u8(fields[28], "io_scy")?,
            io_scx: parse_opt_hex_u8(fields[29], "io_scx")?,
            io_lcdc: parse_opt_hex_u8(fields[30], "io_lcdc")?,
            io_wx: parse_opt_hex_u8(fields[31], "io_wx")?,
            io_wy: parse_opt_hex_u8(fields[32], "io_wy")?,
            pos: parse_opt_dec(fields[33], "pos")?,
            conflict: opt_string(fields[34]),
            write_visible_tick: fields
                .get(35)
                .map_or(Ok(None), |field| parse_opt_dec(field, "write_visible_tick"))?,
            write_visible_dot: fields.get(36).map_or(Ok(None), |field| {
                parse_opt_float(field, "write_visible_dot")
            })?,
        })
    }

    pub fn to_observable_sample(&self, observable: Observable) -> Option<ObservableSample> {
        let value = match observable {
            Observable::PpuLy => self.ly.map(ObservableValue::U16),
            Observable::PpuModeEdge => self.mode.map(ObservableValue::U8),
            Observable::PpuStat => self.stat.map(ObservableValue::U8),
            Observable::PpuStatSources => self.stat_sources.clone().map(ObservableValue::Text),
            Observable::PpuIrqEdge => self.irq_edge.map(ObservableValue::Bool),
            Observable::PpuLcdOn => self.lcd_on.map(ObservableValue::Bool),
            Observable::PpuLyc => self.lyc.map(ObservableValue::U8),
            Observable::PpuMemoryLock => self.stat_sources.clone().map(ObservableValue::Text),
            Observable::OutputPixelLatch => self.raw_color.map(ObservableValue::U8),
            Observable::CpuReadSample | Observable::CpuWriteDrive | Observable::BusConflict => {
                self.byte.map(ObservableValue::U8)
            }
            Observable::TimerEdge | Observable::DmaBeat | Observable::BootRomExit => {
                self.event.clone().map(ObservableValue::Text)
            }
            Observable::CpuIdle | Observable::CpuIntrPoll | Observable::PpuFetchSample => {
                self.state.clone().map(ObservableValue::Text)
            }
        }?;
        Some(ObservableSample {
            time: Time::from_subphases(self.raw_tick),
            observable,
            value,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservableSample {
    pub time: Time,
    pub observable: Observable,
    pub value: ObservableValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservableValue {
    Bool(bool),
    U8(u8),
    U16(u16),
    U64(u64),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceMismatch {
    rom: String,
    observable: Observable,
    index: usize,
    expected: Option<ObservableSample>,
    actual: Option<ObservableSample>,
}

impl fmt::Display for TraceMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "first divergence for {} {:?} at selected edge #{}: expected {:?}, actual {:?}",
            self.rom, self.observable, self.index, self.expected, self.actual
        )
    }
}

impl std::error::Error for TraceMismatch {}

pub fn assert_golden_edges<A, R>(
    actual: A,
    golden: GoldenV2Reader<R>,
    observable: Observable,
    rom: impl Into<String>,
    selection: GoldenSelection,
) -> Result<(), TraceMismatch>
where
    A: IntoIterator<Item = ObservableSample>,
    R: BufRead,
{
    let rom = rom.into();
    let mut actual = actual.into_iter();
    let mut expected = golden.filter_selection(selection).filter_map(|row| {
        row.ok()
            .and_then(|row| row.to_observable_sample(observable))
    });

    let mut index = 0;
    loop {
        let expected_sample = expected.next();
        let actual_sample = actual.next();
        if expected_sample.is_none() && actual_sample.is_none() {
            return Ok(());
        }
        if expected_sample != actual_sample {
            return Err(TraceMismatch {
                rom,
                observable,
                index,
                expected: expected_sample,
                actual: actual_sample,
            });
        }
        index += 1;
    }
}

#[macro_export]
macro_rules! assert_golden_edges {
    ($actual:expr, $golden:expr, $observable:expr, $rom:expr, $selection:expr $(,)?) => {{
        if let Err(err) =
            $crate::golden::assert_golden_edges($actual, $golden, $observable, $rom, $selection)
        {
            panic!("{err}");
        }
    }};
}

fn req_string(field: &str, name: &str) -> Result<String, String> {
    if field.is_empty() {
        Err(format!("{name} is empty"))
    } else {
        Ok(field.to_owned())
    }
}

fn opt_string(field: &str) -> Option<String> {
    (!field.is_empty()).then(|| field.to_owned())
}

fn parse_dec<T>(field: &str, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    field
        .parse::<T>()
        .map_err(|err| format!("invalid {name}: {err}"))
}

fn parse_opt_dec<T>(field: &str, name: &str) -> Result<Option<T>, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    if field.is_empty() {
        Ok(None)
    } else {
        parse_dec(field, name).map(Some)
    }
}

fn parse_float(field: &str, name: &str) -> Result<f64, String> {
    parse_dec(field, name)
}

fn parse_opt_float(field: &str, name: &str) -> Result<Option<f64>, String> {
    parse_opt_dec(field, name)
}

fn parse_opt_nonnegative_i64_as_u64(field: &str, name: &str) -> Result<Option<u64>, String> {
    match parse_opt_dec::<i64>(field, name)? {
        Some(value) if value >= 0 => Ok(Some(value as u64)),
        Some(_) => Ok(None),
        None => Ok(None),
    }
}

fn parse_opt_hex_u8(field: &str, name: &str) -> Result<Option<u8>, String> {
    if field.is_empty() {
        Ok(None)
    } else {
        u8::from_str_radix(field, 16)
            .map(Some)
            .map_err(|err| format!("invalid {name}: {err}"))
    }
}

fn parse_opt_hex_u16(field: &str, name: &str) -> Result<Option<u16>, String> {
    if field.is_empty() {
        Ok(None)
    } else {
        u16::from_str_radix(field, 16)
            .map(Some)
            .map_err(|err| format!("invalid {name}: {err}"))
    }
}

fn parse_opt_bool_u8(field: &str, name: &str) -> Result<Option<bool>, String> {
    parse_opt_dec::<u8>(field, name).map(|value| value.map(|value| value != 0))
}
