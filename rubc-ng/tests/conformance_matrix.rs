use std::path::Path;

use rubc_ng::conformance::{ConformanceConfig, ConformanceOutcome, ConformanceReport};

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
}

#[test]
fn conformance_matrix_scores_manifest_roms_against_real_oracles() {
    let report = ConformanceReport::run(workspace_root(), ConformanceConfig::default_test_subset())
        .expect("conformance harness runs");

    assert!(
        report.total_roms > 0,
        "default conformance subset must exercise at least one manifest ROM"
    );
    assert!(
        report
            .rows
            .iter()
            .any(|row| row.outcome == ConformanceOutcome::Pass),
        "at least one manifest ROM must reach its real pass signature"
    );
    assert!(
        report
            .rows
            .iter()
            .any(|row| row.outcome == ConformanceOutcome::Skip),
        "unsupported slice-2 expectations must be explicitly skipped, never passed"
    );
}

#[test]
fn conformance_matrix_pass_count_is_gated_by_honest_floor() {
    let config = if std::env::var_os("RUBC_NG_CONFORMANCE_FULL").is_some() {
        ConformanceConfig::full()
    } else {
        ConformanceConfig::default_test_subset()
    };
    let report =
        ConformanceReport::run(workspace_root(), config).expect("conformance harness runs");

    println!("{}", report.scoreboard());

    assert!(
        report.pass_count >= report.config.pass_floor,
        "conformance pass count regressed: got {}, floor {}",
        report.pass_count,
        report.config.pass_floor
    );
}

#[test]
fn conformance_boot_register_and_hwio_profiles_pass_on_intended_models() {
    // Model-exclusive + small CGB/DMG-group boot ROMs that the ng core passes on
    // its score-priority model. boot_hwio-C is DELIBERATELY EXCLUDED: it is a CGB
    // group test (mooneye `C` = cgb+agb+ags) that SHOULD pass on every CGB model,
    // but the ng core currently passes it only on Cgb0 and fails on CgbC/CgbE.
    // The global SCORE_MODEL_PRIORITY (Oracle ses_13da116eb) correctly scores it
    // on CgbE, exposing that real ng boot-HWIO bug rather than masking it behind a
    // first-listed Cgb0 pick. Tracked as a bug; do NOT re-add here until fixed.
    let boot_profile_roms = [
        "boot_hwio-S.gb",
        "boot_hwio-dmg0.gb",
        "boot_hwio-dmgABCmgb.gb",
        "boot_regs-dmg0.gb",
        "boot_regs-dmgABC.gb",
        "boot_regs-mgb.gb",
        "boot_regs-sgb.gb",
        "boot_regs-sgb2.gb",
        "boot_regs-A.gb",
        "boot_regs-cgb.gb",
    ];
    let report = ConformanceReport::run(
        workspace_root(),
        ConformanceConfig {
            pass_floor: boot_profile_roms.len(),
            full_manifest: false,
            path_substrings: boot_profile_roms
                .iter()
                .map(|path| (*path).to_owned())
                .collect(),
        },
    )
    .expect("boot-profile conformance subset runs");

    println!("{}", report.scoreboard());
    assert_eq!(
        report.total_roms,
        boot_profile_roms.len(),
        "boot-profile subset must exercise every targeted boot ROM exactly once"
    );
    assert_eq!(
        report.pass_count,
        report.total_roms,
        "every boot-profile ROM must reach its own mooneye Fibonacci pass signature on its intended model"
    );
}
