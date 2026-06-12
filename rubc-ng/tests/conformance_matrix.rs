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
