//! W8b·2b-fifo: behavior tests for the real pixel FIFO (rubc-d85o).
//!
//! These exercise the per-dot FIFO through PpuInternal's public surface:
//! `begin_drawing` (mode-3 line start) + `fifo_dot` (one drawing dot).
//! Geometry under test is the class the direct formula got wrong:
//!   - fine-X: the first SCX&7 BG pixels are discarded by the FIFO
//!     (Pan Docs pixel_fifo.md; SameBoy display.c position_in_line warmup)
//!   - window: activates at lcd_x == WX-7 with a fetcher restart and a
//!     window-line counter that increments at activation (TCAGBD; SameBoy
//!     display.c wx_triggered/window_y)
//!   - sprites: staged into the OBJ FIFO at their trigger X; DMG priority is
//!     first-fetched-wins (X order), CGB is lower-OAM-index-wins at merge.

use rubc_ng::{LcdPixelSource, PpuInternal, SpritePalette, Vram};

fn empty_vram() -> Vram {
    Vram {
        bank0: [0; 0x2000],
        bank1: [0; 0x2000],
    }
}

/// Pump one full line out of the FIFO, returning (x -> pixel) outputs in order.
fn render_line(ppu: &mut PpuInternal, cgb: bool, ly: u8) -> Vec<(usize, u8, LcdPixelSource)> {
    ppu.begin_drawing(ly);
    let mut out = Vec::new();
    for _ in 0..600 {
        if let Some(shipped) = ppu.fifo_dot(cgb, ly) {
            out.push((shipped.x, shipped.pixel.raw_color, shipped.pixel.source));
        }
        if out.len() == 160 {
            break;
        }
    }
    out
}

#[test]
fn fifo_discards_scx_fine_x_pixels_at_line_start() {
    let mut vram = empty_vram();
    // Tile 1 row 0: colors [0,1,2,3,0,1,2,3] (lo=0x55, hi=0x33).
    vram.bank0[0x0010] = 0x55;
    vram.bank0[0x0011] = 0x33;
    for col in 0..32 {
        vram.bank0[0x1800 + col] = 1;
    }
    // LCDC: LCD on, BG on, 8000 tile data, 9800 BG map. SCX=3.
    let mut ppu = PpuInternal::for_test(0x91, 0, 3, 0, 0, vram, [0u8; 0xA0]);

    let line = render_line(&mut ppu, false, 0);
    assert_eq!(line.len(), 160, "FIFO must ship exactly 160 pixels");
    for (i, (x, color, _)) in line.iter().enumerate() {
        assert_eq!(*x, i, "pixels ship in LCD column order");
        let expected = (((i + 3) & 7) & 3) as u8;
        assert_eq!(
            *color, expected,
            "SCX&7=3 discard shifts the BG pattern at x={i}"
        );
    }
}

#[test]
fn fifo_window_restarts_at_wx_trigger_and_advances_window_line_at_activation() {
    let mut vram = empty_vram();
    // BG tile 1 rows 0-1: all color 3.
    vram.bank0[0x0010] = 0xFF;
    vram.bank0[0x0011] = 0xFF;
    vram.bank0[0x0012] = 0xFF;
    vram.bank0[0x0013] = 0xFF;
    // Window tile 2: row 0 all color 2 (lo=0x00 hi=0xFF), row 1 all color 1.
    vram.bank0[0x0020] = 0x00;
    vram.bank0[0x0021] = 0xFF;
    vram.bank0[0x0022] = 0xFF;
    vram.bank0[0x0023] = 0x00;
    for col in 0..32 {
        vram.bank0[0x1800 + col] = 1; // BG map rows (SCY=0, two lines -> row 0)
        vram.bank0[0x1C00 + col] = 2; // window map row 0
    }
    // LCDC: LCD+BG on, window on (bit5), window map 9C00 (bit6), 8000 data.
    // WX=87 -> trigger at lcd_x 80. WY=0.
    let mut ppu = PpuInternal::for_test(0x91 | 0x20 | 0x40, 0, 0, 87, 0, vram, [0u8; 0xA0]);

    let line0 = render_line(&mut ppu, false, 0);
    assert_eq!(line0.len(), 160);
    assert_eq!(line0[79].1, 3, "x=79 is still BG");
    for (x, color, _) in line0.iter().skip(80) {
        assert_eq!(
            *color, 2,
            "x={x}: window row 0 replaces BG from WX-7 onwards"
        );
    }

    // Second line: the window line counter incremented AT ACTIVATION, so this
    // line fetches window row 1 (all color 1) -- not a screen-derived row.
    let line1 = render_line(&mut ppu, false, 1);
    assert_eq!(line1.len(), 160);
    assert_eq!(line1[79].1, 3, "x=79 is still BG on line 1");
    for (x, color, _) in line1.iter().skip(80) {
        assert_eq!(*color, 1, "x={x}: second activation uses window line 1");
    }
}

#[test]
fn fifo_window_at_wx7_ignores_bg_fine_x_scroll() {
    // Regression (Crystal town/zone banner jump): a WX=7 window is flush
    // against the left edge (trigger at lcd_x=0). Its restart clears the BG
    // FIFO, so the BG's line-start SCX&7 fine-scroll discard must NOT bleed
    // into the window pixels. If it does, the banner shifts horizontally by
    // SCX&7 as the player scrolls (the reported bug).
    let mut vram = empty_vram();
    // Window tile 2 row 0: colors [0,1,2,3,0,1,2,3] (lo=0x55, hi=0x33) so any
    // horizontal shift is detectable.
    vram.bank0[0x0020] = 0x55;
    vram.bank0[0x0021] = 0x33;
    // BG tile 1: all color 3 (would show through if the window mis-rendered).
    vram.bank0[0x0010] = 0xFF;
    vram.bank0[0x0011] = 0xFF;
    for col in 0..32 {
        vram.bank0[0x1800 + col] = 1; // BG map -> tile 1
        vram.bank0[0x1C00 + col] = 2; // window map -> tile 2
    }
    // LCD+BG on, window on (bit5), window map 9C00 (bit6), 8000 data. WX=7
    // (window from x=0), WY=0, SCX=3 (a non-zero BG fine-scroll).
    let mut ppu = PpuInternal::for_test(0x91 | 0x20 | 0x40, 0, 3, 7, 0, vram, [0u8; 0xA0]);
    let line = render_line(&mut ppu, false, 0);
    assert_eq!(line.len(), 160);
    for (x, color, source) in line.iter() {
        assert_eq!(
            *source,
            LcdPixelSource::Bg,
            "x={x}: window pixel is BG-source"
        );
        assert_eq!(
            *color,
            (*x as u8) & 3,
            "x={x}: WX=7 window column renders unshifted (no SCX&7 bleed)"
        );
    }
}

#[test]
fn fifo_sprite_merge_is_first_wins_on_dmg_and_oam_index_wins_on_cgb() {
    let mut vram = empty_vram();
    // Sprite tile 5: all color 1. Sprite tile 6: all color 2.
    vram.bank0[0x0050] = 0xFF;
    vram.bank0[0x0051] = 0x00;
    vram.bank0[0x0060] = 0x00;
    vram.bank0[0x0061] = 0xFF;
    // Same tiles in bank 1 for the CGB pass (attr bit3=0 keeps bank 0 anyway).

    let mut oam = [0u8; 0xA0];
    // OAM index 0: sprite B at x=24 (covers 16..24), tile 6, OBP1.
    oam[0] = 16;
    oam[1] = 24;
    oam[2] = 6;
    oam[3] = 0x10;
    // OAM index 1: sprite A at x=20 (covers 12..20), tile 5, OBP0.
    oam[4] = 16;
    oam[5] = 20;
    oam[6] = 5;
    oam[7] = 0x00;

    // DMG: X order wins the overlap (A at x=20 fetched before B at x=24).
    let mut dmg = PpuInternal::for_test(0x93, 0, 0, 0, 0, vram.clone(), oam);
    let line = render_line(&mut dmg, false, 0);
    assert_eq!(line[11].1, 0, "x=11 has no sprite");
    assert_eq!(
        (line[16].1, line[16].2),
        (1, LcdPixelSource::Obj(SpritePalette::Obp0)),
        "DMG overlap x=16: lower-X sprite A wins"
    );
    assert_eq!(
        (line[20].1, line[20].2),
        (2, LcdPixelSource::Obj(SpritePalette::Obp1)),
        "x=20 is past A; B shows"
    );

    // CGB: lower OAM index wins the overlap even though A was fetched first.
    let mut cgb = PpuInternal::for_test(0x93, 0, 0, 0, 0, vram, oam);
    let line = render_line(&mut cgb, true, 0);
    assert_eq!(
        line[16].1, 2,
        "CGB overlap x=16: lower OAM index (B) wins at merge"
    );
}
