//! SM83 single-step vector harness (SingleStepTests/sm83 JSON).
//!
//! A CPU-agnostic loader + apply/compare helpers that run the per-opcode JSON
//! vectors against a [`FlatBus`](super::FlatBus). The actual stepping is
//! supplied by the CPU (N2's `step_m`), so this harness exposes a trait the CPU
//! state implements, keeping the loader reusable across the rewrite.
//!
//! Vector shape (observed in `assets/sm83/v1/`):
//! ```json
//! { "name": "...",
//!   "initial": { "pc","sp","a".."l","ime","ie","ram":[[addr,val]] },
//!   "final":   { "pc","sp","a".."l","ime", optional "ie"/"ei", "ram":[[addr,val]] },
//!   "cycles":  [[addr, data, kind], ...] }
//! ```
//! Notably the `final` state usually OMITS `ie`, and EI vectors carry `ei` in
//! `final`. Interrupt fields are therefore optional and parsed faithfully.

use super::FlatBus;

/// Strict parse error — the harness must never silently corrupt a vector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    MissingField(&'static str),
    NotAnObject(&'static str),
    NotAnArray(&'static str),
    ByteOutOfRange { field: &'static str, value: u64 },
    AddrOutOfRange { value: u64 },
    BadRamEntry,
    BadCycleEntry,
}

/// One side of a vector: the CPU registers + RAM bytes. Interrupt fields are
/// optional because `final` states may omit `ie` and EI vectors carry `ei`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VectorState {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: Option<u8>,
    pub ie: Option<u8>,
    /// Present only in EI-instruction `final` states (`"ei": 1`).
    pub ei: Option<u8>,
    /// `(address, value)` RAM entries.
    pub ram: Vec<(u16, u8)>,
}

/// One bus observation from the vector's `cycles` array: `[addr, data, kind]`.
/// `addr`/`data` may be `null` on idle cycles; `kind` is the access tag
/// (e.g. `"r-m"`, `"w-m"`, `"---"`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VectorCycle {
    pub addr: Option<u16>,
    pub data: Option<u8>,
    pub kind: String,
}

/// One full test vector (initial state, expected final state, bus cycles).
#[derive(Clone, Debug)]
pub struct Vector {
    pub name: String,
    pub initial: VectorState,
    pub final_: VectorState,
    pub cycles: Vec<VectorCycle>,
}

/// Anything that can be loaded from / compared against a [`VectorState`].
/// N2's CPU register file implements this so the harness can drive it.
pub trait VectorCpu {
    fn load_state(&mut self, s: &VectorState, bus: &mut FlatBus);
    fn store_state(&self, bus: &FlatBus, ram_addrs: &[u16]) -> VectorState;
}

/// Apply a vector's initial register + RAM state onto a fresh `FlatBus`.
pub fn apply_initial(bus: &mut FlatBus, s: &VectorState) {
    for &(addr, val) in &s.ram {
        bus.poke(addr, val);
    }
    if let Some(ie) = s.ie {
        bus.set_ie(ie);
    }
}

/// Compare the post-run RAM against a vector's expected final RAM. Returns the
/// first mismatch as `(addr, expected, got)`, or `None` if all match.
pub fn check_final_ram(bus: &FlatBus, expected: &VectorState) -> Option<(u16, u8, u8)> {
    for &(addr, val) in &expected.ram {
        let got = bus.peek(addr);
        if got != val {
            return Some((addr, val, got));
        }
    }
    None
}

/// Strictly parse one vector. Any malformed field is a hard error — the harness
/// must never silently truncate a value or skip a RAM entry.
pub fn parse_vector(v: &serde_json::Value) -> Result<Vector, ParseError> {
    let name = v
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or(ParseError::MissingField("name"))?
        .to_string();
    let initial = parse_state(
        v.get("initial")
            .ok_or(ParseError::MissingField("initial"))?,
    )?;
    let final_ = parse_state(v.get("final").ok_or(ParseError::MissingField("final"))?)?;
    // SingleStepTests vectors always carry `cycles`; a missing array is malformed.
    let cycles = parse_cycles(v.get("cycles").ok_or(ParseError::MissingField("cycles"))?)?;
    Ok(Vector {
        name,
        initial,
        final_,
        cycles,
    })
}

fn req_byte(v: &serde_json::Value, field: &'static str) -> Result<u8, ParseError> {
    let n = v
        .get(field)
        .and_then(|x| x.as_u64())
        .ok_or(ParseError::MissingField(field))?;
    if n > 0xFF {
        return Err(ParseError::ByteOutOfRange { field, value: n });
    }
    Ok(n as u8)
}

fn req_u16(v: &serde_json::Value, field: &'static str) -> Result<u16, ParseError> {
    let n = v
        .get(field)
        .and_then(|x| x.as_u64())
        .ok_or(ParseError::MissingField(field))?;
    if n > 0xFFFF {
        return Err(ParseError::AddrOutOfRange { value: n });
    }
    Ok(n as u16)
}

/// Optional byte field: `None` if absent, error if present but out of range.
fn opt_byte(v: &serde_json::Value, field: &'static str) -> Result<Option<u8>, ParseError> {
    match v.get(field) {
        None => Ok(None),
        Some(x) => {
            let n = x.as_u64().ok_or(ParseError::MissingField(field))?;
            if n > 0xFF {
                return Err(ParseError::ByteOutOfRange { field, value: n });
            }
            Ok(Some(n as u8))
        }
    }
}

fn parse_state(v: &serde_json::Value) -> Result<VectorState, ParseError> {
    if !v.is_object() {
        return Err(ParseError::NotAnObject("state"));
    }
    let ram_arr = v
        .get("ram")
        .and_then(|r| r.as_array())
        .ok_or(ParseError::NotAnArray("ram"))?;
    let mut ram = Vec::with_capacity(ram_arr.len());
    for pair in ram_arr {
        let arr = pair.as_array().ok_or(ParseError::BadRamEntry)?;
        if arr.len() != 2 {
            return Err(ParseError::BadRamEntry);
        }
        let addr = arr[0].as_u64().ok_or(ParseError::BadRamEntry)?;
        let val = arr[1].as_u64().ok_or(ParseError::BadRamEntry)?;
        if addr > 0xFFFF {
            return Err(ParseError::AddrOutOfRange { value: addr });
        }
        if val > 0xFF {
            return Err(ParseError::ByteOutOfRange {
                field: "ram",
                value: val,
            });
        }
        ram.push((addr as u16, val as u8));
    }
    Ok(VectorState {
        a: req_byte(v, "a")?,
        b: req_byte(v, "b")?,
        c: req_byte(v, "c")?,
        d: req_byte(v, "d")?,
        e: req_byte(v, "e")?,
        f: req_byte(v, "f")?,
        h: req_byte(v, "h")?,
        l: req_byte(v, "l")?,
        sp: req_u16(v, "sp")?,
        pc: req_u16(v, "pc")?,
        ime: opt_byte(v, "ime")?,
        ie: opt_byte(v, "ie")?,
        ei: opt_byte(v, "ei")?,
        ram,
    })
}

fn parse_cycles(v: &serde_json::Value) -> Result<Vec<VectorCycle>, ParseError> {
    let arr = v.as_array().ok_or(ParseError::NotAnArray("cycles"))?;
    let mut out = Vec::with_capacity(arr.len());
    for entry in arr {
        let e = entry.as_array().ok_or(ParseError::BadCycleEntry)?;
        if e.len() != 3 {
            return Err(ParseError::BadCycleEntry);
        }
        // addr/data are null on idle cycles, a u64 otherwise. Distinguish null
        // (legit) from a wrong-typed value (malformed) — as_u64() alone conflates
        // them, so check is_null() explicitly first.
        let addr = parse_opt_cycle_num(&e[0], 0xFFFF)?.map(|n| n as u16);
        let data = parse_opt_cycle_num(&e[1], 0xFF)?.map(|n| n as u8);
        let kind = e[2].as_str().ok_or(ParseError::BadCycleEntry)?.to_string();
        out.push(VectorCycle { addr, data, kind });
    }
    Ok(out)
}

/// Parse a cycle addr/data slot: JSON `null` -> `None` (legit idle), a u64 in
/// `0..=max` -> `Some`, and any other value (wrong type or out of range) ->
/// error. `as_u64()` alone conflates null with wrong-typed, so check null first.
fn parse_opt_cycle_num(v: &serde_json::Value, max: u64) -> Result<Option<u64>, ParseError> {
    if v.is_null() {
        return Ok(None);
    }
    match v.as_u64() {
        Some(n) if n <= max => Ok(Some(n)),
        _ => Err(ParseError::BadCycleEntry),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset_path(file: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../assets/sm83/v1")
            .join(file)
    }

    fn sample_vector_json() -> serde_json::Value {
        serde_json::json!({
            "name": "18 00A2",
            "initial": {
                "pc": 0xBFFE, "sp": 0x3A86,
                "a": 0xAB, "b": 0x2E, "c": 0x74, "d": 0x48,
                "e": 0x51, "f": 0xF0, "h": 0x9E, "l": 0x23,
                "ime": 1, "ie": 0,
                "ram": [[0xBFFE, 0x18], [0xBFFF, 0xA2]]
            },
            "final": {
                "pc": 0xBFA2, "sp": 0x3A86,
                "a": 0xAB, "b": 0x2E, "c": 0x74, "d": 0x48,
                "e": 0x51, "f": 0xF0, "h": 0x9E, "l": 0x23,
                "ime": 1,
                "ram": [[0xBFFE, 0x18], [0xBFFF, 0x41]]
            },
            "cycles": [[0xBFFE, 0x18, "r-m"], [0xBFFF, 0xA2, "r-m"], [0xBFFF, 0xA2, "---"]]
        })
    }

    #[test]
    fn parses_vector_initial_final_and_cycles() {
        let v = parse_vector(&sample_vector_json()).expect("parses");
        assert_eq!(v.name, "18 00A2");
        assert_eq!(v.initial.pc, 0xBFFE);
        assert_eq!(v.initial.ie, Some(0));
        // final omits ie -> None (must NOT fabricate 0).
        assert_eq!(v.final_.ie, None);
        assert_eq!(v.final_.ime, Some(1));
        assert_eq!(v.initial.ram, vec![(0xBFFE, 0x18), (0xBFFF, 0xA2)]);
        assert_eq!(v.final_.ram, vec![(0xBFFE, 0x18), (0xBFFF, 0x41)]);
        // cycles parsed, not dropped.
        assert_eq!(v.cycles.len(), 3);
        assert_eq!(
            v.cycles[0],
            VectorCycle {
                addr: Some(0xBFFE),
                data: Some(0x18),
                kind: "r-m".into()
            }
        );
        assert_eq!(v.cycles[2].kind, "---");
    }

    #[test]
    fn strict_parse_rejects_oversized_byte() {
        let mut j = sample_vector_json();
        j["initial"]["a"] = serde_json::json!(0x1FF); // > 0xFF
        let err = parse_vector(&j).unwrap_err();
        assert!(matches!(err, ParseError::ByteOutOfRange { field: "a", .. }));
    }

    #[test]
    fn strict_parse_rejects_malformed_ram_pair() {
        let mut j = sample_vector_json();
        j["initial"]["ram"] = serde_json::json!([[0xC000]]); // missing value
        assert_eq!(parse_vector(&j).unwrap_err(), ParseError::BadRamEntry);
    }

    #[test]
    fn apply_initial_sets_ram_including_0xbfff() {
        // The exact bug: initial RAM at 0xBFFF must land in flat RAM.
        let v = parse_vector(&sample_vector_json()).unwrap();
        let mut bus = FlatBus::new();
        apply_initial(&mut bus, &v.initial);
        assert_eq!(bus.peek(0xBFFE), 0x18);
        assert_eq!(bus.peek(0xBFFF), 0xA2, "initial 0xBFFF lands in flat RAM");
    }

    #[test]
    fn check_final_ram_detects_mismatch() {
        let v = parse_vector(&sample_vector_json()).unwrap();
        let mut bus = FlatBus::new();
        apply_initial(&mut bus, &v.initial);
        let mismatch = check_final_ram(&bus, &v.final_);
        assert_eq!(mismatch, Some((0xBFFF, 0x41, 0xA2)));
        bus.poke(0xBFFF, 0x41);
        assert_eq!(check_final_ram(&bus, &v.final_), None);
    }

    #[test]
    fn loads_real_18_json_with_cycles() {
        // Asset MUST exist in this repo; fail loudly if missing.
        let text = std::fs::read_to_string(asset_path("18.json"))
            .expect("assets/sm83/v1/18.json must exist");
        let arr: serde_json::Value = serde_json::from_str(&text).unwrap();
        let first = &arr.as_array().unwrap()[0];
        let v = parse_vector(first).expect("real 18.json vector parses strictly");
        assert!(!v.name.is_empty());
        assert!(!v.initial.ram.is_empty());
        assert!(v.initial.ie.is_some(), "initial has ie");
        assert_eq!(v.final_.ie, None, "final omits ie");
        assert!(!v.cycles.is_empty(), "cycles parsed from real vector");
    }

    #[test]
    fn loads_real_ei_vector_fb_json() {
        // EI vector: final carries "ei": 1, which the parser must capture.
        let text = std::fs::read_to_string(asset_path("fb.json"))
            .expect("assets/sm83/v1/fb.json must exist");
        let arr: serde_json::Value = serde_json::from_str(&text).unwrap();
        let first = &arr.as_array().unwrap()[0];
        let v = parse_vector(first).expect("real fb.json EI vector parses");
        assert_eq!(v.final_.ei, Some(1), "EI final state carries ei=1");
    }

    #[test]
    fn strict_parse_rejects_wrong_typed_cycle_value() {
        // P1: a wrong-typed (non-null, non-numeric) addr/data must NOT pass as
        // None -- it is malformed.
        let mut j = sample_vector_json();
        j["cycles"] = serde_json::json!([["not_addr", "not_data", "r-m"]]);
        assert_eq!(parse_vector(&j).unwrap_err(), ParseError::BadCycleEntry);
    }

    #[test]
    fn null_cycle_addr_data_is_allowed() {
        // A genuine idle cycle has null addr/data -> None (not an error).
        let mut j = sample_vector_json();
        j["cycles"] =
            serde_json::json!([[serde_json::Value::Null, serde_json::Value::Null, "---"]]);
        let v = parse_vector(&j).expect("null idle cycle is valid");
        assert_eq!(v.cycles[0].addr, None);
        assert_eq!(v.cycles[0].data, None);
        assert_eq!(v.cycles[0].kind, "---");
    }

    #[test]
    fn strict_parse_requires_cycles() {
        // P1: SingleStepTests vectors always carry `cycles`; missing is malformed.
        let mut j = sample_vector_json();
        j.as_object_mut().unwrap().remove("cycles");
        assert_eq!(
            parse_vector(&j).unwrap_err(),
            ParseError::MissingField("cycles")
        );
    }
}
