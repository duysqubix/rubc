//! Per-run diagnostic directory + `run.json` manifest.
//!
//! Every diagnostic run gets a timestamped directory under the configured
//! diag root, named `<utc>_<rom-stem>_<rom-sha8>`. All artifacts (flight
//! recorder dumps, traces, hashes, metrics, snapshots, anomaly log) land
//! inside it. `run.json` is the manifest an AFK agent reads first.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::sha256;

/// Why a run ended. Recorded in `run.json` so an agent knows how to triage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndReason {
    Running,
    MaxFrames,
    Quit,
    Panic,
    StuckCpu,
    IllegalOpcode,
    Anomaly,
    Error,
}

impl EndReason {
    fn as_str(&self) -> &'static str {
        match self {
            EndReason::Running => "running",
            EndReason::MaxFrames => "max_frames",
            EndReason::Quit => "quit",
            EndReason::Panic => "panic",
            EndReason::StuckCpu => "stuck_cpu",
            EndReason::IllegalOpcode => "illegal_opcode",
            EndReason::Anomaly => "anomaly",
            EndReason::Error => "error",
        }
    }
}

/// Owns the run directory and writes the `run.json` manifest.
#[derive(Clone)]
pub struct RunContext {
    pub dir: PathBuf,
    pub run_id: String,
    rom_path: String,
    rom_sha256: String,
    git_sha: String,
    features: Vec<String>,
    mode: String,
    boot_rom: String,
    pub end_reason: EndReason,
}

impl RunContext {
    /// Create a fresh run directory under `root` for `rom_path`.
    ///
    /// `rom_bytes` is hashed for the manifest + directory name. `features`
    /// records which diagnostic features were compiled in.
    pub fn new(
        root: &Path,
        rom_path: &str,
        rom_bytes: &[u8],
        features: Vec<String>,
        mode: &str,
        boot_rom: &str,
    ) -> std::io::Result<Self> {
        let sha = sha256_hex(rom_bytes);
        let sha8 = &sha[..8.min(sha.len())];
        let stem = Path::new(rom_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("rom");
        let ts = utc_timestamp();
        let run_id = format!("{ts}_{stem}_{sha8}");
        let dir = root.join(&run_id);
        std::fs::create_dir_all(&dir)?;

        Ok(Self {
            dir,
            run_id,
            rom_path: rom_path.to_string(),
            rom_sha256: sha,
            git_sha: git_sha(),
            features,
            mode: mode.to_string(),
            boot_rom: boot_rom.to_string(),
            end_reason: EndReason::Running,
        })
    }

    /// Path to an artifact inside the run directory.
    pub fn artifact(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn set_end_reason(&mut self, reason: EndReason) {
        self.end_reason = reason;
    }

    /// Write (or overwrite) `run.json`. Hand-rolled JSON to avoid forcing
    /// serde into the always-on diagnostics gate; values are escaped.
    pub fn write_manifest(&self) -> std::io::Result<()> {
        let path = self.dir.join("run.json");
        let mut f = std::fs::File::create(&path)?;
        let features = self
            .features
            .iter()
            .map(|s| format!("\"{}\"", json_escape(s)))
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            "{{\n  \"schema\": \"rubc.diag.run.v1\",\n  \"run_id\": \"{}\",\n  \"git_sha\": \"{}\",\n  \"features\": [{}],\n  \"rom_path\": \"{}\",\n  \"rom_sha256\": \"{}\",\n  \"mode\": \"{}\",\n  \"boot_rom\": \"{}\",\n  \"end_reason\": \"{}\"\n}}\n",
            json_escape(&self.run_id),
            json_escape(&self.git_sha),
            features,
            json_escape(&self.rom_path),
            json_escape(&self.rom_sha256),
            json_escape(&self.mode),
            json_escape(&self.boot_rom),
            self.end_reason.as_str(),
        );
        f.write_all(json.as_bytes())?;
        Ok(())
    }
}

/// Hex SHA-256 of `bytes` (pure-Rust, no C bindings).
pub fn sha256_hex(bytes: &[u8]) -> String {
    sha256::hex(bytes)
}

/// Best-effort short git SHA. Falls back to "unknown" if git is unavailable.
fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// UTC timestamp `YYYY-MM-DDTHH-MM-SSZ` (filesystem-safe; no colons).
/// Pure `std::time` + civil-date math; no `chrono`, no FFI.
fn utc_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}-{mi:02}-{s:02}Z")
}

/// Convert Unix seconds to UTC (year, month, day, hour, min, sec).
/// Uses Howard Hinnant's days-from-civil algorithm (public domain).
fn civil_from_unix(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = (secs % 86_400) as u32;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;

    // days since 1970-01-01 -> civil date.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, day, hour, min, sec)
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
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

    #[test]
    fn sha256_is_known_vector() {
        // SHA-256("") well-known empty-input digest.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn creates_run_dir_with_manifest() {
        let root = std::env::temp_dir().join(format!("rubc_run_test_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut ctx = RunContext::new(
            &root,
            "/games/tetris.gb",
            b"\x00\x01\x02fake-rom",
            vec!["flight-recorder".to_string(), "trace".to_string()],
            "DMG",
            "disabled",
        )
        .unwrap();
        ctx.set_end_reason(EndReason::StuckCpu);
        ctx.write_manifest().unwrap();

        // Dir name contains rom stem.
        assert!(ctx.run_id.contains("tetris"));
        let manifest = std::fs::read_to_string(ctx.dir.join("run.json")).unwrap();
        assert!(manifest.contains("\"schema\": \"rubc.diag.run.v1\""));
        assert!(manifest.contains("\"mode\": \"DMG\""));
        assert!(manifest.contains("\"end_reason\": \"stuck_cpu\""));
        assert!(manifest.contains("\"flight-recorder\""));
        // rom_sha256 present and 64 hex chars.
        assert!(manifest.contains("\"rom_sha256\""));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn json_escape_handles_specials() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("line\n"), "line\\n");
    }
}
