#![forbid(unsafe_code)]

pub mod bus_intent;
pub mod golden;
pub mod machine;
pub mod manifest;
pub mod model;
pub mod time;
pub mod timing;

pub use bus_intent::{CpuBusIntent, CpuIntentSource, IntentOutcome};
pub use golden::{
    assert_golden_edges, GoldenRow, GoldenSelection, GoldenTrace, GoldenV2Reader, ObservableSample,
    ObservableValue, TraceMismatch,
};
pub use machine::{MachineNg, StepRecord};
pub use manifest::{Expectation, Manifest, RomManifestEntry, VectorSuiteEntry};
pub use model::GbModel;
pub use time::{ClockPhase, ClockSpine, Time};
pub use timing::{
    Anchor, Observable, PhaseRule, TimingDomain, TimingEntry, TimingProfile, TimingTable,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn gb_model_exposes_full_hardware_set_and_predicates() {
        assert_eq!(GbModel::ALL.len(), 13);
        assert!(GbModel::DmgB.is_dmg_family());
        assert!(GbModel::Mgb.is_dmg_family());
        assert!(GbModel::Sgb2.is_dmg_family());
        assert!(GbModel::CgbE.is_cgb());
        assert!(GbModel::Agb.is_cgb());
        assert!(!GbModel::Dmg0.is_cgb());
    }

    #[test]
    fn timing_table_lookup_is_declarative_and_model_selected() {
        let dmg = TimingTable::for_model(GbModel::DmgB);
        let cgb = TimingTable::for_model(GbModel::CgbE);

        let dmg_entry = dmg
            .lookup(TimingDomain::PpuInternal, "bg_fetch_tile_high_t1")
            .expect("DMG table must expose named BG fetch high T1 entry");
        assert_eq!(dmg_entry.anchor, Anchor::PpuMode3Start);
        assert_eq!(dmg_entry.observable, Observable::PpuFetchSample);
        assert_eq!(dmg_entry.offset.subphases(), 22 * 4);

        let cgb_dot = cgb
            .lookup(TimingDomain::PpuPublic, "ppu_dot")
            .expect("CGB table must expose PPU public dot cadence");
        assert_eq!(cgb_dot.phase, PhaseRule::EveryCpuT { divisor: 1 });
    }

    #[test]
    fn machine_ng_nop_rom_is_deterministic() {
        let rom = vec![0x00; 0x8000];
        let mut a = MachineNg::from_rom(GbModel::DmgB, &rom).expect("valid ROM loads");
        let mut b = MachineNg::from_rom(GbModel::DmgB, &rom).expect("valid ROM loads");

        let trace_a = a.run_steps(32);
        let trace_b = b.run_steps(32);

        assert_eq!(
            trace_a, trace_b,
            "same ROM/model must produce identical Time sequence"
        );
        assert_eq!(trace_a.first().map(|r| r.time), Some(Time::ZERO));
        assert_eq!(trace_a.last().map(|r| r.time.subphases()), Some(31));
    }

    #[test]
    fn golden_reader_loads_sameboy_tsv_schema_as_typed_rows() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent")
            .join("reference/goldens/m3_scy_change_ly000.tsv");
        let trace = GoldenTrace::read_tsv(path).expect("golden TSV parses");

        assert_eq!(trace.rows.len(), 86);
        let write = trace
            .rows
            .iter()
            .find(|row| row.kind == "write" && row.addr == Some(0xFF42) && row.byte == Some(0x03))
            .expect("SCY=03 write row is typed");
        assert_eq!(write.norm_dot, Some(14.0));
        assert_eq!(write.conflict.as_deref(), Some("READ_NEW"));

        let fetch = trace
            .rows
            .iter()
            .find(|row| {
                row.kind == "fetch"
                    && row.state.as_deref() == Some("GET_TILE_DATA_HIGH_T1")
                    && row.io_scy == Some(0x03)
            })
            .expect("HIGH_T1 fetch with new SCY is typed");
        assert_eq!(fetch.norm_dot, Some(22.0));
    }

    #[test]
    fn golden_v2_reader_streams_real_trace_rows_without_eager_trace_load() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent")
            .join("reference/goldens/v2/acceptance__ppu__hblank_ly_scx_timing-GS__dmg.tsv");
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            return;
        }

        let mut rows = GoldenV2Reader::open(&path)
            .expect("v2 reader opens")
            .filter_selection(
                GoldenSelection::new()
                    .kind("ppu_public")
                    .event("stat_sample")
                    .frames(60..=60)
                    .lines(145..=145)
                    .limit(1),
            );
        let row = rows
            .next()
            .expect("real trace has selected row")
            .expect("selected row parses");

        assert_eq!(row.schema, 2);
        assert_eq!(row.kind, "ppu_public");
        assert_eq!(row.frame, 60);
        assert_eq!(row.ly, Some(145));
        assert_eq!(row.mode, Some(1));
        assert_eq!(row.stat_sources.as_deref(), Some("00"));
    }

    #[test]
    fn golden_assertion_reports_first_divergence_and_accepts_matching_stub() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent")
            .join("reference/goldens/v2/acceptance__ppu__hblank_ly_scx_timing-GS__dmg.tsv");
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            return;
        }
        let selection = GoldenSelection::new()
            .kind("ppu_public")
            .event("stat_sample")
            .frames(60..=60)
            .lines(145..=145)
            .limit(1);
        let expected = GoldenV2Reader::open(&path)
            .expect("v2 reader opens")
            .filter_selection(selection.clone())
            .next()
            .expect("selected row exists")
            .expect("selected row parses")
            .to_observable_sample(Observable::PpuModeEdge)
            .expect("row maps to PpuModeEdge sample");

        let mismatching = [ObservableSample {
            value: ObservableValue::U8(0),
            ..expected.clone()
        }];
        let err = assert_golden_edges(
            mismatching.iter().cloned(),
            GoldenV2Reader::open(&path).expect("v2 reader opens"),
            Observable::PpuModeEdge,
            "hblank_ly_scx_timing-GS",
            selection.clone(),
        )
        .expect_err("deliberate stub mismatch must report diagnostic");
        let diagnostic = err.to_string();
        assert!(diagnostic.contains("first divergence for hblank_ly_scx_timing-GS PpuModeEdge"));
        assert!(diagnostic.contains("expected"));
        assert!(diagnostic.contains("actual"));

        assert_golden_edges!(
            [expected].into_iter(),
            GoldenV2Reader::open(&path).expect("v2 reader opens"),
            Observable::PpuModeEdge,
            "hblank_ly_scx_timing-GS",
            selection
        );
    }

    #[test]
    fn manifest_loader_parses_rom_entries_and_validates_reference_paths_when_present() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent");
        let manifest_path = workspace.join("rubc-ng-data/test-manifest.toml");
        let manifest = Manifest::read(&manifest_path).expect("manifest parses");

        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.rom_count, 207);
        assert_eq!(manifest.roms.len(), 207);
        assert!(manifest
            .roms
            .iter()
            .any(|rom| rom.path == "reference/test-suites/acid2/dmg-acid2.gb"
                && rom.expectation.kind == "pixel-exact"));

        let reference_root = workspace.join("reference/test-suites");
        if !reference_root.exists() {
            eprintln!("skip: {reference_root:?} absent");
            return;
        }
        let missing = manifest.missing_reference_paths(workspace);
        assert!(missing.is_empty(), "missing manifest paths: {missing:?}");
    }
}
