use rubc_ng::{assert_bg_fetch_golden, assert_bg_fetch_golden_with_perturbation, GoldenV2Reader};
use std::path::{Path, PathBuf};

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
        .join("reference/goldens/v2")
        .join(name)
}

#[test]
fn w2_real_bg_fetcher_matches_scy_and_lcdc_map_v23_fetch_rows() {
    for file in [
        "m3_scy_change_ly000_v2.tsv",
        "m3_scy_change_ly138_v2.tsv",
        "m3_lcdc_bg_map_change_ly000_v2.tsv",
        "m3_lcdc_bg_map_change_ly072_v2.tsv",
    ] {
        let path = golden(file);
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            continue;
        }
        assert_bg_fetch_golden(&path).unwrap_or_else(|err| panic!("{file}: {err}"));
    }
}

#[test]
fn golden_v2_reader_extracts_v23_vram_state_without_eager_trace_load() {
    let path = golden("m3_scy_change_ly000_v2.tsv");
    if !path.exists() {
        eprintln!("skip: {path:?} absent");
        return;
    }

    let state = GoldenV2Reader::read_vram_state(&path).expect("v2.3 VRAM state parses");

    assert_eq!(state.regs.lcdc, 0x93);
    assert_eq!(state.regs.scx, 0x00);
    assert_eq!(state.regs.scy, 0x01);
    assert_eq!(state.vram.read(0x1800, 0).expect("tile map byte"), 0x41);
    assert_eq!(state.vram.read(0x0410, 0).expect("tile low byte"), 0x3C);
}

#[test]
fn w2_bg_fetch_gate_is_falsifiable_with_first_divergence_diagnostic() {
    let path = golden("m3_lcdc_bg_map_change_ly000_v2.tsv");
    if !path.exists() {
        eprintln!("skip: {path:?} absent");
        return;
    }

    let err = assert_bg_fetch_golden_with_perturbation(&path, 0x1C01, 0xFF)
        .expect_err("corrupting captured VRAM must fail the independent fetch oracle");
    let diagnostic = err.to_string();
    assert!(diagnostic.contains("first BG fetch divergence"));
    assert!(diagnostic.contains("machine byte"));
    assert!(diagnostic.contains("golden byte"));
}
