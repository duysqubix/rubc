use rubc_ng::GoldenV2Reader;
use std::path::{Path, PathBuf};

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
        .join("reference/goldens/v2")
        .join(name)
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
