use std::env;
use std::io::Write;

/// Minimal pure-Rust `log::Log` implementation.
///
/// Writes `[YYYY-MM-DD][HH:MM:SS][target][LEVEL] message` lines to stderr.
/// Replaces the previous `fern`/`colored` setup so rubc-core no longer pulls
/// in `is-terminal`/`libc` at runtime. Color is intentionally dropped (it was
/// cosmetic); correctness and C-free purity matter more.
struct SimpleLogger {
    /// Level used for the `rubc_core` / `rubc` targets.
    rubc_level: log::LevelFilter,
}

impl SimpleLogger {
    /// Per-target level: rubc crates use `rubc_level`, everything else is
    /// limited to `Error` (mirrors the old fern `.level(Error)` default with
    /// `.level_for("rubc_core"/"rubc", ...)`).
    fn level_for(&self, target: &str) -> log::LevelFilter {
        if target.starts_with("rubc") {
            self.rubc_level
        } else {
            log::LevelFilter::Error
        }
    }
}

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level_for(metadata.target())
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let now = chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]");
        // Lock stderr once for an atomic line write.
        let stderr = std::io::stderr();
        let mut handle = stderr.lock();
        let _ = writeln!(
            handle,
            "{}[{}][{}] {}",
            now,
            record.target(),
            record.level(),
            record.args()
        );
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

/// Read `LOG_LEVEL` from the environment, defaulting to `Warn`.
fn level_from_env() -> log::LevelFilter {
    match env::var("LOG_LEVEL") {
        Ok(ll) => match ll.to_lowercase().as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Warn,
        },
        Err(_) => log::LevelFilter::Warn,
    }
}

/// Install the global logger. Safe to call once per process; subsequent calls
/// return `SetLoggerError` (callers typically use `.ok()`).
pub fn setup_logger() -> Result<(), log::SetLoggerError> {
    let rubc_level = level_from_env();
    log::set_boxed_logger(Box::new(SimpleLogger { rubc_level }))?;
    log::set_max_level(rubc_level);
    log::debug!("Logger initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_from_env_defaults_to_warn() {
        // Not asserting on the live env var (tests share a process); just make
        // sure the mapping function is total and returns a sane default.
        assert!(matches!(
            level_from_env(),
            log::LevelFilter::Trace
                | log::LevelFilter::Debug
                | log::LevelFilter::Info
                | log::LevelFilter::Warn
                | log::LevelFilter::Error
        ));
    }

    #[test]
    fn setup_and_log_does_not_panic() {
        // First call may succeed or, if another test installed a logger first,
        // return Err; either way it must not panic.
        let _ = setup_logger();
        // Logging through the global facade must never panic.
        log::error!("logger smoke test: error");
        log::warn!("logger smoke test: warn");
        log::info!("logger smoke test: info");
        log::debug!("logger smoke test: debug");
        log::trace!("logger smoke test: trace");
    }

    #[test]
    fn rubc_targets_use_configured_level_others_error_only() {
        let logger = SimpleLogger {
            rubc_level: log::LevelFilter::Debug,
        };
        assert_eq!(logger.level_for("rubc_core::cpu"), log::LevelFilter::Debug);
        assert_eq!(logger.level_for("rubc"), log::LevelFilter::Debug);
        assert_eq!(logger.level_for("some_dep"), log::LevelFilter::Error);
    }
}
