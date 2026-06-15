use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub bead: String,
    pub adr: String,
    pub definition_of_100_percent: String,
    pub models: Vec<String>,
    pub rom_count: usize,
    pub generated_from: Vec<String>,
    pub roms: Vec<RomManifestEntry>,
    pub vector_suites: Vec<VectorSuiteEntry>,
    pub known_unwinnable_by_single_model: Vec<KnownUnwinnableEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RomManifestEntry {
    pub path: String,
    pub suite: String,
    pub intended_models: Vec<String>,
    pub current_old_core_status: String,
    pub notes: String,
    pub expectation: Expectation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VectorSuiteEntry {
    pub path: String,
    pub suite: String,
    pub intended_models: Vec<String>,
    pub current_old_core_status: String,
    pub expectation: Expectation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KnownUnwinnableEntry {
    pub path: String,
    pub intended_models: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expectation {
    pub kind: String,
    pub fields: BTreeMap<String, ManifestValue>,
    pub entries: Vec<BTreeMap<String, ManifestValue>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestValue {
    String(String),
    Integer(i64),
    StringArray(Vec<String>),
}

impl Manifest {
    pub fn read(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path.as_ref()).map_err(|err| err.to_string())?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut manifest = Manifest::default();
        let mut table = Table::None;
        let mut rom: Option<RomManifestEntry> = None;
        let mut vector: Option<VectorSuiteEntry> = None;
        let mut known: Option<KnownUnwinnableEntry> = None;

        for (line_index, raw_line) in text.lines().enumerate() {
            let line_number = line_index + 1;
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }

            match line {
                "[manifest]" => {
                    flush_entry(&mut manifest, &mut rom, &mut vector, &mut known);
                    table = Table::Manifest;
                    continue;
                }
                "[[rom]]" => {
                    flush_entry(&mut manifest, &mut rom, &mut vector, &mut known);
                    rom = Some(RomManifestEntry::default());
                    table = Table::Rom;
                    continue;
                }
                "[rom.expectation]" => {
                    table = Table::RomExpectation;
                    continue;
                }
                "[[rom.expectation.entries]]" => {
                    let rom = rom
                        .as_mut()
                        .ok_or_else(|| format!("line {line_number}: rom entry missing"))?;
                    rom.expectation.entries.push(BTreeMap::new());
                    table = Table::RomExpectationEntry;
                    continue;
                }
                "[[vector_suite]]" => {
                    flush_entry(&mut manifest, &mut rom, &mut vector, &mut known);
                    vector = Some(VectorSuiteEntry::default());
                    table = Table::VectorSuite;
                    continue;
                }
                "[vector_suite.expectation]" => {
                    table = Table::VectorExpectation;
                    continue;
                }
                "[[known_unwinnable_by_single_model]]" => {
                    flush_entry(&mut manifest, &mut rom, &mut vector, &mut known);
                    known = Some(KnownUnwinnableEntry::default());
                    table = Table::KnownUnwinnable;
                    continue;
                }
                _ if line.starts_with('[') => {
                    return Err(format!("line {line_number}: unsupported table {line}"));
                }
                _ => {}
            }

            let (key, value) = split_key_value(line)
                .ok_or_else(|| format!("line {line_number}: expected key = value"))?;
            let value = parse_value(value).map_err(|err| format!("line {line_number}: {err}"))?;
            match table {
                Table::Manifest => assign_manifest(&mut manifest, key, value)?,
                Table::Rom => assign_rom(
                    rom.as_mut()
                        .ok_or_else(|| format!("line {line_number}: rom entry missing"))?,
                    key,
                    value,
                )?,
                Table::RomExpectation => assign_expectation(
                    &mut rom
                        .as_mut()
                        .ok_or_else(|| format!("line {line_number}: rom entry missing"))?
                        .expectation,
                    key,
                    value,
                )?,
                Table::RomExpectationEntry => assign_expectation_entry(
                    rom.as_mut()
                        .ok_or_else(|| format!("line {line_number}: rom entry missing"))?
                        .expectation
                        .entries
                        .last_mut()
                        .ok_or_else(|| format!("line {line_number}: expectation entry missing"))?,
                    key,
                    value,
                ),
                Table::VectorSuite => assign_vector(
                    vector
                        .as_mut()
                        .ok_or_else(|| format!("line {line_number}: vector entry missing"))?,
                    key,
                    value,
                )?,
                Table::VectorExpectation => assign_expectation(
                    &mut vector
                        .as_mut()
                        .ok_or_else(|| format!("line {line_number}: vector entry missing"))?
                        .expectation,
                    key,
                    value,
                )?,
                Table::KnownUnwinnable => assign_known(
                    known
                        .as_mut()
                        .ok_or_else(|| format!("line {line_number}: known entry missing"))?,
                    key,
                    value,
                )?,
                Table::None => return Err(format!("line {line_number}: key before table")),
            }
        }
        flush_entry(&mut manifest, &mut rom, &mut vector, &mut known);

        if manifest.rom_count != 0 && manifest.rom_count != manifest.roms.len() {
            return Err(format!(
                "manifest rom_count {} does not match {} [[rom]] entries",
                manifest.rom_count,
                manifest.roms.len()
            ));
        }
        Ok(manifest)
    }

    pub fn missing_reference_paths(&self, workspace_root: impl AsRef<Path>) -> Vec<PathBuf> {
        let workspace_root = workspace_root.as_ref();
        self.roms
            .iter()
            .map(|rom| workspace_root.join(&rom.path))
            .filter(|path| !path.exists())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Table {
    None,
    Manifest,
    Rom,
    RomExpectation,
    RomExpectationEntry,
    VectorSuite,
    VectorExpectation,
    KnownUnwinnable,
}

fn flush_entry(
    manifest: &mut Manifest,
    rom: &mut Option<RomManifestEntry>,
    vector: &mut Option<VectorSuiteEntry>,
    known: &mut Option<KnownUnwinnableEntry>,
) {
    if let Some(entry) = rom.take() {
        manifest.roms.push(entry);
    }
    if let Some(entry) = vector.take() {
        manifest.vector_suites.push(entry);
    }
    if let Some(entry) = known.take() {
        manifest.known_unwinnable_by_single_model.push(entry);
    }
}

fn strip_comment(line: &str) -> &str {
    line.split_once('#').map_or(line, |(before, _)| before)
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    line.split_once('=')
        .map(|(key, value)| (key.trim(), value.trim()))
}

fn parse_value(value: &str) -> Result<ManifestValue, String> {
    if value.starts_with('"') {
        Ok(ManifestValue::String(parse_string(value)?))
    } else if value.starts_with('[') {
        parse_string_array(value).map(ManifestValue::StringArray)
    } else {
        value
            .parse::<i64>()
            .map(ManifestValue::Integer)
            .map_err(|err| format!("invalid integer {value:?}: {err}"))
    }
}

fn parse_string(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
        return Err(format!("invalid string literal {value:?}"));
    }
    Ok(value[1..value.len() - 1].replace("\\\"", "\""))
}

fn parse_string_array(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err(format!("invalid array literal {value:?}"));
    }
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|item| parse_string(item.trim()))
        .collect()
}

fn expect_string(value: ManifestValue, key: &str) -> Result<String, String> {
    match value {
        ManifestValue::String(value) => Ok(value),
        _ => Err(format!("{key} must be a string")),
    }
}

fn expect_string_array(value: ManifestValue, key: &str) -> Result<Vec<String>, String> {
    match value {
        ManifestValue::StringArray(value) => Ok(value),
        _ => Err(format!("{key} must be a string array")),
    }
}

fn expect_usize(value: ManifestValue, key: &str) -> Result<usize, String> {
    match value {
        ManifestValue::Integer(value) if value >= 0 => Ok(value as usize),
        _ => Err(format!("{key} must be a non-negative integer")),
    }
}

fn assign_manifest(manifest: &mut Manifest, key: &str, value: ManifestValue) -> Result<(), String> {
    match key {
        "version" => manifest.version = expect_usize(value, key)? as u32,
        "bead" => manifest.bead = expect_string(value, key)?,
        "adr" => manifest.adr = expect_string(value, key)?,
        "definition_of_100_percent" => {
            manifest.definition_of_100_percent = expect_string(value, key)?;
        }
        "models" => manifest.models = expect_string_array(value, key)?,
        "rom_count" => manifest.rom_count = expect_usize(value, key)?,
        "generated_from" => manifest.generated_from = expect_string_array(value, key)?,
        _ => return Err(format!("unsupported manifest key {key}")),
    }
    Ok(())
}

fn assign_rom(rom: &mut RomManifestEntry, key: &str, value: ManifestValue) -> Result<(), String> {
    match key {
        "path" => rom.path = expect_string(value, key)?,
        "suite" => rom.suite = expect_string(value, key)?,
        "intended_models" => rom.intended_models = expect_string_array(value, key)?,
        "current_old_core_status" => rom.current_old_core_status = expect_string(value, key)?,
        "notes" => rom.notes = expect_string(value, key)?,
        _ => return Err(format!("unsupported rom key {key}")),
    }
    Ok(())
}

fn assign_vector(
    vector: &mut VectorSuiteEntry,
    key: &str,
    value: ManifestValue,
) -> Result<(), String> {
    match key {
        "path" => vector.path = expect_string(value, key)?,
        "suite" => vector.suite = expect_string(value, key)?,
        "intended_models" => vector.intended_models = expect_string_array(value, key)?,
        "current_old_core_status" => vector.current_old_core_status = expect_string(value, key)?,
        _ => return Err(format!("unsupported vector_suite key {key}")),
    }
    Ok(())
}

fn assign_known(
    known: &mut KnownUnwinnableEntry,
    key: &str,
    value: ManifestValue,
) -> Result<(), String> {
    match key {
        "path" => known.path = expect_string(value, key)?,
        "intended_models" => known.intended_models = expect_string_array(value, key)?,
        "note" => known.note = expect_string(value, key)?,
        _ => return Err(format!("unsupported known_unwinnable key {key}")),
    }
    Ok(())
}

fn assign_expectation(
    expectation: &mut Expectation,
    key: &str,
    value: ManifestValue,
) -> Result<(), String> {
    if key == "kind" {
        expectation.kind = expect_string(value, key)?;
    } else {
        expectation.fields.insert(key.to_owned(), value);
    }
    Ok(())
}

fn assign_expectation_entry(
    entry: &mut BTreeMap<String, ManifestValue>,
    key: &str,
    value: ManifestValue,
) {
    entry.insert(key.to_owned(), value);
}
