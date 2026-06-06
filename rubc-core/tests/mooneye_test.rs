//! Headless mooneye-test-suite runner (ticket rubc-96x).
//!
//! Mooneye acceptance tests are built from WLA-DX `.s` source into `.gb` ROMs
//! under `<suite>/build/` (see `just mooneye-build`). Each ROM ends at a `LD B,B`
//! magic breakpoint; a PASS leaves the Fibonacci signature in the registers
//! (B=3, C=5, D=8, E=13, H=21, L=34 -- [`MOONEYE_PASS`]).
//!
//! This harness drives those ROMs through the public [`Machine`] runner (no GUI,
//! no CLI), so mooneye coverage is verifiable headlessly via `cargo test` and
//! `just mooneye`. It is feature-light: ROM discovery uses only `std::fs`.
//!
//! ## Selecting ROMs
//! The `MOONEYE_GLOB` env var picks a subset by substring of the ROM's path
//! relative to `build/` (e.g. `acceptance/timer`, `tim00`, `bits/`). When unset,
//! ALL built ROMs are run. The harness is a no-op (skips, does not fail) when the
//! build directory is absent, so a checkout without `just mooneye-build` still
//! passes `cargo test`.
//!
//! ## DMG vs CGB
//! Each ROM is booted in the mode its header requests: CGB flag ($0143 bit 7)
//! set -> [`Machine::boot_cgb`], else [`Machine::boot_dmg`].

use rubc_core::machine::{Machine, RunStop};
use std::path::{Path, PathBuf};

/// Max instructions before a mooneye ROM is declared stuck/timed-out.
const MAX_INSTRUCTIONS: u64 = 20_000_000;

/// Absolute path to the WLA-DX build output directory.
fn build_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/test-suites/mooneye-test-suite/build")
}

/// Recursively collect every `.gb` under `dir`, returning (abs_path, rel_path).
fn collect_roms(dir: &Path, root: &Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_roms(&path, root, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("gb") {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((path, rel));
        }
    }
}

/// Outcome of running one mooneye ROM.
enum Outcome {
    Pass,
    /// Reached the breakpoint but registers did not hold the pass signature.
    FailSignature,
    /// Did not reach the breakpoint (stuck or timed out).
    NoBreakpoint(RunStop),
}

fn run_rom(path: &Path) -> std::io::Result<Outcome> {
    let rom = std::fs::read(path)?;
    // Decide DMG vs CGB. Most ROMs use the header CGB flag, but mooneye ships
    // CGB-behavior tests with a DMG header and signals CGB intent via the file
    // name suffix (`-cgb*`, `-C`, `-A` = CGB-variant; `-GS`/`-S`/`-dmg*` = DMG).
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let header_cgb = rom.get(0x0143).is_some_and(|f| f & 0x80 != 0);
    let name_cgb = name.contains("-cgb") || name.ends_with("-C") || name.ends_with("-A");
    let cgb = header_cgb || name_cgb;
    let mut m = if cgb {
        Machine::boot_cgb(&rom)
    } else {
        Machine::boot_dmg(&rom)
    };
    let stop = m.run_mooneye(MAX_INSTRUCTIONS);
    Ok(match stop {
        RunStop::MooneyeBreakpoint if m.mooneye_passed() => Outcome::Pass,
        RunStop::MooneyeBreakpoint => Outcome::FailSignature,
        other => Outcome::NoBreakpoint(other),
    })
}

/// Run built mooneye ROMs and report pass/fail. A FILTERED run (`MOONEYE_GLOB`
/// set) is a HARD GATE: every matched ROM must pass, and at least one must
/// match. An UNFILTERED run is a REPORTING harness: it prints pass/fail for the
/// whole suite and only fails if zero ROMs are discovered -- it does NOT track a
/// previous-pass baseline. Regression enforcement is owned by the per-category
/// filtered gates (rubc-15l etc.), which select their ROMs via `MOONEYE_GLOB`.
#[test]
fn mooneye_suite() {
    let root = build_dir();
    if !root.exists() {
        eprintln!(
            "mooneye: build dir {root:?} absent -- run `just mooneye-build` (needs WLA-DX). Skipping."
        );
        return;
    }

    let filter = std::env::var("MOONEYE_GLOB").ok();
    let mut roms = Vec::new();
    collect_roms(&root, &root, &mut roms);
    roms.sort_by(|a, b| a.1.cmp(&b.1));

    let mut found = 0usize;
    let mut pass = 0usize;
    let mut fails: Vec<String> = Vec::new();

    for (path, rel) in &roms {
        if let Some(f) = &filter {
            if !rel.contains(f.as_str()) {
                continue;
            }
        }
        found += 1;
        match run_rom(path) {
            Ok(Outcome::Pass) => {
                pass += 1;
                println!("PASS  {rel}");
            }
            Ok(Outcome::FailSignature) => {
                println!("FAIL  {rel} (wrong register signature)");
                fails.push(rel.clone());
            }
            Ok(Outcome::NoBreakpoint(stop)) => {
                println!("FAIL  {rel} (no breakpoint: {stop:?})");
                fails.push(rel.clone());
            }
            Err(e) => {
                println!("FAIL  {rel} (io error: {e})");
                fails.push(rel.clone());
            }
        }
    }

    println!("----");
    println!(
        "mooneye {}: {found} found, {pass} pass, {} fail",
        filter.as_deref().unwrap_or("<all>"),
        fails.len()
    );

    // A filtered run is a hard gate (used by per-category ROM-gate tickets +
    // `just mooneye <glob>`): every selected ROM must pass. An UNFILTERED run is
    // a reporting harness (many ROMs need features not yet implemented), so it
    // only fails if zero ROMs were found (build/discovery broken).
    if filter.is_some() {
        assert!(
            fails.is_empty(),
            "mooneye selection had {} failing ROM(s): {:?}",
            fails.len(),
            fails
        );
        assert!(found > 0, "MOONEYE_GLOB matched no ROMs");
    } else {
        assert!(found > 0, "no mooneye ROMs found under {root:?}");
    }
}
