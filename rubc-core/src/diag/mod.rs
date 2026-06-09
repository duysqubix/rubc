//! Self-diagnosis framework for AFK debugging.
//!
//! Compiled in only under the `diagnostics` feature (or a sub-feature that
//! implies it). When disabled, this module does not exist and the
//! [`crate::diag_record_mcycle`] / [`crate::diag_trace_instr`] macros expand to
//! nothing — zero cost, no fields, no branches.
//!
//! Design rule: diagnostics OBSERVE, they never PARTICIPATE. No code here calls
//! the bus `read_m`/`write_m`/`idle_m` or any ticking function. Side-effect-free
//! peeks only. The flight recorder is written *after* a bus M-cycle completes,
//! so it never perturbs cycle timing.

pub mod anomaly;
pub mod panic;
pub mod run;
pub mod sha256;

#[cfg(feature = "flight-recorder")]
pub mod flight;

#[cfg(feature = "trace")]
pub mod trace;

#[cfg(feature = "trace")]
pub mod ppu_trace;

#[cfg(feature = "hash")]
pub mod hash;

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "snapshot")]
pub mod snapshot;

pub use anomaly::{AnomalyEvent, AnomalyKind, Severity};
pub use run::{EndReason, RunContext};

#[cfg(feature = "flight-recorder")]
pub use flight::{BusKind, ExecTag, FlightRecord, FlightRecorder};

#[cfg(feature = "trace")]
pub use trace::{format_bgb_line, BgbRegs};

#[cfg(feature = "trace")]
pub use ppu_trace::{PpuPhase, PpuPhaseTrace, PpuSample, PpuWrite};

#[cfg(feature = "hash")]
pub use hash::{fnv1a64, HashCsv, StateHasher};

#[cfg(feature = "metrics")]
pub use metrics::Metrics;

#[cfg(feature = "snapshot")]
pub use snapshot::MachineSnapshot;

/// Which diagnostic features were compiled in. Recorded in `run.json`.
pub fn compiled_features() -> Vec<String> {
    let mut v = vec!["diagnostics".to_string()];
    if cfg!(feature = "flight-recorder") {
        v.push("flight-recorder".to_string());
    }
    if cfg!(feature = "metrics") {
        v.push("metrics".to_string());
    }
    if cfg!(feature = "trace") {
        v.push("trace".to_string());
    }
    if cfg!(feature = "hash") {
        v.push("hash".to_string());
    }
    if cfg!(feature = "snapshot") {
        v.push("snapshot".to_string());
    }
    v
}

/// Orchestrates the run directory, flight recorder, and anomaly log.
///
/// This is the single object the emulator threads through its bus/CPU. The
/// `diag_record_mcycle!` / `diag_trace_instr!` macros call its methods, so call
/// sites stay zero-cost when features are off (the macro expands to nothing).
pub struct Diagnostics {
    pub run: RunContext,
    #[cfg(feature = "flight-recorder")]
    pub recorder: flight::FlightRecorder,
    #[cfg(feature = "metrics")]
    pub metrics: metrics::Metrics,
    /// Shared handle the panic hook reads to dump the lead-up on a crash.
    /// `None` until `arm_panic_hook` is called.
    panic_dumper: Option<std::sync::Arc<std::sync::Mutex<panic::PanicDumper>>>,
}

impl Diagnostics {
    /// Build from an existing run context. Recorder capacity is a power of two;
    /// `recorder_enabled = false` keeps the ring allocated but inert.
    pub fn new(run: RunContext, #[cfg(feature = "flight-recorder")] recorder_cap: usize) -> Self {
        Self {
            run,
            #[cfg(feature = "flight-recorder")]
            recorder: flight::FlightRecorder::new(recorder_cap, true),
            #[cfg(feature = "metrics")]
            metrics: metrics::Metrics::new(),
            panic_dumper: None,
        }
    }

    /// Write `metrics.json` to the run directory. Best-effort; call on exit /
    /// anomaly / stuck.
    #[cfg(feature = "metrics")]
    pub fn write_metrics(&self) -> std::io::Result<()> {
        self.metrics.write_json(&self.run.dir)
    }

    /// Install the panic hook for this thread so that an unwinding panic dumps
    /// the flight recorder + manifest (`end_reason=panic`). Call once after
    /// constructing `Diagnostics`. The hook reads a shared snapshot refreshed by
    /// [`publish`](Self::publish), so call `publish` before risky sections (e.g.
    /// once per frame) to keep the dumped lead-up current.
    pub fn arm_panic_hook(&mut self) {
        let dumper = std::sync::Arc::new(std::sync::Mutex::new(panic::PanicDumper {
            dir: self.run.dir.clone(),
            #[cfg(feature = "flight-recorder")]
            records: Vec::new(),
            run: self.run.clone(),
        }));
        panic::arm(dumper.clone());
        self.panic_dumper = Some(dumper);
        self.publish();
    }

    /// Disarm this thread's panic hook (the process-wide hook stays installed but
    /// becomes inert for this thread).
    pub fn disarm_panic_hook(&mut self) {
        panic::disarm();
        self.panic_dumper = None;
    }

    /// Publish the current recorder snapshot to the panic dumper so a subsequent
    /// panic dumps the up-to-date lead-up. NOT on the per-M-cycle hot path —
    /// call periodically (e.g. per frame).
    pub fn publish(&self) {
        if let Some(dumper) = &self.panic_dumper {
            #[cfg(feature = "flight-recorder")]
            panic::publish_records(dumper, self.recorder.snapshot_chronological());
            #[cfg(not(feature = "flight-recorder"))]
            let _ = dumper;
        }
    }

    /// Record one CPU M-cycle. Called via `diag_record_mcycle!` after the bus
    /// M-cycle completes. Observe-only: never touches the bus.
    #[cfg(feature = "flight-recorder")]
    #[inline(always)]
    pub fn record_mcycle(&mut self, rec: flight::FlightRecord) {
        self.recorder.record(rec);
    }

    /// Append one PRE-FORMATTED BGB-style trace line to `trace.bgb`.
    ///
    /// Takes an owned `String` (NOT a closure) by design: the diagnostics layer
    /// must never execute arbitrary caller code that could touch the bus/tick.
    /// The caller builds the line from a side-effect-free snapshot first, then
    /// hands the finished string here. Best-effort: write errors are ignored to
    /// avoid perturbing emulation.
    #[cfg(feature = "trace")]
    pub fn trace_instr_line(&mut self, line: String) {
        use std::io::Write;
        let path = self.run.dir.join("trace.bgb");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }

    /// Append an anomaly to `anomalies.jsonl`. On Error/Fatal, set the run's
    /// end reason from the anomaly and dump all artifacts so the manifest
    /// reflects the failure (not a stale "running").
    pub fn report_anomaly(&mut self, event: &AnomalyEvent) -> std::io::Result<()> {
        event.append_to(&self.run.dir)?;
        if event.severity.dumps() {
            self.run.set_end_reason(end_reason_for(&event.kind));
            self.dump_on_error()?;
        }
        Ok(())
    }

    /// Flush every available artifact to the run directory. Each artifact is
    /// attempted independently so one failure does not skip the others; the
    /// first error is returned after all attempts complete.
    pub fn dump_on_error(&self) -> std::io::Result<()> {
        let mut first_err: Option<std::io::Error> = None;
        let mut record = |r: std::io::Result<()>| {
            if let Err(e) = r {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        };

        #[cfg(feature = "flight-recorder")]
        record(self.recorder.dump(&self.run.dir, 4096));
        #[cfg(feature = "metrics")]
        record(self.metrics.write_json(&self.run.dir));
        record(self.run.write_manifest());

        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// Map an anomaly kind to the run end reason it should record.
fn end_reason_for(kind: &AnomalyKind) -> EndReason {
    match kind {
        AnomalyKind::IllegalOpcode { .. } => EndReason::IllegalOpcode,
        AnomalyKind::StuckCpu { .. } => EndReason::StuckCpu,
        _ => EndReason::Anomaly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_ctx(tag: &str) -> RunContext {
        let root =
            std::env::temp_dir().join(format!("rubc_diag_orch_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        RunContext::new(
            &root,
            "/g/x.gb",
            b"rom-bytes",
            compiled_features(),
            "DMG",
            "disabled",
        )
        .unwrap()
    }

    #[cfg(feature = "flight-recorder")]
    #[test]
    fn record_macro_feeds_recorder_then_dump_on_error() {
        let ctx = fresh_ctx("rec");
        let dir = ctx.dir.clone();
        let mut diag = Diagnostics::new(ctx, 16);

        for m in 0..3u64 {
            let rec = flight::FlightRecord {
                mcycle: m,
                pc: 0x100 + m as u16,
                ..Default::default()
            };
            // Exercise the actual macro (the N4 call-site contract).
            crate::diag_record_mcycle!(&mut diag, rec);
        }

        let ev = AnomalyEvent {
            severity: Severity::Fatal,
            kind: AnomalyKind::IllegalOpcode {
                opcode: 0xD3,
                pc: 0x0150,
            },
            mcycle: 3,
            frame: 0,
            pc: 0x0150,
            opcode: 0xD3,
            ly: 0,
            ppu_mode: 0,
            ie: 0,
            if_: 0,
        };
        diag.report_anomaly(&ev).unwrap();

        // Fatal anomaly dumped the recorder + manifest.
        assert!(dir.join("flight.bin").exists());
        assert!(dir.join("flight.tail.txt").exists());
        assert!(dir.join("anomalies.jsonl").exists());
        assert!(dir.join("run.json").exists());
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[cfg(feature = "trace")]
    #[test]
    fn trace_macro_appends_bgb_line_lazily() {
        let ctx = fresh_ctx("trace");
        let dir = ctx.dir.clone();
        let mut diag = Diagnostics::new(
            ctx,
            #[cfg(feature = "flight-recorder")]
            16,
        );
        // Exercise the trace macro; the closure produces a BGB-style line.
        crate::diag_trace_instr!(&mut diag, || {
            "A:01 F:B0 B:00 C:13 SP:FFFE PC:0100 PCMEM:00,C3,13,02".to_string()
        });
        let bgb = std::fs::read_to_string(dir.join("trace.bgb")).unwrap();
        assert!(bgb.contains("PC:0100"));
        assert_eq!(bgb.lines().count(), 1);
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[test]
    fn report_anomaly_warn_does_not_dump() {
        let ctx = fresh_ctx("warn");
        let dir = ctx.dir.clone();
        let mut diag = Diagnostics::new(
            ctx,
            #[cfg(feature = "flight-recorder")]
            16,
        );
        let ev = AnomalyEvent {
            severity: Severity::Warn,
            kind: AnomalyKind::UnmappedRead { addr: 0xFEA0 },
            mcycle: 1,
            frame: 0,
            pc: 0,
            opcode: 0,
            ly: 0,
            ppu_mode: 0,
            ie: 0,
            if_: 0,
        };
        diag.report_anomaly(&ev).unwrap();
        // Warn logs the anomaly but does NOT dump artifacts.
        assert!(dir.join("anomalies.jsonl").exists());
        assert!(!dir.join("flight.bin").exists());
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    /// End-to-end: build Diagnostics, record cycles through the macro, arm the
    /// panic hook from THAT Diagnostics, panic, and assert the dumped artifacts
    /// contain the recorded cycles + `end_reason=panic`. This is the real wiring
    /// path (not a hand-built PanicDumper).
    #[cfg(feature = "flight-recorder")]
    #[test]
    fn armed_diagnostics_dumps_recorded_cycles_on_panic() {
        let ctx = fresh_ctx("e2e_panic");
        let dir = ctx.dir.clone();
        let mut diag = Diagnostics::new(ctx, 64);

        for m in 0..5u64 {
            let rec = flight::FlightRecord {
                mcycle: m,
                pc: 0x0150 + m as u16,
                ..Default::default()
            };
            crate::diag_record_mcycle!(&mut diag, rec);
        }
        // Arm + publish the current recorder snapshot to the panic dumper.
        diag.arm_panic_hook();

        let result = std::panic::catch_unwind(|| {
            panic!("simulated crash mid-emulation");
        });
        assert!(result.is_err());

        // Artifacts exist and reflect the recorded cycles + panic reason.
        assert!(dir.join("flight.bin").exists());
        let tail = std::fs::read_to_string(dir.join("flight.tail.txt")).unwrap();
        assert!(
            tail.contains("pc=0150"),
            "dumped tail must contain recorded cycles"
        );
        assert!(tail.contains("pc=0154"));
        let manifest = std::fs::read_to_string(dir.join("run.json")).unwrap();
        assert!(manifest.contains("\"end_reason\": \"panic\""));

        diag.disarm_panic_hook();
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }
}
