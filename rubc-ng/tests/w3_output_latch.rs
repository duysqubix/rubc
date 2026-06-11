use rubc_ng::{
    assert_lcd_output_palette_golden, assert_lcd_output_palette_golden_with_perturbation,
    assert_lcd_output_palette_golden_with_wrong_register, LcdOutputLatch, LcdPaletteSource,
    OutputRawPixel, PaletteWrite, Time,
};
use std::path::{Path, PathBuf};

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
        .join("reference/goldens")
        .join(name)
}

#[test]
fn w3_lcd_latch_applies_dmg_palette_from_raw_color_and_sampled_register() {
    let mut latch = LcdOutputLatch::dmg_default();
    latch.apply_write(PaletteWrite {
        time: Time::from_subphases(10),
        source: LcdPaletteSource::Bg,
        value: 0b11_10_01_00,
    });

    let pixel = latch
        .latch_pixel(OutputRawPixel {
            time: Time::from_subphases(10),
            ly: 0,
            x: 7,
            source: LcdPaletteSource::Bg,
            raw_color: 2,
        })
        .expect("pixel latches");

    assert_eq!(pixel.sampled_palette_value, 0b11_10_01_00);
    assert_eq!(pixel.final_color, 0b10);
}

#[test]
fn w3_lcd_output_palette_golden_oracle_matches_bgp_without_copying_applied_value() {
    for file in [
        "m3_bgp_change_ly000.tsv",
        "m3_bgp_change_ly072.tsv",
        "m3_bgp_change_sprites_ly058.tsv",
    ] {
        let path = golden(file);
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            continue;
        }
        assert_lcd_output_palette_golden(&path).unwrap_or_else(|err| panic!("{file}: {err}"));
    }
}

#[test]
fn w3_lcd_output_palette_golden_gate_fails_on_one_dot_latch_perturbation() {
    let path = golden("m3_bgp_change_ly000.tsv");
    if !path.exists() {
        eprintln!("skip: {path:?} absent");
        return;
    }

    let err = assert_lcd_output_palette_golden_with_perturbation(&path, 2)
        .expect_err("one SameBoy-dot late latch must fail the palette oracle");
    let diagnostic = err.to_string();
    assert!(diagnostic.contains("first LCD output palette divergence"));
    assert!(diagnostic.contains("machine palette"));
    assert!(diagnostic.contains("golden palette"));
}

#[test]
fn w3_lcd_output_palette_golden_gate_fails_on_wrong_palette_register_selection() {
    let path = golden("m3_bgp_change_ly000.tsv");
    if !path.exists() {
        eprintln!("skip: {path:?} absent");
        return;
    }

    let err = assert_lcd_output_palette_golden_with_wrong_register(&path)
        .expect_err("forcing BG pixels through OBP0 must fail the palette oracle");
    let diagnostic = err.to_string();
    assert!(diagnostic.contains("first LCD output palette divergence"));
    assert!(diagnostic.contains("machine source Obp0"));
    assert!(diagnostic.contains("golden source Bg"));
}
