use std::fs;
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
