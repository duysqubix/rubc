#![forbid(unsafe_code)]

pub mod bus_intent;
pub mod cpu;
pub mod golden;
pub mod machine;
pub mod manifest;
pub mod model;
pub mod ppu_internal;
pub mod ppu_public;
pub mod time;
pub mod timing;

pub use bus_intent::{CpuBusIntent, CpuIntentSource, IntentOutcome};
pub use golden::{
    assert_golden_edges, GoldenInitialState, GoldenRow, GoldenSelection, GoldenTrace,
    GoldenV2Reader, GoldenVramRegisters, GoldenVramState, ObservableSample, ObservableValue,
    TraceMismatch, Vram,
};
pub use machine::{MachineNg, StepRecord};
pub use manifest::{Expectation, Manifest, RomManifestEntry, VectorSuiteEntry};
pub use model::GbModel;
pub use ppu_internal::{
    assert_bg_fetch_golden, assert_bg_fetch_golden_with_perturbation, BgFetchDivergence,
    BgFetchSample, PpuInternal,
};
pub use ppu_public::{
    replay_ppu_public_observable, replay_ppu_public_observable_with_table_perturbation, PpuPublic,
    PpuPublicEvent, PpuRegisterWrite,
};
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
        let mode3 = dmg
            .lookup(TimingDomain::PpuPublic, "mode3_public_start")
            .expect("DMG table must expose golden-derived public mode3 start");
        assert_eq!(mode3.offset.subphases(), 176);

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
    fn ppu_public_replay_matches_hblank_golden_mode_edges() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent")
            .join("reference/goldens/v2/acceptance__ppu__hblank_ly_scx_timing-GS__dmg.tsv");
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            return;
        }

        for (event, lines) in [
            ("mode2_enter", 0..=143),
            ("mode3_enter", 0..=143),
            ("mode0_enter", 0..=143),
            ("frame_vblank", 144..=144),
        ] {
            let selection = GoldenSelection::new()
                .kind("ppu_public")
                .event(event)
                .frames(60..=61)
                .lines(lines);
            let actual = replay_ppu_public_observable(
                &path,
                GbModel::DmgB,
                event,
                Observable::PpuModeEdge,
                selection.clone(),
            )
            .expect("PPU-public replay emits selected edges");

            assert_golden_edges!(
                actual,
                GoldenV2Reader::open(&path).expect("v2 reader opens"),
                Observable::PpuModeEdge,
                "hblank_ly_scx_timing-GS",
                selection,
            );
        }
    }

    #[test]
    fn ppu_public_replay_matches_vblank_golden_ly_edges() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent")
            .join("reference/goldens/v2/acceptance__ppu__vblank_stat_intr-GS__dmg.tsv");
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            return;
        }

        let selection = GoldenSelection::new()
            .kind("ppu_public")
            .event("frame_vblank")
            .frames(60..=61)
            .lines(144..=144);
        let actual = replay_ppu_public_observable(
            &path,
            GbModel::DmgB,
            "frame_vblank",
            Observable::PpuLy,
            selection.clone(),
        )
        .expect("PPU-public replay emits selected LY edges");

        assert_golden_edges!(
            actual,
            GoldenV2Reader::open(&path).expect("v2 reader opens"),
            Observable::PpuLy,
            "vblank_stat_intr-GS",
            selection,
        );
    }

    #[test]
    fn golden_v2_reader_accepts_additive_v21_write_visible_columns() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent")
            .join("reference/goldens/v2/acceptance__ppu__hblank_ly_scx_timing-GS__dmg.tsv");
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            return;
        }

        let write = GoldenV2Reader::open(&path)
            .expect("v2.1 reader opens")
            .filter_map(Result::ok)
            .find(|row| row.kind == "cpu" && row.addr == Some(0xFF40))
            .expect("trace carries real LCDC CPU writes");

        assert_eq!(write.write_visible_tick, Some(write.raw_tick));
        assert_eq!(write.write_visible_dot, write.dot);
    }

    #[test]
    fn golden_v2_reader_extracts_capture_window_initial_state_block() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent")
            .join("reference/goldens/v2/acceptance__ppu__hblank_ly_scx_timing-GS__dmg.tsv");
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            return;
        }

        let initial =
            GoldenV2Reader::read_initial_state(&path).expect("v2.2 initial-state block parses");

        assert_eq!(initial.frame, 60);
        assert_eq!(initial.ly, 145);
        assert_eq!(initial.line_tick, 8);
        assert_eq!(initial.mode, 1);
        assert_eq!(initial.lcdc, 0x91);
        assert_eq!(initial.stat, 0x01);
        assert_eq!(initial.scy, 0x00);
        assert_eq!(initial.scx, 0x00);
        assert_eq!(initial.lyc, 0x00);
    }

    #[test]
    fn ppu_public_wave_gate_matches_full_s3_captured_windows_from_replayed_writes() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent");
        let traces = [
            (
                "hblank_ly_scx_timing-GS",
                "acceptance__ppu__hblank_ly_scx_timing-GS__dmg.tsv",
            ),
            (
                "intr_2_0_timing",
                "acceptance__ppu__intr_2_0_timing__dmg.tsv",
            ),
            (
                "lcdon_timing-GS",
                "acceptance__ppu__lcdon_timing-GS__dmg.tsv",
            ),
            (
                "lcdon_write_timing-GS",
                "acceptance__ppu__lcdon_write_timing-GS__dmg.tsv",
            ),
        ];
        let events = [
            "stat_sample",
            "mode2_irq_prepare",
            "mode2_enter",
            "mode3_enter",
            "mode0_enter",
            "frame_vblank",
            "vblank_irq_edge",
            "stat_irq_edge",
            "lcd_off",
            "lcd_on_line0_oam_prelude",
            "mode3_enter_line0",
        ];
        let observables = [
            Observable::PpuModeEdge,
            Observable::PpuStat,
            Observable::PpuStatSources,
            Observable::PpuIrqEdge,
            Observable::PpuLcdOn,
            Observable::PpuLyc,
        ];

        for (rom, file) in traces {
            let path = workspace.join("reference/goldens/v2").join(file);
            if !path.exists() {
                eprintln!("skip: {path:?} absent");
                continue;
            }
            for event in events {
                let selection = GoldenSelection::new().kind("ppu_public").event(event);
                if GoldenV2Reader::open(&path)
                    .expect("v2 reader opens")
                    .filter_selection(selection.clone())
                    .next()
                    .is_none()
                {
                    continue;
                }
                for observable in observables {
                    if observable == Observable::PpuModeEdge && event == "stat_sample" {
                        continue;
                    }
                    if observable == Observable::PpuModeEdge
                        && matches!(event, "frame_vblank" | "vblank_irq_edge" | "stat_irq_edge")
                    {
                        continue;
                    }
                    if observable == Observable::PpuIrqEdge
                        && !matches!(event, "frame_vblank" | "vblank_irq_edge" | "stat_irq_edge")
                    {
                        continue;
                    }
                    if observable == Observable::PpuLcdOn
                        && !matches!(event, "lcd_off" | "lcd_on_line0_oam_prelude")
                    {
                        continue;
                    }
                    let actual = replay_ppu_public_observable(
                        &path,
                        GbModel::DmgB,
                        event,
                        observable,
                        selection.clone(),
                    )
                    .expect("PPU-public replay emits selected observable from writes+table");

                    assert_golden_edges!(
                        actual,
                        GoldenV2Reader::open(&path).expect("v2 reader opens"),
                        observable,
                        rom,
                        selection.clone(),
                    );
                }
            }
        }
    }

    #[test]
    fn ppu_public_wave_gate_is_falsifiable_when_mode0_entry_moves_by_one_tick() {
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
            .event("mode0_enter")
            .frames(60..=60)
            .lines(0..=143);
        let actual = replay_ppu_public_observable_with_table_perturbation(
            &path,
            GbModel::DmgB,
            "mode0_enter",
            Observable::PpuModeEdge,
            selection.clone(),
            "dmg_b_mode0_enter_tick",
            1,
        )
        .expect("perturbed replay runs");

        let err = assert_golden_edges(
            actual,
            GoldenV2Reader::open(&path).expect("v2 reader opens"),
            Observable::PpuModeEdge,
            "hblank_ly_scx_timing-GS perturbed mode0",
            selection,
        )
        .expect_err("+1 tick mode0 table perturbation must fail wave gate");
        assert!(err.to_string().contains("first divergence"));
    }

    #[test]
    fn ppu_public_accepts_slice1_register_writes() {
        let mut ppu = PpuPublic::new(GbModel::DmgB, Time::ZERO, 0);

        ppu.write_register(PpuRegisterWrite {
            time: Time::from_subphases(10),
            addr: 0xFF40,
            value: 0x80,
        });
        ppu.write_register(PpuRegisterWrite {
            time: Time::from_subphases(11),
            addr: 0xFF41,
            value: 0x78,
        });
        ppu.write_register(PpuRegisterWrite {
            time: Time::from_subphases(12),
            addr: 0xFF45,
            value: 0x42,
        });

        assert_eq!(ppu.lcdc(), 0x80);
        assert_eq!(ppu.stat(), 0xF8);
        assert_eq!(ppu.lyc(), 0x42);
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
