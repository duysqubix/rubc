use rubc_ng::{
    assert_bg_fetch_golden, assert_lcd_output_palette_golden, GoldenV2Reader, PpuInternal,
    ResolvedDmgPixel, SpritePalette, SpritePriorityMode,
};
use std::path::{Path, PathBuf};

fn golden_v2(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
        .join("reference/goldens/v2")
        .join(name)
}

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has workspace parent")
        .join("reference/goldens")
        .join(name)
}

#[test]
fn w4_window_restart_uses_window_tilemap_and_drops_bg_fine_x() {
    let path = golden_v2("m3_scy_change_ly000_v2.tsv");
    if !path.exists() {
        eprintln!("skip: {path:?} absent");
        return;
    }
    let state = GoldenV2Reader::read_vram_state(&path).expect("v2.3 state parses");
    let mut ppu = PpuInternal::for_test(
        0x91 | 0x20 | 0x40,
        0x33,
        0x05,
        0x07,
        0x00,
        state.vram,
        state.oam,
    );

    let restart = ppu
        .trigger_window_for_test(0, 0)
        .expect("WX=7/WY=0 triggers at screen x0");
    assert_eq!(
        restart.penalty_dots, 6,
        "window restart injects the hardware 6-dot fill penalty"
    );
    assert_eq!(
        restart.scx_low_bits_after_restart, 0,
        "window resume ignores BG SCX low 3 bits"
    );

    let fetch = ppu
        .fetch_next_tile_for_test(0)
        .expect("window tile fetch is machine-computed");
    assert_eq!(
        fetch.machine_addr, 0x1C00,
        "LCDC.6 window tilemap base must be used"
    );
    assert_eq!(fetch.fetcher_x, 0, "window restart resets fetcher X");
}

#[test]
fn w4_window_and_sprite_representative_fetch_rows_still_match_v23_vram_oracle() {
    for file in [
        "m3_window_timing_ly000_v2.tsv",
        "m3_window_timing_ly072_v2.tsv",
    ] {
        let path = golden_v2(file);
        if !path.exists() {
            eprintln!("skip: {path:?} absent");
            continue;
        }
        assert_bg_fetch_golden(&path).unwrap_or_else(|err| panic!("{file}: {err}"));
    }
}

#[test]
fn w4_window_oracle_fails_when_trigger_dot_is_wrong() {
    let path = golden_v2("m3_scy_change_ly000_v2.tsv");
    if !path.exists() {
        eprintln!("skip: {path:?} absent");
        return;
    }
    let state = GoldenV2Reader::read_vram_state(&path).expect("v2.3 state parses");
    let mut ppu = PpuInternal::for_test(0x91 | 0x20, 0, 0, 0x20, 0, state.vram, state.oam);

    assert!(
        ppu.trigger_window_for_test(0, 23).is_none(),
        "wrong pre-WX trigger dot must not silently activate window"
    );
    assert!(
        ppu.trigger_window_for_test(0, 25).is_some(),
        "first eligible dot triggers at WX-7"
    );
}

#[test]
fn w4_sprite_scan_selects_first_ten_by_oam_then_fetches_by_dmg_x_order() {
    let mut oam = [0u8; 0xA0];
    for (i, x) in [56, 40, 72, 24, 88, 16, 104, 8, 120, 32, 136, 48]
        .iter()
        .copied()
        .enumerate()
    {
        let base = i * 4;
        oam[base] = 16 + 58;
        oam[base + 1] = x;
        oam[base + 2] = i as u8;
        oam[base + 3] = if i % 2 == 0 { 0x00 } else { 0x10 };
    }

    let selected = PpuInternal::for_test(0x93, 0, 0, 0, 0, empty_vram(), oam)
        .selected_scanline_sprites_for_test(58);
    assert_eq!(
        selected.iter().map(|s| s.oam_index).collect::<Vec<_>>(),
        (0u8..10).collect::<Vec<_>>(),
        "OAM scan captures only first 10 in memory order"
    );

    let fetch = PpuInternal::sprite_fetch_order_for_test(&selected, SpritePriorityMode::DmgXOrder);
    assert_eq!(
        fetch.iter().map(|s| s.oam_index).collect::<Vec<_>>(),
        vec![7, 5, 3, 9, 1, 0, 2, 4, 6, 8],
        "DMG fetch/priority order is X ascending with OAM tie-break"
    );
}

#[test]
fn w4_sprite_priority_and_palette_resolve_at_same_output_dot_as_bg() {
    let sprite = PpuInternal::selected_sprite_for_test(16, 40, 3, 0x90, 0);
    assert_eq!(
        sprite.palette,
        SpritePalette::Obp1,
        "OBJ attr bit 4 selects OBP1"
    );

    assert_eq!(
        PpuInternal::resolve_dmg_obj_over_bg_for_test(1, false, 2, sprite),
        ResolvedDmgPixel::Bg,
        "OBJ attr bit 7 leaves nonzero BG in front"
    );
    assert_eq!(
        PpuInternal::resolve_dmg_obj_over_bg_for_test(0, false, 2, sprite),
        ResolvedDmgPixel::Obj(SpritePalette::Obp1),
        "BG color 0 lets OBJ source/palette win at the same dot"
    );
}

#[test]
fn w4_sprite_oracle_fails_on_corrupted_oam_byte_and_wrong_priority() {
    let mut oam = [0u8; 0xA0];
    oam[0] = 74;
    oam[1] = 20;
    oam[4] = 74;
    oam[5] = 12;

    let selected = PpuInternal::select_sprites_for_test(0x93, &oam, 58);
    assert_eq!(
        selected.iter().map(|s| s.oam_index).collect::<Vec<_>>(),
        vec![0, 1]
    );

    oam[4] = 0;
    let corrupted = PpuInternal::select_sprites_for_test(0x93, &oam, 58);
    assert_ne!(
        corrupted.iter().map(|s| s.oam_index).collect::<Vec<_>>(),
        vec![0, 1],
        "corrupting captured OAM Y must change selected-10 oracle"
    );

    let dmg = PpuInternal::sprite_fetch_order_for_test(&selected, SpritePriorityMode::DmgXOrder);
    let oam_order =
        PpuInternal::sprite_fetch_order_for_test(&selected, SpritePriorityMode::OamOrder);
    assert_ne!(
        dmg.iter().map(|s| s.oam_index).collect::<Vec<_>>(),
        oam_order.iter().map(|s| s.oam_index).collect::<Vec<_>>(),
        "wrong priority comparator must be falsifiable"
    );
}

#[test]
fn w4_output_latch_keeps_obj_and_bg_palette_sampling_on_same_dot() {
    let path = golden("m3_bgp_change_sprites_ly058.tsv");
    if !path.exists() {
        eprintln!("skip: {path:?} absent");
        return;
    }
    assert_lcd_output_palette_golden(&path)
        .expect("BG+OBJ samples are machine-latched at same dot");
}

fn empty_vram() -> rubc_ng::Vram {
    rubc_ng::Vram {
        bank0: [0; 0x2000],
        bank1: [0; 0x2000],
    }
}
