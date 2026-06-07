//! File -> Debug VRAM viewer (ticket rubc-e4u).
//!
//! A read-only egui window that visualizes the live PPU VRAM as the game runs,
//! modeled on gobc's `internal/windows/vramview.go`. Two switchable views:
//!
//! - **BG Tiles (raw):** the 384 tiles in `$8000-$97FF` decoded straight from
//!   VRAM as a 16x24 tile sheet, independent of any tilemap.
//! - **Tilemap (rendered BG):** the 32x32 background tilemap resolved through
//!   the current tile-addressing mode (`$8000` unsigned / `$8800` signed) and
//!   BG-map base (`$9800` / `$9C00`), with the SCX/SCY viewport overlaid.
//!
//! Addressing/base/bank are independently toggleable (mirroring gobc's T/B/V
//! keys) so the same VRAM can be inspected under either interpretation. The
//! window also shows the decoded LCDC background/window state.
//!
//! This module is strictly presentation: it reads a [`VramDebugSnapshot`]
//! copied out of the bus once per frame and never mutates emulator state.

use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use rubc_core::machine::Machine;
use std::hash::{Hash, Hasher};

/// Bytes of VRAM per bank (`$8000-$9FFF`).
const VRAM_BANK_LEN: usize = 0x2000;
/// Tiles in the `$8000-$97FF` tile-data block (384 * 16 bytes = `0x1800`).
const TILE_COUNT: usize = 384;
/// Tile sheet layout: 16 columns x 24 rows = 384 tiles.
const SHEET_COLS: usize = 16;
const SHEET_ROWS: usize = TILE_COUNT / SHEET_COLS;
/// Tile sheet image dimensions in pixels (8x8 tiles).
const SHEET_W: usize = SHEET_COLS * 8;
const SHEET_H: usize = SHEET_ROWS * 8;
/// Background tilemap is 32x32 tiles -> 256x256 px.
const MAP_TILES: usize = 32;
const MAP_W: usize = MAP_TILES * 8;
const MAP_H: usize = MAP_TILES * 8;
/// Visible screen extent (the viewport rectangle overlaid on the tilemap).
const VIEW_W: usize = 160;
const VIEW_H: usize = 144;

/// The 4 DMG shades (lightest -> darkest) as RGB. Kept in sync with
/// `capture::DMG_SHADES` so the debug view matches the on-screen palette.
const DMG_SHADES: [[u8; 3]; 4] = [
    [0xE0, 0xF8, 0xD0],
    [0x88, 0xC0, 0x70],
    [0x34, 0x68, 0x56],
    [0x08, 0x18, 0x20],
];

/// A read-only copy of the VRAM-relevant PPU state, taken from the bus once per
/// frame and handed to the viewer. Copying (~16 KiB) is cheap and keeps the
/// viewer from borrowing the `Machine` across the egui closure.
#[derive(Clone)]
pub struct VramDebugSnapshot {
    /// Both VRAM banks (`$8000-$9FFF`). Bank 1 holds CGB tile attributes.
    pub vram: [[u8; VRAM_BANK_LEN]; 2],
    pub lcdc: u8,
    pub bgp: u8,
    /// CGB background palette RAM (8 palettes x 4 colors x RGB555 LE).
    pub bg_palette_ram: [u8; 64],
    pub scx: u8,
    pub scy: u8,
    pub wx: u8,
    pub wy: u8,
    pub cgb: bool,
}

impl VramDebugSnapshot {
    /// Extract a read-only snapshot from the machine bus. Pure reads only --
    /// never mutates emulator state and has no timing side effects.
    pub fn capture(machine: &Machine) -> Self {
        let bus = &machine.bus;
        Self {
            vram: bus.vram,
            lcdc: bus.ppu.read_lcdc(),
            bgp: bus.dmg_bgp(),
            bg_palette_ram: bus.bg_palette_ram,
            scx: bus.ppu.read_scx(),
            scy: bus.ppu.read_scy(),
            wx: bus.ppu.read_wx(),
            wy: bus.ppu.read_wy(),
            cgb: bus.cgb.cgb_mode,
        }
    }
}

/// Decode one 8x8 2bpp Game Boy tile (16 bytes: 8 rows of low/high plane pairs)
/// into 64 color indices (0..=3), row-major (row 0 first, left pixel first).
///
/// Each row is two bytes: the low bit-plane then the high bit-plane. Within a
/// byte, bit 7 is the leftmost pixel. A pixel's 2-bit color is `(high<<1)|low`.
pub fn decode_tile_2bpp(tile: &[u8; 16]) -> [u8; 64] {
    let mut out = [0u8; 64];
    for row in 0..8 {
        let low = tile[row * 2];
        let high = tile[row * 2 + 1];
        for col in 0..8 {
            let bit = 7 - col;
            let lo = (low >> bit) & 0x01;
            let hi = (high >> bit) & 0x01;
            out[row * 8 + col] = (hi << 1) | lo;
        }
    }
    out
}

/// Map a DMG color index (0..=3) through BGP to a shade color.
fn dmg_color(bgp: u8, color: u8) -> Color32 {
    let shade = (bgp >> ((color & 0x03) * 2)) & 0x03;
    let [r, g, b] = DMG_SHADES[shade as usize];
    Color32::from_rgb(r, g, b)
}

/// Map a CGB (palette, color) pair through BG palette RAM (RGB555 LE) to a
/// color, expanding each 5-bit channel to 8-bit (`(c<<3)|(c>>2)`).
fn cgb_color(palette_ram: &[u8; 64], palette: u8, color: u8) -> Color32 {
    let base = (palette as usize & 0x07) * 8 + (color as usize & 0x03) * 2;
    let rgb555 = (palette_ram[base] as u16) | ((palette_ram[base + 1] as u16) << 8);
    let expand = |c: u16| -> u8 {
        let c = (c & 0x1F) as u8;
        (c << 3) | (c >> 2)
    };
    Color32::from_rgb(expand(rgb555), expand(rgb555 >> 5), expand(rgb555 >> 10))
}

/// Which picture the debug window is currently showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    /// Raw 384-tile sheet from `$8000-$97FF`.
    Tiles,
    /// 32x32 background tilemap, resolved + palette-applied.
    Tilemap,
}

/// The File -> Debug VRAM viewer window state.
pub struct VramView {
    /// Whether the debug window is shown. Toggled by the File -> Debug menu.
    pub open: bool,
    view: ViewMode,
    /// Tile addressing: `true` = `$8000` unsigned, `false` = `$8800` signed.
    tile_8000: bool,
    /// BG-map base: `false` = `$9800`, `true` = `$9C00`.
    map_9c00: bool,
    /// Raw-tile VRAM bank to display (0/1; bank 1 only meaningful on CGB).
    bank: usize,
    /// Integer upscale factor for the rendered texture.
    scale: usize,
    show_grid: bool,
    show_viewport: bool,
    tex: Option<TextureHandle>,
    tex_size: [usize; 2],
    /// Signature of the last-built texture; rebuild only when it changes.
    sig: u64,
    snapshot: Option<VramDebugSnapshot>,
}

impl Default for VramView {
    fn default() -> Self {
        Self {
            open: false,
            view: ViewMode::Tiles,
            tile_8000: true,
            map_9c00: false,
            bank: 0,
            scale: 2,
            show_grid: true,
            show_viewport: true,
            tex: None,
            tex_size: [0, 0],
            sig: 0,
            snapshot: None,
        }
    }
}

impl VramView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle the window open/closed (wired to File -> Debug).
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Receive this frame's read-only VRAM snapshot.
    pub fn set_snapshot(&mut self, snapshot: VramDebugSnapshot) {
        self.snapshot = Some(snapshot);
    }

    /// Compute a cheap signature of everything that affects the rendered
    /// texture, so it is only rebuilt when the live state (or a toggle) changes.
    fn signature(&self, snap: &VramDebugSnapshot) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.view == ViewMode::Tiles).hash(&mut h);
        self.tile_8000.hash(&mut h);
        self.map_9c00.hash(&mut h);
        self.bank.hash(&mut h);
        snap.cgb.hash(&mut h);
        snap.lcdc.hash(&mut h);
        snap.bgp.hash(&mut h);
        snap.bg_palette_ram.hash(&mut h);
        match self.view {
            ViewMode::Tiles => {
                // Only the displayed bank's tile-data block matters.
                snap.vram[self.bank & 1][..TILE_COUNT * 16].hash(&mut h);
            }
            ViewMode::Tilemap => {
                // Tilemap pulls from tile data + the selected map + (CGB) attrs.
                snap.vram[0].hash(&mut h);
                if snap.cgb {
                    snap.vram[1].hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// Build the `ColorImage` for the current view from the snapshot.
    fn build_image(&self, snap: &VramDebugSnapshot) -> ColorImage {
        match self.view {
            ViewMode::Tiles => self.build_tiles_image(snap),
            ViewMode::Tilemap => self.build_tilemap_image(snap),
        }
    }

    fn build_tiles_image(&self, snap: &VramDebugSnapshot) -> ColorImage {
        let mut img = ColorImage::filled([SHEET_W, SHEET_H], Color32::BLACK);
        let bank = &snap.vram[self.bank & 1];
        for tile_idx in 0..TILE_COUNT {
            let tx = (tile_idx % SHEET_COLS) * 8;
            let ty = (tile_idx / SHEET_COLS) * 8;
            let off = tile_idx * 16;
            let tile: &[u8; 16] = bank[off..off + 16].try_into().unwrap();
            let px = decode_tile_2bpp(tile);
            for row in 0..8 {
                for col in 0..8 {
                    let color = px[row * 8 + col];
                    let c = if snap.cgb {
                        // Raw view has no map context: show CGB tiles via BG
                        // palette 0.
                        cgb_color(&snap.bg_palette_ram, 0, color)
                    } else {
                        dmg_color(snap.bgp, color)
                    };
                    img[(tx + col, ty + row)] = c;
                }
            }
        }
        img
    }

    fn build_tilemap_image(&self, snap: &VramDebugSnapshot) -> ColorImage {
        let mut img = ColorImage::filled([MAP_W, MAP_H], Color32::BLACK);
        let map_base = if self.map_9c00 { 0x1C00 } else { 0x1800 };
        for cy in 0..MAP_TILES {
            for cx in 0..MAP_TILES {
                let cell = cy * MAP_TILES + cx;
                let tile_no = snap.vram[0][map_base + cell];
                let attr = if snap.cgb {
                    snap.vram[1][map_base + cell]
                } else {
                    0
                };
                let data_bank = if snap.cgb { (attr >> 3) & 0x01 } else { 0 } as usize;
                let palette = attr & 0x07;
                let xflip = snap.cgb && attr & 0x20 != 0;
                let yflip = snap.cgb && attr & 0x40 != 0;
                let data_off = if self.tile_8000 {
                    tile_no as usize * 16
                } else {
                    (0x1000i32 + (tile_no as i8 as i32) * 16) as usize
                };
                let tile: &[u8; 16] = snap.vram[data_bank][data_off..data_off + 16]
                    .try_into()
                    .unwrap();
                let px = decode_tile_2bpp(tile);
                for row in 0..8 {
                    for col in 0..8 {
                        let srow = if yflip { 7 - row } else { row };
                        let scol = if xflip { 7 - col } else { col };
                        let color = px[srow * 8 + scol];
                        let c = if snap.cgb {
                            cgb_color(&snap.bg_palette_ram, palette, color)
                        } else {
                            dmg_color(snap.bgp, color)
                        };
                        img[(cx * 8 + col, cy * 8 + row)] = c;
                    }
                }
            }
        }
        img
    }

    /// Apply the keyboard shortcuts (only while the window is open): mirrors
    /// gobc -- T toggles tile addressing, B toggles BG-map base, V toggles the
    /// raw-tile bank, Tab switches between the tiles and tilemap views.
    fn handle_shortcuts(&mut self, ctx: &Context) {
        let (t, b, v, tab) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::T),
                i.key_pressed(egui::Key::B),
                i.key_pressed(egui::Key::V),
                i.key_pressed(egui::Key::Tab),
            )
        });
        if t {
            self.tile_8000 = !self.tile_8000;
        }
        if b {
            self.map_9c00 = !self.map_9c00;
        }
        if v {
            self.bank ^= 1;
        }
        if tab {
            self.view = match self.view {
                ViewMode::Tiles => ViewMode::Tilemap,
                ViewMode::Tilemap => ViewMode::Tiles,
            };
        }
    }

    /// Draw the debug window. No-op when closed.
    pub fn ui(&mut self, ctx: &Context) {
        if !self.open {
            return;
        }
        self.handle_shortcuts(ctx);

        let Some(snap) = self.snapshot.clone() else {
            return;
        };

        // Rebuild the texture only when the signature (state/toggles) changes.
        let sig = self.signature(&snap);
        if self.tex.is_none() || sig != self.sig {
            let image = self.build_image(&snap);
            self.tex_size = image.size;
            let handle = ctx.load_texture("rubc-vram-debug", image, TextureOptions::NEAREST);
            self.tex = Some(handle);
            self.sig = sig;
        }

        let title = self.title(&snap);
        let mut open = self.open;
        egui::Window::new(title)
            .open(&mut open)
            .resizable(true)
            .default_width(560.0)
            .show(ctx, |ui| self.window_body(ui, &snap));
        self.open = open;
    }

    fn title(&self, snap: &VramDebugSnapshot) -> String {
        let view = match self.view {
            ViewMode::Tiles => "BG Tiles",
            ViewMode::Tilemap => "Tilemap",
        };
        let bg = if self.map_9c00 { 0x9C00 } else { 0x9800 };
        let tile = if self.tile_8000 { 0x8000 } else { 0x8800 };
        format!(
            "VRAM Debug | {view} | BG:${bg:04X} Tile:${tile:04X} | Unsigned:{} | {}",
            self.tile_8000,
            if snap.cgb { "CGB" } else { "DMG" }
        )
    }

    fn window_body(&mut self, ui: &mut egui::Ui, snap: &VramDebugSnapshot) {
        // --- View switch + addressing/base/bank toggles (mirror gobc keys) ---
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.view, ViewMode::Tiles, "BG Tiles (raw)");
            ui.selectable_value(&mut self.view, ViewMode::Tilemap, "Tilemap (rendered)");
            ui.separator();
            ui.label("Tab: switch view");
        });
        ui.horizontal(|ui| {
            let tile_label = if self.tile_8000 {
                "Tile data: $8000 (unsigned) [T]"
            } else {
                "Tile data: $8800 (signed) [T]"
            };
            if ui.button(tile_label).clicked() {
                self.tile_8000 = !self.tile_8000;
            }
            let bg_label = if self.map_9c00 {
                "BG map: $9C00 [B]"
            } else {
                "BG map: $9800 [B]"
            };
            if ui.button(bg_label).clicked() {
                self.map_9c00 = !self.map_9c00;
            }
            if snap.cgb && ui.button(format!("VRAM bank: {} [V]", self.bank)).clicked() {
                self.bank ^= 1;
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_grid, "8x8 grid");
            if self.view == ViewMode::Tilemap {
                ui.checkbox(&mut self.show_viewport, "Viewport (SCX/SCY)");
            }
            ui.add(egui::Slider::new(&mut self.scale, 1..=4).text("scale"));
        });

        ui.separator();
        self.background_state_panel(ui, snap);
        ui.separator();

        // --- The rendered VRAM texture, scaled up, with overlays ---
        let Some(tex) = &self.tex else {
            return;
        };
        let [iw, ih] = self.tex_size;
        let scale = self.scale.max(1) as f32;
        let size = egui::vec2(iw as f32 * scale, ih as f32 * scale);
        let st = egui::load::SizedTexture::new(tex.id(), size);
        let resp = ui.add(egui::Image::new(st));
        let rect = resp.rect;
        let painter = ui.painter_at(rect);

        if self.show_grid {
            self.draw_grid(&painter, rect, iw, ih, scale);
        }
        if self.view == ViewMode::Tilemap && self.show_viewport {
            self.draw_viewport(&painter, rect, scale, snap);
        }
    }

    fn background_state_panel(&self, ui: &mut egui::Ui, snap: &VramDebugSnapshot) {
        let l = snap.lcdc;
        let on = |bit: u8| (l >> bit) & 1 == 1;
        ui.label(format!("LCDC ${l:02X}"));
        ui.label(format!(
            "  LCD:{}  BG/Win:{}  Win:{}  Obj:{}  ObjSize:{}",
            yn(on(7)),
            yn(on(0)),
            yn(on(5)),
            yn(on(1)),
            if on(2) { "8x16" } else { "8x8" },
        ));
        ui.label(format!(
            "  BG map:{}  Win map:{}  Tile data:{}",
            if on(3) { "$9C00" } else { "$9800" },
            if on(6) { "$9C00" } else { "$9800" },
            if on(4) { "$8000" } else { "$8800" },
        ));
        ui.label(format!(
            "SCX:{:3} SCY:{:3}   WX:{:3} WY:{:3}   BGP:${:02X}   {}",
            snap.scx,
            snap.scy,
            snap.wx,
            snap.wy,
            snap.bgp,
            if snap.cgb { "CGB color" } else { "DMG" },
        ));
    }

    fn draw_grid(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        iw: usize,
        ih: usize,
        scale: f32,
    ) {
        let stroke = egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 0, 0, 64));
        let step = 8.0 * scale;
        let mut x = rect.min.x;
        while x <= rect.min.x + iw as f32 * scale + 0.5 {
            painter.line_segment(
                [
                    egui::pos2(x, rect.min.y),
                    egui::pos2(x, rect.min.y + ih as f32 * scale),
                ],
                stroke,
            );
            x += step;
        }
        let mut y = rect.min.y;
        while y <= rect.min.y + ih as f32 * scale + 0.5 {
            painter.line_segment(
                [
                    egui::pos2(rect.min.x, y),
                    egui::pos2(rect.min.x + iw as f32 * scale, y),
                ],
                stroke,
            );
            y += step;
        }
    }

    /// Overlay the 160x144 viewport rectangle at (SCX, SCY), wrapping around the
    /// 256x256 tilemap (drawn as up to four offset copies; the painter clips).
    fn draw_viewport(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        scale: f32,
        snap: &VramDebugSnapshot,
    ) {
        let stroke = egui::Stroke::new(2.0, Color32::from_rgb(255, 64, 64));
        let sx = snap.scx as f32;
        let sy = snap.scy as f32;
        for (ox, oy) in [
            (0.0, 0.0),
            (-(MAP_W as f32), 0.0),
            (0.0, -(MAP_H as f32)),
            (-(MAP_W as f32), -(MAP_H as f32)),
        ] {
            let min = egui::pos2(
                rect.min.x + (sx + ox) * scale,
                rect.min.y + (sy + oy) * scale,
            );
            let vp = egui::Rect::from_min_size(
                min,
                egui::vec2(VIEW_W as f32 * scale, VIEW_H as f32 * scale),
            );
            painter.rect_stroke(vp, 0.0, stroke, egui::StrokeKind::Middle);
        }
    }
}

fn yn(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_2bpp_mixes_low_and_high_planes() {
        // Row 0: low=0b1010_0000, high=0b1100_0000.
        // pixel0: lo=1 hi=1 -> 3; pixel1: lo=0 hi=1 -> 2;
        // pixel2: lo=1 hi=0 -> 1; pixel3: lo=0 hi=0 -> 0; rest 0.
        let mut tile = [0u8; 16];
        tile[0] = 0b1010_0000;
        tile[1] = 0b1100_0000;
        let px = decode_tile_2bpp(&tile);
        assert_eq!(&px[0..4], &[3, 2, 1, 0]);
        assert_eq!(&px[4..8], &[0, 0, 0, 0]);
        // Untouched rows decode to all-zero.
        assert!(px[8..].iter().all(|&c| c == 0));
    }

    #[test]
    fn decode_2bpp_solid_color_3() {
        // All planes set -> every pixel is color 3.
        let tile = [0xFFu8; 16];
        let px = decode_tile_2bpp(&tile);
        assert!(px.iter().all(|&c| c == 3));
    }

    #[test]
    fn decode_2bpp_bit_order_is_msb_left() {
        // Only bit 0 set in the low plane of row 0 -> rightmost pixel = color 1.
        let mut tile = [0u8; 16];
        tile[0] = 0b0000_0001;
        let px = decode_tile_2bpp(&tile);
        assert_eq!(px[7], 1);
        assert_eq!(&px[0..7], &[0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn dmg_color_maps_through_bgp() {
        // BGP 0xE4 = 11_10_01_00: color i -> shade i (identity).
        assert_eq!(dmg_color(0xE4, 0), Color32::from_rgb(0xE0, 0xF8, 0xD0));
        assert_eq!(dmg_color(0xE4, 3), Color32::from_rgb(0x08, 0x18, 0x20));
        // BGP 0x00: every color maps to shade 0 (lightest).
        assert_eq!(dmg_color(0x00, 2), Color32::from_rgb(0xE0, 0xF8, 0xD0));
    }
}
