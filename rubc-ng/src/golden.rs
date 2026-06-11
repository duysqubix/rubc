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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldenInitialState {
    pub frame: u64,
    pub raw_tick: u64,
    pub ly: u16,
    pub line_tick: u64,
    pub mode: u8,
    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub lyc: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenVramState {
    pub regs: GoldenVramRegisters,
    pub vram: Vram,
    pub oam: [u8; 0xA0],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldenVramRegisters {
    pub lcdc: u8,
    pub scx: u8,
    pub scy: u8,
    pub wx: u8,
    pub wy: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vram {
    pub bank0: [u8; 0x2000],
    pub bank1: [u8; 0x2000],
}

impl Vram {
    pub fn read(&self, addr: u16, bank: u8) -> Result<u8, String> {
        let offset = usize::from(addr);
        if offset >= 0x2000 {
            return Err(format!("VRAM offset out of range: {addr:04X}"));
        }
        match bank {
            0 => Ok(self.bank0[offset]),
            1 => Ok(self.bank1[offset]),
            _ => Err(format!("unsupported VRAM bank: {bank}")),
        }
    }

    pub fn write_for_test(&mut self, addr: u16, bank: u8, value: u8) -> Result<(), String> {
        let offset = usize::from(addr);
        if offset >= 0x2000 {
            return Err(format!("VRAM offset out of range: {addr:04X}"));
        }
        match bank {
            0 => self.bank0[offset] = value,
            1 => self.bank1[offset] = value,
            _ => return Err(format!("unsupported VRAM bank: {bank}")),
        }
        Ok(())
    }
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

    pub fn read_initial_state(path: impl AsRef<Path>) -> Result<GoldenInitialState, String> {
        let mut state = InitialStateBuilder::default();
        for row in Self::open(path)? {
            let row = row?;
            if row.kind == "initial_state" {
                state.push(row)?;
            } else if state.started {
                break;
            }
        }
        state.finish()
    }

    pub fn read_vram_state(path: impl AsRef<Path>) -> Result<GoldenVramState, String> {
        let mut state = VramStateBuilder::default();
        for row in Self::open(path)? {
            let row = row?;
            if row.kind == "vram_state" {
                state.push(row)?;
            } else if state.started {
                break;
            }
        }
        state.finish()
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
        if columns != V2_COLUMNS && columns != V21_COLUMNS && columns != V23_COLUMNS {
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

#[derive(Default)]
struct InitialStateBuilder {
    started: bool,
    frame: Option<u64>,
    raw_tick: Option<u64>,
    ly: Option<u16>,
    line_tick: Option<u64>,
    mode: Option<u8>,
    lcdc: Option<u8>,
    stat: Option<u8>,
    scy: Option<u8>,
    scx: Option<u8>,
    lyc: Option<u8>,
}

#[derive(Default)]
struct VramStateBuilder {
    started: bool,
    regs: Option<GoldenVramRegisters>,
    bank0: Option<[u8; 0x2000]>,
    bank1: Option<[u8; 0x2000]>,
    oam: Option<[u8; 0xA0]>,
}

impl VramStateBuilder {
    fn push(&mut self, row: GoldenV2Row) -> Result<(), String> {
        self.started = true;
        let block = row
            .state_block
            .as_deref()
            .ok_or_else(|| "vram_state row missing state_block".to_owned())?;
        let len = row
            .state_len
            .ok_or_else(|| "vram_state row missing state_len".to_owned())?;
        let data = decode_hex_bytes(
            row.state_data
                .as_deref()
                .ok_or_else(|| "vram_state row missing state_data".to_owned())?,
            block,
        )?;
        if data.len() != len {
            return Err(format!(
                "vram_state {block} length mismatch: header {len}, data {}",
                data.len()
            ));
        }
        match block {
            "registers_lcdc_scx_scy_wx_wy" => {
                if data.len() != 5 {
                    return Err(format!(
                        "register block expected 5 bytes, got {}",
                        data.len()
                    ));
                }
                set_once_value(
                    &mut self.regs,
                    GoldenVramRegisters {
                        lcdc: data[0],
                        scx: data[1],
                        scy: data[2],
                        wx: data[3],
                        wy: data[4],
                    },
                    "vram registers",
                )
            }
            "oam" => {
                let bytes: [u8; 0xA0] = data.try_into().map_err(|data: Vec<u8>| {
                    format!("OAM expected 160 bytes, got {}", data.len())
                })?;
                set_once_value(&mut self.oam, bytes, "OAM")
            }
            "vram" => {
                let bytes: [u8; 0x2000] = data.try_into().map_err(|data: Vec<u8>| {
                    format!("VRAM bank expected 8192 bytes, got {}", data.len())
                })?;
                match row.vram_bank {
                    Some(0) => set_once_value(&mut self.bank0, bytes, "VRAM bank0"),
                    Some(1) => set_once_value(&mut self.bank1, bytes, "VRAM bank1"),
                    bank => Err(format!(
                        "vram_state vram row has unsupported bank: {bank:?}"
                    )),
                }
            }
            _ => Err(format!("unsupported vram_state block: {block}")),
        }
    }

    fn finish(self) -> Result<GoldenVramState, String> {
        Ok(GoldenVramState {
            regs: self
                .regs
                .ok_or_else(|| "missing v2.3 VRAM register state".to_owned())?,
            vram: Vram {
                bank0: self
                    .bank0
                    .ok_or_else(|| "missing v2.3 VRAM bank0 state".to_owned())?,
                bank1: self.bank1.unwrap_or([0; 0x2000]),
            },
            oam: self
                .oam
                .ok_or_else(|| "missing v2.3 OAM state".to_owned())?,
        })
    }
}

impl InitialStateBuilder {
    fn push(&mut self, row: GoldenV2Row) -> Result<(), String> {
        self.started = true;
        let ly = row
            .ly
            .ok_or_else(|| "initial_state row missing LY".to_owned())?;
        let line_tick = row
            .line_tick
            .ok_or_else(|| "initial_state row missing line_tick".to_owned())?;
        let mode = row
            .mode
            .ok_or_else(|| "initial_state row missing mode".to_owned())?;
        match (
            self.frame,
            self.raw_tick,
            self.ly,
            self.line_tick,
            self.mode,
        ) {
            (
                Some(frame),
                Some(raw_tick),
                Some(existing_ly),
                Some(existing_tick),
                Some(existing_mode),
            ) if frame == row.frame
                && raw_tick == row.raw_tick
                && existing_ly == ly
                && existing_tick == line_tick
                && existing_mode == mode => {}
            (None, None, None, None, None) => {
                self.frame = Some(row.frame);
                self.raw_tick = Some(row.raw_tick);
                self.ly = Some(ly);
                self.line_tick = Some(line_tick);
                self.mode = Some(mode);
            }
            _ => return Err("initial_state rows disagree on capture-window position".to_owned()),
        }

        let addr = row
            .addr
            .ok_or_else(|| "initial_state row missing addr".to_owned())?;
        let byte = row
            .byte
            .ok_or_else(|| "initial_state row missing byte".to_owned())?;
        match addr {
            0xFF40 => set_once(&mut self.lcdc, byte, "FF40"),
            0xFF41 => set_once(&mut self.stat, byte, "FF41"),
            0xFF42 => set_once(&mut self.scy, byte, "FF42"),
            0xFF43 => set_once(&mut self.scx, byte, "FF43"),
            0xFF45 => set_once(&mut self.lyc, byte, "FF45"),
            _ => Err(format!("unexpected initial_state addr: {addr:04X}")),
        }
    }

    fn finish(self) -> Result<GoldenInitialState, String> {
        Ok(GoldenInitialState {
            frame: self
                .frame
                .ok_or_else(|| "missing initial_state block".to_owned())?,
            raw_tick: self.raw_tick.expect("frame presence implies raw_tick"),
            ly: self.ly.expect("frame presence implies ly"),
            line_tick: self.line_tick.expect("frame presence implies line_tick"),
            mode: self.mode.expect("frame presence implies mode"),
            lcdc: self.lcdc.ok_or_else(|| "missing initial FF40".to_owned())?,
            stat: self.stat.ok_or_else(|| "missing initial FF41".to_owned())?,
            scy: self.scy.ok_or_else(|| "missing initial FF42".to_owned())?,
            scx: self.scx.ok_or_else(|| "missing initial FF43".to_owned())?,
            lyc: self.lyc.ok_or_else(|| "missing initial FF45".to_owned())?,
        })
    }
}

fn set_once(slot: &mut Option<u8>, value: u8, name: &str) -> Result<(), String> {
    match slot {
        Some(existing) if *existing == value => Ok(()),
        Some(existing) => Err(format!(
            "duplicate initial {name} disagrees: {existing:02X} vs {value:02X}"
        )),
        None => {
            *slot = Some(value);
            Ok(())
        }
    }
}

fn set_once_value<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String>
where
    T: PartialEq,
{
    match slot {
        Some(existing) if *existing == value => Ok(()),
        Some(_) => Err(format!("duplicate {name} disagrees")),
        None => {
            *slot = Some(value);
            Ok(())
        }
    }
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

const V23_COLUMNS: [&str; 41] = [
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
    "state_block",
    "vram_bank",
    "state_len",
    "state_data",
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
    pub model: Option<u16>,
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
    pub state_block: Option<String>,
    pub vram_bank: Option<i16>,
    pub state_len: Option<usize>,
    pub state_data: Option<String>,
}

impl GoldenV2Row {
    fn parse(line: &str) -> Result<Self, String> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != V2_COLUMNS.len()
            && fields.len() != V21_COLUMNS.len()
            && fields.len() != V23_COLUMNS.len()
        {
            return Err(format!(
                "expected {}, {}, or {} fields, got {}",
                V2_COLUMNS.len(),
                V21_COLUMNS.len(),
                V23_COLUMNS.len(),
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
            state_block: fields.get(37).and_then(|field| opt_string(field)),
            vram_bank: fields
                .get(38)
                .map_or(Ok(None), |field| parse_opt_dec(field, "vram_bank"))?,
            state_len: fields
                .get(39)
                .map_or(Ok(None), |field| parse_opt_dec(field, "state_len"))?,
            state_data: fields.get(40).and_then(|field| opt_string(field)),
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

fn decode_hex_bytes(hex: &str, name: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err(format!("{name} hex length is odd: {}", hex.len()));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let raw = hex.as_bytes();
    for chunk in raw.chunks_exact(2) {
        let high = hex_nibble(chunk[0], name)?;
        let low = hex_nibble(chunk[1], name)?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8, name: &str) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid {name} hex digit: {}", byte as char)),
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
