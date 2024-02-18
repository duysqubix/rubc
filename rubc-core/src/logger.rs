use fern::colors::{Color, ColoredLevelConfig};
use std::{env, fs, time::SystemTime};

pub fn setup_logger() -> Result<(), fern::InitError> {
    let now = SystemTime::now();
    let now = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    // convert to mm-dd-yyyy
    let now = chrono::NaiveDateTime::from_timestamp_opt(now.as_secs() as i64, 0).unwrap();
    let now = now.format("%m-%d-%Y:%H").to_string();
    let now = now.as_str();

    let log_dir = std::env::temp_dir().join("logs/rubc");
    fs::create_dir_all(&log_dir)?;

    let mut log_level = log::LevelFilter::Warn;

    if let Ok(ll) = env::var("LOG_LEVEL") {
        log_level = match ll.to_lowercase().as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Warn,
        };
    }

    let log_file = log_dir.join(format!("{}.log", now));

    let colors = ColoredLevelConfig {
        error: Color::Red,
        warn: Color::Yellow,
        info: Color::Green,
        debug: Color::Blue,
        trace: Color::Magenta,
    };

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}{}[{}][{}] {}",
                format_args!("\x1B[{}m", colors.get_color(&record.level()).to_fg_str()),
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors.color(record.level()),
                message
            ))
        })
        .level(log::LevelFilter::Error)
        .level_for("rubc_core", log_level)
        .level_for("rubc", log_level)
        .chain(std::io::stdout())
        .chain(fern::log_file(log_file)?)
        .apply()?;

    log::debug!("Logger initialized");

    Ok(())
}
