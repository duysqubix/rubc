#![forbid(unsafe_code)]

pub mod bus;
pub mod cpu;
pub mod logger;
pub mod machine;
mod savestate;

#[cfg(feature = "diagnostics")]
pub mod diag;

/// Record one CPU M-cycle into the flight recorder. Expands to nothing unless
/// the `flight-recorder` feature is enabled, so the hot path pays zero cost in
/// release builds. Call site lives in the bus, AFTER the M-cycle completes.
///
/// Usage: `diag_record_mcycle!(diag_expr, record_expr)`.
#[cfg(feature = "flight-recorder")]
#[macro_export]
macro_rules! diag_record_mcycle {
    ($diag:expr, $rec:expr) => {{
        // Evaluate the record FIRST so constructing it (which may borrow the
        // same owner as $diag) does not conflict with the &mut borrow below.
        let rec = $rec;
        $crate::diag::Diagnostics::record_mcycle($diag, rec);
    }};
}

/// No-op form when the `flight-recorder` feature is disabled.
#[cfg(not(feature = "flight-recorder"))]
#[macro_export]
macro_rules! diag_record_mcycle {
    ($($tt:tt)*) => {{}};
}

/// Emit one BGB-format instruction trace line. Expands to nothing unless the
/// `trace` feature is enabled. Call site is the CPU at a real opcode fetch.
///
/// Usage: `diag_trace_instr!(diag_expr, build_line_closure)` where the closure
/// is `FnOnce() -> String`. The closure is invoked HERE (only under the `trace`
/// feature, so formatting cost is never paid in release builds), and the
/// resulting `String` is handed to the diagnostics layer. The diagnostics
/// method itself takes a plain `String` and never executes caller code, so it
/// cannot touch the bus/tick.
#[cfg(feature = "trace")]
#[macro_export]
macro_rules! diag_trace_instr {
    ($diag:expr, $build:expr) => {{
        // Build the line FIRST (caller's closure runs here, before we borrow
        // $diag), then hand the finished String to the sink.
        let line: String = ($build)();
        $crate::diag::Diagnostics::trace_instr_line($diag, line);
    }};
}

/// No-op form when the `trace` feature is disabled.
#[cfg(not(feature = "trace"))]
#[macro_export]
macro_rules! diag_trace_instr {
    ($($tt:tt)*) => {{}};
}

pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;
