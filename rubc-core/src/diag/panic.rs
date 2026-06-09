//! Panic-driven artifact dumping. Pure-safe Rust: no `unsafe`, no raw pointers.
//!
//! The emulator runs on one thread. We keep a thread-local handle to a small
//! [`PanicDumper`] — the run directory plus a shared, lockable view of the
//! flight recorder's records. When a panic unwinds, the installed
//! [`std::panic::set_hook`] reads that handle and writes the same artifacts a
//! fatal anomaly would (`flight.bin`, `flight.tail.txt`, `run.json`), so an AFK
//! agent gets the lead-up to the crash even when no anomaly was reported.
//!
//! The hot path (`record_mcycle`) does NOT go through this — it writes the
//! owned recorder directly. The dumper only holds what the panic hook needs.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::run::{EndReason, RunContext};

/// Everything the panic hook needs to dump, shared via `Arc<Mutex<..>>` so the
/// hook can access it without borrowing `Diagnostics`.
pub struct PanicDumper {
    pub dir: PathBuf,
    /// A snapshot of the flight records, refreshed by the owner. The panic hook
    /// dumps whatever was last published here.
    #[cfg(feature = "flight-recorder")]
    pub records: Vec<super::flight::FlightRecord>,
    /// A clone of the run context (sans recorder) so the manifest can be written
    /// with a panic end reason.
    pub run: RunContext,
}

#[cfg(feature = "flight-recorder")]
impl PanicDumper {
    fn dump(&mut self) {
        use std::io::Write;

        // run.json with panic end reason.
        self.run.set_end_reason(EndReason::Panic);
        let _ = self.run.write_manifest();

        // flight.bin + flight.tail.txt from the last published snapshot.
        let bin_path = self.dir.join("flight.bin");
        if let Ok(f) = std::fs::File::create(&bin_path) {
            let mut bin = std::io::BufWriter::new(f);
            let _ = bin.write_all(b"RUBCFR01");
            let _ = bin.write_all(&(self.records.len() as u32).to_le_bytes());
            let _ = bin.write_all(&(super::flight::FlightRecord::ENCODED_LEN as u32).to_le_bytes());
            let mut scratch = [0u8; super::flight::FlightRecord::ENCODED_LEN];
            for r in &self.records {
                r.write_le(&mut scratch);
                let _ = bin.write_all(&scratch);
            }
            let _ = bin.flush();
        }

        let tail_path = self.dir.join("flight.tail.txt");
        if let Ok(f) = std::fs::File::create(&tail_path) {
            let mut tail = std::io::BufWriter::new(f);
            let start = self.records.len().saturating_sub(4096);
            for r in &self.records[start..] {
                let _ = writeln!(tail, "{}", r.decode_line());
            }
            let _ = tail.flush();
        }
    }
}

#[cfg(not(feature = "flight-recorder"))]
impl PanicDumper {
    fn dump(&mut self) {
        // Without the flight recorder, at least write the panic manifest.
        self.run.set_end_reason(EndReason::Panic);
        let _ = self.run.write_manifest();
    }
}

thread_local! {
    /// The active dumper for this thread, if any. Set by `arm`, cleared by `disarm`.
    static CURRENT: RefCell<Option<Arc<Mutex<PanicDumper>>>> = const { RefCell::new(None) };
}

/// Register `dumper` as this thread's active panic dumper and install the panic
/// hook (idempotent — installs the chained hook only once per process).
pub fn arm(dumper: Arc<Mutex<PanicDumper>>) {
    CURRENT.with(|c| *c.borrow_mut() = Some(dumper));
    install_hook_once();
}

/// Clear this thread's active dumper. The hook stays installed but becomes inert
/// for this thread.
pub fn disarm() {
    CURRENT.with(|c| *c.borrow_mut() = None);
}

use std::sync::Once;
static HOOK_INSTALLED: Once = Once::new();

fn install_hook_once() {
    HOOK_INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Best-effort: dump the active recorder, then chain to the previous
            // hook so normal panic reporting (and the test harness) still works.
            CURRENT.with(|c| {
                if let Some(dumper) = c.borrow().as_ref() {
                    if let Ok(mut d) = dumper.lock() {
                        d.dump();
                    }
                }
            });
            previous(info);
        }));
    });
}

/// Publish the latest flight records into the dumper so a subsequent panic dumps
/// the up-to-date lead-up. Called by the owner (e.g. each frame or on demand) —
/// NOT on the per-M-cycle hot path.
#[cfg(feature = "flight-recorder")]
pub fn publish_records(
    dumper: &Arc<Mutex<PanicDumper>>,
    records: Vec<super::flight::FlightRecord>,
) {
    if let Ok(mut d) = dumper.lock() {
        d.records = records;
    }
}

// All tests here exercise the panic dumper, which only exists under
// `flight-recorder`; gate the whole module so a `trace`-only build doesn't flag
// the helpers/imports as dead code.
#[cfg(all(test, feature = "flight-recorder"))]
mod tests {
    use super::*;
    use crate::diag::compiled_features;

    fn fresh_run(tag: &str) -> (PathBuf, RunContext) {
        let root = std::env::temp_dir().join(format!("rubc_panic_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let ctx = RunContext::new(
            &root,
            "/g/x.gb",
            b"rom",
            compiled_features(),
            "DMG",
            "disabled",
        )
        .unwrap();
        (ctx.dir.clone(), ctx)
    }

    #[cfg(feature = "flight-recorder")]
    #[test]
    fn panic_dumps_artifacts_via_hook() {
        let (dir, run) = fresh_run("hook");
        let records: Vec<super::super::flight::FlightRecord> = (0..3)
            .map(|m| super::super::flight::FlightRecord {
                mcycle: m,
                pc: 0x100 + m as u16,
                ..Default::default()
            })
            .collect();
        let dumper = Arc::new(Mutex::new(PanicDumper {
            dir: dir.clone(),
            records,
            run,
        }));
        arm(dumper.clone());

        // Trigger a panic on this thread; catch_unwind lets the test continue.
        let result = std::panic::catch_unwind(|| {
            panic!("simulated emulator crash");
        });
        assert!(result.is_err());

        // The hook should have dumped artifacts.
        assert!(
            dir.join("flight.bin").exists(),
            "flight.bin written on panic"
        );
        assert!(dir.join("flight.tail.txt").exists());
        assert!(dir.join("run.json").exists());
        let manifest = std::fs::read_to_string(dir.join("run.json")).unwrap();
        assert!(manifest.contains("\"end_reason\": \"panic\""));

        disarm();
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }
}
