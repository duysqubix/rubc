//! Structured anomaly taxonomy.
//!
//! Anything the emulator can self-detect as impossible or suspicious goes
//! through one path: [`AnomalyEvent`] -> append to `anomalies.jsonl`. This
//! prevents "silent wrong-pixel" failures for conditions the machine can
//! actually know are wrong. Genuinely visual/audio wrongness is still caught
//! by the trace/hash/snapshot artifacts.

use std::io::Write;
use std::path::Path;

/// How serious an anomaly is. `Error`/`Fatal` trigger an artifact dump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
    Fatal,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Error => "error",
            Severity::Fatal => "fatal",
        }
    }

    /// Error and Fatal warrant a full artifact dump.
    pub fn dumps(self) -> bool {
        matches!(self, Severity::Error | Severity::Fatal)
    }
}

/// The closed set of self-detectable anomalies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnomalyKind {
    IllegalOpcode {
        opcode: u8,
        pc: u16,
    },
    StuckCpu {
        pc: u16,
        repeated: u64,
    },
    InfiniteLoopHeuristic {
        pc: u16,
        iterations: u64,
    },
    UnmappedRead {
        addr: u16,
    },
    UnmappedWrite {
        addr: u16,
        value: u8,
    },
    BusInvariantViolation {
        detail: &'static str,
    },
    PpuModeTimingImpossible {
        ly: u8,
        mode: u8,
        dot: u16,
        detail: &'static str,
    },
    InterruptInvariantViolation {
        ie: u8,
        if_: u8,
        ime: bool,
        detail: &'static str,
    },
    StateHashDivergence {
        expected: u64,
        actual: u64,
        frame: u64,
        mcycle: u64,
    },
    AssertionFailure {
        subsystem: &'static str,
        detail: String,
    },
}

impl AnomalyKind {
    pub fn tag(&self) -> &'static str {
        match self {
            AnomalyKind::IllegalOpcode { .. } => "IllegalOpcode",
            AnomalyKind::StuckCpu { .. } => "StuckCpu",
            AnomalyKind::InfiniteLoopHeuristic { .. } => "InfiniteLoopHeuristic",
            AnomalyKind::UnmappedRead { .. } => "UnmappedRead",
            AnomalyKind::UnmappedWrite { .. } => "UnmappedWrite",
            AnomalyKind::BusInvariantViolation { .. } => "BusInvariantViolation",
            AnomalyKind::PpuModeTimingImpossible { .. } => "PpuModeTimingImpossible",
            AnomalyKind::InterruptInvariantViolation { .. } => "InterruptInvariantViolation",
            AnomalyKind::StateHashDivergence { .. } => "StateHashDivergence",
            AnomalyKind::AssertionFailure { .. } => "AssertionFailure",
        }
    }

    /// JSON for fields NOT already in the envelope (pc/opcode/ly/ppu_mode/ie/if).
    /// Returns an empty string when the kind adds nothing new, so the caller
    /// can avoid emitting a stray comma or duplicate key.
    fn extra_fields_json(&self) -> String {
        match self {
            // opcode + pc are both in the envelope; nothing extra.
            AnomalyKind::IllegalOpcode { .. } => String::new(),
            // pc is in the envelope; only `repeated` is new.
            AnomalyKind::StuckCpu { repeated, .. } => format!("\"repeated\":{repeated}"),
            AnomalyKind::InfiniteLoopHeuristic { iterations, .. } => {
                format!("\"iterations\":{iterations}")
            }
            AnomalyKind::UnmappedRead { addr } => format!("\"addr\":{addr}"),
            AnomalyKind::UnmappedWrite { addr, value } => {
                format!("\"addr\":{addr},\"value\":{value}")
            }
            AnomalyKind::BusInvariantViolation { detail } => {
                format!("\"detail\":\"{}\"", escape(detail))
            }
            // ly is in the envelope; mode/dot/detail are new.
            AnomalyKind::PpuModeTimingImpossible { mode, dot, detail, .. } => format!(
                "\"mode\":{mode},\"dot\":{dot},\"detail\":\"{}\"",
                escape(detail)
            ),
            // ie/if are in the envelope; ime/detail are new.
            AnomalyKind::InterruptInvariantViolation { ime, detail, .. } => {
                format!("\"ime\":{ime},\"detail\":\"{}\"", escape(detail))
            }
            AnomalyKind::StateHashDivergence { expected, actual, frame, mcycle } => format!(
                "\"expected\":{expected},\"actual\":{actual},\"hash_frame\":{frame},\"hash_mcycle\":{mcycle}"
            ),
            AnomalyKind::AssertionFailure { subsystem, detail } => format!(
                "\"subsystem\":\"{}\",\"detail\":\"{}\"",
                escape(subsystem),
                escape(detail)
            ),
        }
    }
}

/// A fully-contextualised anomaly, ready to serialize to `anomalies.jsonl`.
#[derive(Clone, Debug)]
pub struct AnomalyEvent {
    pub severity: Severity,
    pub kind: AnomalyKind,
    pub mcycle: u64,
    pub frame: u64,
    pub pc: u16,
    pub opcode: u8,
    pub ly: u8,
    pub ppu_mode: u8,
    pub ie: u8,
    pub if_: u8,
}

impl AnomalyEvent {
    /// One JSON object line for `anomalies.jsonl`.
    pub fn to_json_line(&self) -> String {
        let extra = self.kind.extra_fields_json();
        let extra = if extra.is_empty() {
            String::new()
        } else {
            format!(",{extra}")
        };
        format!(
            "{{\"severity\":\"{}\",\"kind\":\"{}\",\"mcycle\":{},\"frame\":{},\"pc\":{},\"opcode\":{},\"ly\":{},\"ppu_mode\":{},\"ie\":{},\"if\":{}{}}}",
            self.severity.as_str(),
            self.kind.tag(),
            self.mcycle,
            self.frame,
            self.pc,
            self.opcode,
            self.ly,
            self.ppu_mode,
            self.ie,
            self.if_,
            extra,
        )
    }

    /// Append this event to `anomalies.jsonl` in `dir`.
    pub fn append_to(&self, dir: &Path) -> std::io::Result<()> {
        let path = dir.join("anomalies.jsonl");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(f, "{}", self.to_json_line())?;
        Ok(())
    }
}

/// Escape a string for embedding in a JSON string literal. Handles the
/// required control characters so `anomalies.jsonl` is always valid JSON.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: AnomalyKind, sev: Severity) -> AnomalyEvent {
        AnomalyEvent {
            severity: sev,
            kind,
            mcycle: 12345,
            frame: 7,
            pc: 0x0150,
            opcode: 0xD3,
            ly: 144,
            ppu_mode: 1,
            ie: 0x1F,
            if_: 0xE1,
        }
    }

    #[test]
    fn illegal_opcode_json_has_kind_and_fields() {
        let e = event(
            AnomalyKind::IllegalOpcode {
                opcode: 0xD3,
                pc: 0x0150,
            },
            Severity::Error,
        );
        let line = e.to_json_line();
        assert!(line.contains("\"kind\":\"IllegalOpcode\""));
        assert!(line.contains("\"opcode\":211")); // 0xD3
        assert!(line.contains("\"pc\":336")); // 0x0150
        assert!(line.contains("\"severity\":\"error\""));
    }

    #[test]
    fn json_line_has_no_duplicate_keys() {
        // Envelope already carries pc/opcode/ly/ppu_mode/ie/if; kind fields
        // must not repeat them. Parse and assert each key appears once.
        let e = event(
            AnomalyKind::IllegalOpcode {
                opcode: 0xD3,
                pc: 0x0150,
            },
            Severity::Error,
        );
        let line = e.to_json_line();
        let v: serde_json::Value =
            serde_json::from_str(&line).expect("anomaly line must be valid JSON");
        let obj = v.as_object().unwrap();
        // serde_json keeps last value on dup keys, so a raw substring count is
        // the real guard against duplicates in the emitted text.
        assert_eq!(
            line.matches("\"opcode\":").count(),
            1,
            "opcode emitted once"
        );
        assert_eq!(line.matches("\"pc\":").count(), 1, "pc emitted once");
        assert!(obj.contains_key("opcode"));
        assert!(obj.contains_key("pc"));
    }

    #[test]
    fn severity_dumps_only_on_error_fatal() {
        assert!(!Severity::Info.dumps());
        assert!(!Severity::Warn.dumps());
        assert!(Severity::Error.dumps());
        assert!(Severity::Fatal.dumps());
    }

    #[test]
    fn append_creates_jsonl() {
        let dir = std::env::temp_dir().join(format!("rubc_anom_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        event(
            AnomalyKind::StuckCpu {
                pc: 0x100,
                repeated: 99,
            },
            Severity::Fatal,
        )
        .append_to(&dir)
        .unwrap();
        event(AnomalyKind::UnmappedRead { addr: 0xFEA0 }, Severity::Warn)
            .append_to(&dir)
            .unwrap();

        let content = std::fs::read_to_string(dir.join("anomalies.jsonl")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("StuckCpu"));
        assert!(lines[1].contains("UnmappedRead"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// All 10 variants: every emitted line must be valid JSON, carry the kind
    /// tag, and contain no duplicate top-level keys in the raw text.
    #[test]
    fn every_variant_emits_valid_unique_keyed_json() {
        let kinds = vec![
            AnomalyKind::IllegalOpcode {
                opcode: 0xD3,
                pc: 0x0150,
            },
            AnomalyKind::StuckCpu {
                pc: 0x100,
                repeated: 99,
            },
            AnomalyKind::InfiniteLoopHeuristic {
                pc: 0x200,
                iterations: 5000,
            },
            AnomalyKind::UnmappedRead { addr: 0xFEA0 },
            AnomalyKind::UnmappedWrite {
                addr: 0xFEA0,
                value: 0x42,
            },
            AnomalyKind::BusInvariantViolation {
                detail: "tick before sample",
            },
            AnomalyKind::PpuModeTimingImpossible {
                ly: 144,
                mode: 3,
                dot: 999,
                detail: "mode3 overrun",
            },
            AnomalyKind::InterruptInvariantViolation {
                ie: 0x1F,
                if_: 0xE1,
                ime: true,
                detail: "ie cleared mid-dispatch",
            },
            AnomalyKind::StateHashDivergence {
                expected: 1,
                actual: 2,
                frame: 7,
                mcycle: 99,
            },
            AnomalyKind::AssertionFailure {
                subsystem: "ppu",
                detail: "line\nwith\tcontrol\rchars and \"quotes\"".to_string(),
            },
        ];
        assert_eq!(kinds.len(), 10, "all variants covered");

        for kind in kinds {
            let tag = kind.tag().to_string();
            let e = event(kind, Severity::Error);
            let line = e.to_json_line();

            // 1. Valid JSON (control chars in AssertionFailure must be escaped).
            let v: serde_json::Value = serde_json::from_str(&line)
                .unwrap_or_else(|err| panic!("{tag} produced invalid JSON: {err}\n{line}"));
            let obj = v.as_object().unwrap();

            // 2. Kind tag present.
            assert_eq!(obj["kind"], serde_json::json!(tag));

            // 3. No duplicate envelope keys in the raw text.
            for key in ["pc", "opcode", "ly", "ppu_mode", "ie", "frame", "mcycle"] {
                let needle = format!("\"{key}\":");
                assert_eq!(
                    line.matches(&needle).count(),
                    1,
                    "{tag}: key {key} must appear exactly once in {line}"
                );
            }
        }
    }
}
