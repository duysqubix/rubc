//! DMG PPU mode scheduler + pixel FIFO (tickets rubc-9d4, rubc-fde).
//!
//! This drives the PPU's per-dot state machine: modes 0/1/2/3 across 456 dots
//! per scanline and 154 scanlines per frame, the LY/LYC compare, the STAT
//! register, VBlank + STAT interrupts, VRAM/OAM blocking, and the DMG pixel
//! FIFO. The framebuffer stores raw 2-bit color indices (0..=3) before palette
//! application; palette mapping belongs to the frontend/CGB palette waves.
//!
//! **Scope:** DMG BG/window/sprite FIFO only. VRAM and OAM remain owned by the
//! bus; the PPU receives read-only slices each dot. CGB tile attributes, CGB
//! palettes, and exact OBJ fetch edge quirks are deferred to later waves.
//!
//! Reference: GBEDG `ppu/index.md`; Pan Docs `Rendering.md`, `STAT.md`,
//! `LCDC.md`, `Accessing_VRAM_and_OAM.md`, `Tile_Data.md`, `OAM.md`,
//! `Window.md`.

use super::stubs::Interrupts;

/// Visible pixels per scanline.
pub const SCREEN_WIDTH: usize = 160;
/// Visible scanlines per frame.
pub const SCREEN_HEIGHT: usize = 144;
/// Pixels in the raw framebuffer.
pub const FRAMEBUFFER_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

/// Dots per scanline (mode 2 + mode 3 + mode 0 always sum to this).
const DOTS_PER_LINE: u32 = 456;
/// Mode 2 (OAM scan) duration in dots.
const MODE2_DOTS: u32 = 80;
/// Baseline mode 3 length: 12-dot fetch startup + 160 visible pixels.
#[cfg(test)]
const BASE_MODE3_DOTS: u32 = 172;
/// Last visible scanline (LY 0..=143 are visible; 144..=153 are VBlank).
const LAST_VISIBLE_LINE: u8 = 143;
/// Last scanline before LY wraps back to 0.
const LAST_LINE: u8 = 153;
const FIFO_CAPACITY: usize = 8;
const MAX_SPRITES_PER_LINE: usize = 10;
const OAM_ENTRY_COUNT: usize = 40;

/// PPU mode (the low 2 bits of STAT).
pub mod mode {
    pub const HBLANK: u8 = 0; // mode 0
    pub const VBLANK: u8 = 1; // mode 1
    pub const OAM_SCAN: u8 = 2; // mode 2
    pub const DRAWING: u8 = 3; // mode 3
}

/// Interrupt bit positions (matching the `Interrupts` request API).
const INT_VBLANK: u8 = 0;
const INT_STAT: u8 = 1;

#[derive(Clone, Copy, Debug, Default)]
struct FifoPixel {
    color: u8,
    bg_priority: bool,
    occupied: bool,
}

#[derive(Clone, Copy, Debug)]
struct PixelFifo {
    pixels: [FifoPixel; FIFO_CAPACITY],
    len: usize,
}

impl Default for PixelFifo {
    fn default() -> Self {
        Self {
            pixels: [FifoPixel::default(); FIFO_CAPACITY],
            len: 0,
        }
    }
}

impl PixelFifo {
    fn clear(&mut self) {
        self.pixels = [FifoPixel::default(); FIFO_CAPACITY];
        self.len = 0;
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn push_bg_pixels(&mut self, colors: [u8; FIFO_CAPACITY]) {
        self.len = FIFO_CAPACITY;
        for (slot, color) in self.pixels.iter_mut().zip(colors) {
            *slot = FifoPixel {
                color: color & 0x03,
                bg_priority: false,
                occupied: true,
            };
        }
    }

    fn overlay_sprite_pixels(
        &mut self,
        colors: [u8; FIFO_CAPACITY],
        bg_priority: bool,
        first_visible_pixel: usize,
    ) {
        for (slot, color) in colors.iter().copied().skip(first_visible_pixel).enumerate() {
            if slot >= FIFO_CAPACITY {
                break;
            }
            if color == 0 || self.pixels[slot].occupied {
                continue;
            }
            self.pixels[slot] = FifoPixel {
                color: color & 0x03,
                bg_priority,
                occupied: true,
            };
            self.len = self.len.max(slot + 1);
        }
    }

    fn pop(&mut self) -> Option<FifoPixel> {
        if self.len == 0 {
            return None;
        }

        let pixel = self.pixels[0];
        for i in 1..self.len {
            self.pixels[i - 1] = self.pixels[i];
        }
        self.len -= 1;
        self.pixels[self.len] = FifoPixel::default();
        Some(pixel)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ScanlineSprite {
    y: u8,
    x: u8,
    tile: u8,
    attr: u8,
    oam_index: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum FetchStep {
    #[default]
    TileNo,
    TileDataLow,
    TileDataHigh,
    Push,
}

#[derive(Clone, Copy, Debug, Default)]
struct BgFetcher {
    step: FetchStep,
    step_ticks: u8,
    fetcher_x: u8,
    tile: u8,
    low: u8,
    high: u8,
    dummy_fetch_done: bool,
    window: bool,
}

impl BgFetcher {
    /// Full reset (window-trigger / scanline start): resets EVERYTHING including
    /// the internal X-position counter.
    fn reset(&mut self, window: bool, dummy_fetch_done: bool) {
        *self = Self {
            window,
            dummy_fetch_done,
            ..Self::default()
        };
    }

    /// Sprite-fetch reset: GBEDG specifies a sprite fetch resets the BG fetcher
    /// to step 1 and pauses it, but does NOT reset the internal X-position
    /// counter, the window flag, or the dummy-fetch state (only window fetching
    /// resets fetcher_x). Preserve those so BG tile fetches resume in column.
    fn reset_for_sprite(&mut self) {
        self.step = FetchStep::default();
        self.step_ticks = 0;
        self.tile = 0;
        self.low = 0;
        self.high = 0;
        // fetcher_x, window, dummy_fetch_done preserved.
    }
}

/// The DMG PPU mode scheduler and raw-pixel renderer.
///
/// The public fields `ly`, `mode`, and `dot_ticks` preserve the old `PpuStub`
/// interface so the bus tick loop and flight recorder need no changes.
pub struct Ppu {
    /// Total dot-ticks since power-on (diagnostic; was `PpuStub::dot_ticks`).
    pub dot_ticks: u64,
    /// Current scanline (LCDC Y, `$FF44`).
    pub ly: u8,
    /// Current mode (STAT bits 1-0).
    pub mode: u8,
    /// Raw 2-bit color-index framebuffer. Palettes are intentionally not applied.
    pub framebuffer: Box<[u8; FRAMEBUFFER_PIXELS]>,

    /// LCD master enable (LCDC bit 7).
    enabled: bool,
    /// Full LCDC register (`$FF40`).
    lcdc: u8,
    /// Dot counter within the current scanline (0..456).
    line_dot: u32,
    /// LY compare register (`$FF45`).
    lyc: u8,
    /// STAT interrupt-source-select bits (3-6), stored as written.
    stat_enables: u8,
    /// LYC == LY coincidence flag (STAT bit 2).
    coincidence: bool,
    /// Previous level of the ORed "STAT line" (for rising-edge IRQ detection).
    stat_line: bool,

    scy: u8,
    scx: u8,
    wy: u8,
    wx: u8,

    bg_fifo: PixelFifo,
    sprite_fifo: PixelFifo,
    bg_fetcher: BgFetcher,
    scanline_sprites: [ScanlineSprite; MAX_SPRITES_PER_LINE],
    scanline_sprite_count: usize,
    next_sprite: usize,
    oam_scan_index: usize,
    pending_sprite: Option<ScanlineSprite>,
    sprite_fetch_ticks: u8,
    sprite_idle_ticks: u8,
    lcd_x: usize,
    scx_discard: u8,
    window_y_condition: bool,
    window_line_counter: u8,
    window_active: bool,
    window_started_this_line: bool,
    drawing_dots: u32,
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            dot_ticks: 0,
            ly: 0,
            mode: mode::OAM_SCAN,
            framebuffer: Box::new([0; FRAMEBUFFER_PIXELS]),
            // LCD starts enabled with the post-boot LCDC ($91 = on, BG on, ...).
            enabled: true,
            lcdc: 0x91,
            line_dot: 0,
            lyc: 0,
            stat_enables: 0,
            coincidence: true, // LY=0, LYC=0 at power-on
            stat_line: false,
            scy: 0,
            scx: 0,
            wy: 0,
            wx: 0,
            bg_fifo: PixelFifo::default(),
            sprite_fifo: PixelFifo::default(),
            bg_fetcher: BgFetcher::default(),
            scanline_sprites: [ScanlineSprite::default(); MAX_SPRITES_PER_LINE],
            scanline_sprite_count: 0,
            next_sprite: 0,
            oam_scan_index: 0,
            pending_sprite: None,
            sprite_fetch_ticks: 0,
            sprite_idle_ticks: 0,
            lcd_x: 0,
            scx_discard: 0,
            window_y_condition: false,
            window_line_counter: 0,
            window_active: false,
            window_started_this_line: false,
            drawing_dots: 0,
        }
    }
}

impl Ppu {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- the per-dot scheduler ---------------------------------------------

    /// Advance one dot (one T-cycle). Called 4x per M-cycle (or 2x in CGB
    /// double-speed) from the bus tick loop. Raises VBlank / STAT interrupts via
    /// `irq`. The bus owns VRAM/OAM and passes the current DMG VRAM bank + OAM.
    pub fn tick_dot(
        &mut self,
        irq: &mut Interrupts,
        vram: &[u8; 0x2000],
        oam: &[u8; 0xA0],
    ) {
        self.dot_ticks += 1;
        if !self.enabled {
            return;
        }

        self.line_dot += 1;
        if self.line_dot >= DOTS_PER_LINE {
            self.start_next_scanline(irq);
            self.update_stat_line(irq);
            return;
        }

        if self.ly <= LAST_VISIBLE_LINE {
            match self.mode {
                mode::OAM_SCAN => {
                    self.tick_oam_scan(oam);
                    if self.line_dot >= MODE2_DOTS {
                        self.enter_drawing(irq);
                    }
                }
                mode::DRAWING => self.tick_drawing(vram, irq),
                _ => {}
            }
        }

        self.update_stat_line(irq);
    }

    fn start_next_scanline(&mut self, irq: &mut Interrupts) {
        self.line_dot = 0;
        self.ly = if self.ly >= LAST_LINE { 0 } else { self.ly + 1 };
        self.update_coincidence();

        if self.ly > LAST_VISIBLE_LINE {
            if self.ly == LAST_VISIBLE_LINE + 1 {
                self.window_y_condition = false;
                self.window_line_counter = 0;
            }
            self.set_mode(mode::VBLANK, irq);
        } else {
            self.set_mode(mode::OAM_SCAN, irq);
        }
    }

    fn set_mode(&mut self, new_mode: u8, irq: &mut Interrupts) {
        if new_mode == self.mode {
            return;
        }
        self.mode = new_mode;
        // VBlank interrupt fires once, when the PPU first enters mode 1
        // (i.e. at the start of line 144).
        if new_mode == mode::VBLANK {
            irq.request(INT_VBLANK);
        }
    }

    fn tick_oam_scan(&mut self, oam: &[u8; 0xA0]) {
        if self.line_dot == 1 {
            self.begin_oam_scan();
        }
        if self.line_dot == 0 || self.line_dot > MODE2_DOTS || !self.line_dot.is_multiple_of(2) {
            return;
        }

        let index = self.oam_scan_index;
        self.oam_scan_index += 1;
        if index >= OAM_ENTRY_COUNT {
            return;
        }

        let base = index * 4;
        let sprite = ScanlineSprite {
            y: oam[base],
            x: oam[base + 1],
            tile: oam[base + 2],
            attr: oam[base + 3],
            oam_index: index as u8,
        };
        // OAM scan selects by Y-coverage ONLY (Pan Docs: "the PPU only checks
        // the Y coordinate to select objects"). An X=0 sprite is off-screen but
        // STILL consumes one of the 10 per-line slots -- it is skipped later at
        // sprite-fetch time, not here.
        if self.sprite_covers_current_line(sprite)
            && self.scanline_sprite_count < MAX_SPRITES_PER_LINE
        {
            self.scanline_sprites[self.scanline_sprite_count] = sprite;
            self.scanline_sprite_count += 1;
        }
    }

    fn begin_oam_scan(&mut self) {
        self.scanline_sprite_count = 0;
        self.next_sprite = 0;
        self.oam_scan_index = 0;
        self.pending_sprite = None;
        self.sprite_fetch_ticks = 0;
        self.sprite_idle_ticks = 0;
        self.bg_fifo.clear();
        self.sprite_fifo.clear();
        self.window_active = false;
        self.window_started_this_line = false;
        if self.ly == self.wy {
            self.window_y_condition = true;
        }
    }

    fn sprite_covers_current_line(&self, sprite: ScanlineSprite) -> bool {
        let height = self.sprite_height() as i16;
        let ly = self.ly as i16 + 16;
        let y = sprite.y as i16;
        ly >= y && ly < y + height
    }

    fn enter_drawing(&mut self, irq: &mut Interrupts) {
        self.scanline_sprites[..self.scanline_sprite_count]
            .sort_by_key(|sprite| (sprite.x, sprite.oam_index));
        self.bg_fifo.clear();
        self.sprite_fifo.clear();
        self.bg_fetcher.reset(false, false);
        self.pending_sprite = None;
        self.sprite_fetch_ticks = 0;
        self.sprite_idle_ticks = 0;
        self.lcd_x = 0;
        self.scx_discard = self.scx & 0x07;
        self.window_active = false;
        self.window_started_this_line = false;
        self.drawing_dots = 0;
        self.set_mode(mode::DRAWING, irq);
    }

    fn tick_drawing(&mut self, vram: &[u8; 0x2000], irq: &mut Interrupts) {
        self.drawing_dots += 1;

        if self.sprite_idle_ticks > 0 {
            self.clock_bg_fetcher(vram);
            self.sprite_idle_ticks -= 1;
            return;
        }

        if self.pending_sprite.is_some() {
            self.advance_sprite_fetch(vram);
            return;
        }

        self.maybe_start_window();
        if self.try_start_sprite_fetch() {
            self.advance_sprite_fetch(vram);
            return;
        }

        self.clock_bg_fetcher(vram);
        if self.shift_pixel(irq) {
            self.maybe_start_window();
        }
    }

    fn finish_drawing(&mut self, irq: &mut Interrupts) {
        if self.window_started_this_line {
            self.window_line_counter = self.window_line_counter.wrapping_add(1);
        }
        self.bg_fifo.clear();
        self.sprite_fifo.clear();
        self.pending_sprite = None;
        self.sprite_fetch_ticks = 0;
        self.sprite_idle_ticks = 0;
        self.set_mode(mode::HBLANK, irq);
    }

    fn clock_bg_fetcher(&mut self, vram: &[u8; 0x2000]) {
        if self.bg_fetcher.step == FetchStep::Push {
            if self.bg_fifo.is_empty() {
                let colors = if self.lcdc & 0x01 == 0 {
                    [0; FIFO_CAPACITY]
                } else {
                    decode_2bpp(self.bg_fetcher.low, self.bg_fetcher.high, false)
                };
                self.bg_fifo.push_bg_pixels(colors);
                self.bg_fetcher.fetcher_x = self.bg_fetcher.fetcher_x.wrapping_add(1);
                self.bg_fetcher.step = FetchStep::TileNo;
                self.bg_fetcher.step_ticks = 0;
            }
            return;
        }

        self.bg_fetcher.step_ticks += 1;
        if self.bg_fetcher.step_ticks < 2 {
            return;
        }
        self.bg_fetcher.step_ticks = 0;

        match self.bg_fetcher.step {
            FetchStep::TileNo => {
                self.bg_fetcher.tile = self.fetch_bg_tile_no(vram);
                self.bg_fetcher.step = FetchStep::TileDataLow;
            }
            FetchStep::TileDataLow => {
                let addr = self.fetch_bg_tile_data_addr();
                self.bg_fetcher.low = read_vram(vram, addr);
                self.bg_fetcher.step = FetchStep::TileDataHigh;
            }
            FetchStep::TileDataHigh => {
                let addr = self.fetch_bg_tile_data_addr() + 1;
                self.bg_fetcher.high = read_vram(vram, addr);
                if self.bg_fetcher.dummy_fetch_done {
                    self.bg_fetcher.step = FetchStep::Push;
                } else {
                    // First high-byte completion on a scanline is the documented
                    // dummy fetch: reset to step 1, creating the 12-dot startup.
                    self.bg_fetcher.dummy_fetch_done = true;
                    self.bg_fetcher.step = FetchStep::TileNo;
                }
            }
            FetchStep::Push => unreachable!("push step handled before tick accounting"),
        }
    }

    fn fetch_bg_tile_no(&self, vram: &[u8; 0x2000]) -> u8 {
        let map_base = if self.bg_fetcher.window {
            if self.lcdc & 0x40 != 0 { 0x1C00 } else { 0x1800 }
        } else if self.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };

        let x_offset = if self.bg_fetcher.window {
            self.bg_fetcher.fetcher_x & 0x1F
        } else {
            self.bg_fetcher
                .fetcher_x
                .wrapping_add(self.scx / 8)
                & 0x1F
        };
        let y_offset = if self.bg_fetcher.window {
            32 * ((self.window_line_counter as usize / 8) & 0x1F)
        } else {
            32 * (((self.ly.wrapping_add(self.scy) as usize) / 8) & 0x1F)
        };
        let offset = (y_offset + x_offset as usize) & 0x03FF;
        read_vram(vram, map_base + offset)
    }

    fn fetch_bg_tile_data_addr(&self) -> usize {
        let row = if self.bg_fetcher.window {
            (self.window_line_counter & 0x07) as usize
        } else {
            (self.ly.wrapping_add(self.scy) & 0x07) as usize
        };

        if self.lcdc & 0x10 != 0 {
            self.bg_fetcher.tile as usize * 16 + row * 2
        } else {
            let signed_tile = self.bg_fetcher.tile as i8 as i16;
            (0x1000i16 + signed_tile * 16 + (row * 2) as i16) as usize
        }
    }

    fn maybe_start_window(&mut self) {
        if self.window_active || self.window_started_this_line {
            return;
        }
        if self.lcdc & 0x20 == 0 || self.lcdc & 0x01 == 0 || !self.window_y_condition {
            return;
        }

        let window_x = self.wx.saturating_sub(7) as usize;
        if self.lcd_x < window_x {
            return;
        }

        self.window_active = true;
        self.window_started_this_line = true;
        self.bg_fifo.clear();
        self.bg_fetcher.reset(true, true);
    }

    fn try_start_sprite_fetch(&mut self) -> bool {
        if self.lcdc & 0x02 == 0 {
            return false;
        }

        // Sprites are sorted by (x, oam_index). Skip any leading off-screen
        // sprites (X==0; they consumed a scan slot but are never drawn), then
        // the next sprite is ready iff its X reaches the current pixel.
        while self.next_sprite < self.scanline_sprite_count
            && self.scanline_sprites[self.next_sprite].x == 0
        {
            self.next_sprite += 1;
        }
        if self.next_sprite < self.scanline_sprite_count {
            let sprite = self.scanline_sprites[self.next_sprite];
            if sprite.x as usize <= self.lcd_x + 8 {
                self.next_sprite += 1;
                self.pending_sprite = Some(sprite);
                self.sprite_fetch_ticks = 6;
                // Sprite fetch resets and pauses the BG fetcher, but leaves any
                // already queued BG pixels in the FIFO.
                self.bg_fetcher.reset_for_sprite();
                return true;
            }
        }
        false
    }

    fn advance_sprite_fetch(&mut self, vram: &[u8; 0x2000]) {
        if self.sprite_fetch_ticks > 0 {
            self.sprite_fetch_ticks -= 1;
        }
        if self.sprite_fetch_ticks != 0 {
            return;
        }

        if let Some(sprite) = self.pending_sprite.take() {
            self.load_sprite_fifo(vram, sprite);
            // TODO(rubc-fde sprite-timing): calibrate the exact dot where OBJ
            // push and LCD shift overlap. The coarse GBEDG 6-remaining-pixel
            // idle penalty is modelled here.
            let remaining = self.bg_fifo.len().min(6) as u8;
            self.sprite_idle_ticks = 6 - remaining;
        }
    }

    fn load_sprite_fifo(&mut self, vram: &[u8; 0x2000], sprite: ScanlineSprite) {
        let addr = self.sprite_tile_data_addr(sprite);
        let low = read_vram(vram, addr);
        let high = read_vram(vram, addr + 1);
        let x_flip = sprite.attr & 0x20 != 0;
        let colors = decode_2bpp(low, high, x_flip);
        let first_visible = 8usize.saturating_sub(sprite.x as usize).min(7);
        self.sprite_fifo
            .overlay_sprite_pixels(colors, sprite.attr & 0x80 != 0, first_visible);
    }

    fn sprite_tile_data_addr(&self, sprite: ScanlineSprite) -> usize {
        let height = self.sprite_height();
        let mut row = self.ly.wrapping_add(16).wrapping_sub(sprite.y);
        if sprite.attr & 0x40 != 0 {
            row = height - 1 - row;
        }

        let tile = if height == 16 {
            (sprite.tile & 0xFE).wrapping_add(row / 8)
        } else {
            sprite.tile
        };
        tile as usize * 16 + (row as usize & 0x07) * 2
    }

    fn sprite_height(&self) -> u8 {
        if self.lcdc & 0x04 != 0 { 16 } else { 8 }
    }

    fn shift_pixel(&mut self, irq: &mut Interrupts) -> bool {
        let Some(bg_pixel) = self.bg_fifo.pop() else {
            return false;
        };

        if self.scx_discard > 0 {
            self.scx_discard -= 1;
            let _ = self.sprite_fifo.pop();
            return true;
        }

        let bg_color = if self.lcdc & 0x01 == 0 {
            0
        } else {
            bg_pixel.color & 0x03
        };
        let sprite_pixel = self.sprite_fifo.pop().unwrap_or_default();
        let final_color = if sprite_pixel.occupied
            && sprite_pixel.color != 0
            && !(sprite_pixel.bg_priority && bg_color != 0)
        {
            sprite_pixel.color & 0x03
        } else {
            bg_color
        };

        if self.ly <= LAST_VISIBLE_LINE && self.lcd_x < SCREEN_WIDTH {
            let index = self.ly as usize * SCREEN_WIDTH + self.lcd_x;
            self.framebuffer[index] = final_color;
        }
        self.lcd_x += 1;

        if self.lcd_x >= SCREEN_WIDTH {
            self.finish_drawing(irq);
        }
        true
    }

    /// The "STAT line": OR of the enabled STAT conditions. The STAT interrupt
    /// fires on the RISING edge of this line (transition-based, a.k.a. STAT
    /// blocking), NOT on its level.
    fn update_stat_line(&mut self, irq: &mut Interrupts) {
        let line = self.stat_line_level();
        if line && !self.stat_line {
            irq.request(INT_STAT);
        }
        self.stat_line = line;
    }

    fn stat_line_level(&self) -> bool {
        let e = self.stat_enables;
        let mode0 = (e & 0x08) != 0 && self.mode == mode::HBLANK;
        let mode1 = (e & 0x10) != 0 && self.mode == mode::VBLANK;
        let mode2 = (e & 0x20) != 0 && self.mode == mode::OAM_SCAN;
        let lyc = (e & 0x40) != 0 && self.coincidence;
        mode0 || mode1 || mode2 || lyc
    }

    fn update_coincidence(&mut self) {
        self.coincidence = self.ly == self.lyc;
    }

    // ---- register access ----------------------------------------------------

    /// Read LCDC (`$FF40`).
    pub fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    /// Write LCDC (`$FF40`). Toggling bit 7 turns the LCD on/off.
    pub fn write_lcdc(&mut self, value: u8, irq: &mut Interrupts) {
        let was_on = self.enabled;
        self.lcdc = value;
        self.enabled = value & 0x80 != 0;

        if was_on && !self.enabled {
            // LCD off: PPU stops, LY resets, mode -> 0, dot counter resets.
            // VRAM/OAM become fully accessible (see `vram_blocked`/`oam_blocked`).
            // The LYC coincidence flag is retained while the comparison clock is
            // stopped (mooneye stat_lyc_onoff); do not recompute it or clear the
            // STAT line here.
            self.ly = 0;
            self.line_dot = 0;
            self.mode = mode::HBLANK;
            self.bg_fifo.clear();
            self.sprite_fifo.clear();
        } else if !was_on && self.enabled {
            // LCD on: restart the frame from the top.
            // TODO(rubc-9d4 lcdon wave): the first line after enable starts in
            // mode 0 (not mode 2) and has special shorter timing. We restart in
            // mode 2 for now; lcdon_timing-GS is gated to that wave.
            self.ly = 0;
            self.line_dot = 0;
            self.mode = mode::OAM_SCAN;
            self.window_y_condition = false;
            self.window_line_counter = 0;
            self.bg_fifo.clear();
            self.sprite_fifo.clear();
            // Re-enabling resumes the comparison clock: recompute coincidence,
            // then let update_stat_line apply normal rising-edge detection
            // against the RETAINED stat_line. A condition that was already true
            // (and stays true) must NOT re-fire (mooneye stat_lyc_onoff); only a
            // genuine false->true transition raises STAT.
            self.update_coincidence();
            self.update_stat_line(irq);
        }
    }

    /// Read STAT (`$FF41`): enables (bits 3-6) | bit7=1 | coincidence<<2 | mode.
    pub fn read_stat(&self) -> u8 {
        0x80 | self.stat_enables | ((self.coincidence as u8) << 2) | self.mode
    }

    /// Write STAT (`$FF41`): only the interrupt-source-select bits (3-6) are
    /// writable; mode (1-0) and coincidence (2) are read-only.
    ///
    /// TODO(rubc-9d4 spurious-STAT wave): on DMG, writing STAT during
    /// OAM/HBlank/VBlank or while LYC=LY briefly forces the STAT line as if 0xFF
    /// were written, which can spuriously raise the STAT IRQ. That quirk is not
    /// modelled here (it needs sub-write timing); only the enable bits update.
    pub fn write_stat(&mut self, value: u8, irq: &mut Interrupts) {
        self.stat_enables = value & 0x78;
        // A newly-enabled source may make the STAT line rise immediately.
        self.update_stat_line(irq);
    }

    /// Read LY (`$FF44`).
    pub fn read_ly(&self) -> u8 {
        self.ly
    }

    /// Read LYC (`$FF45`).
    pub fn read_lyc(&self) -> u8 {
        self.lyc
    }

    /// Write LYC (`$FF45`). While the LCD is on this recomputes coincidence and
    /// may raise STAT; while the LCD is off the comparison clock is stopped, so
    /// the value is stored but coincidence is NOT recomputed and no STAT fires
    /// (mooneye stat_lyc_onoff) -- it settles on re-enable.
    pub fn write_lyc(&mut self, value: u8, irq: &mut Interrupts) {
        self.lyc = value;
        if self.enabled {
            self.update_coincidence();
            self.update_stat_line(irq);
        }
    }

    pub fn read_scy(&self) -> u8 {
        self.scy
    }

    pub fn write_scy(&mut self, value: u8) {
        self.scy = value;
    }

    pub fn read_scx(&self) -> u8 {
        self.scx
    }

    pub fn write_scx(&mut self, value: u8) {
        self.scx = value;
    }

    pub fn read_wy(&self) -> u8 {
        self.wy
    }

    pub fn write_wy(&mut self, value: u8) {
        self.wy = value;
    }

    pub fn read_wx(&self) -> u8 {
        self.wx
    }

    pub fn write_wx(&mut self, value: u8) {
        self.wx = value;
    }

    // ---- VRAM / OAM access gating -------------------------------------------

    /// VRAM (`$8000-$9FFF`) is inaccessible during mode 3 (returns 0xFF / writes
    /// dropped). When the LCD is off, VRAM is always accessible.
    pub fn vram_blocked(&self) -> bool {
        self.enabled && self.mode == mode::DRAWING
    }

    /// OAM (`$FE00-$FE9F`) is inaccessible during modes 2 and 3. When the LCD is
    /// off, OAM is always accessible.
    pub fn oam_blocked(&self) -> bool {
        self.enabled && (self.mode == mode::OAM_SCAN || self.mode == mode::DRAWING)
    }

    /// CGB palette RAM (BCPD/OCPD, `$FF69`/`$FF6B`) is inaccessible during mode 3
    /// (writes fail, reads return garbage), same window as VRAM. This predicate
    /// is the hook the CGB palette wave (rubc-5a0) uses to gate its palette RAM;
    /// the palette storage itself is NOT implemented here (scheduler scope only).
    pub fn cgb_palette_blocked(&self) -> bool {
        self.vram_blocked()
    }
}

fn read_vram(vram: &[u8; 0x2000], offset: usize) -> u8 {
    vram.get(offset).copied().unwrap_or(0xFF)
}

fn decode_2bpp(low: u8, high: u8, x_flip: bool) -> [u8; FIFO_CAPACITY] {
    let mut pixels = [0; FIFO_CAPACITY];
    for (i, pixel) in pixels.iter_mut().enumerate() {
        let bit = if x_flip { i } else { 7 - i };
        let lo = (low >> bit) & 0x01;
        let hi = (high >> bit) & 0x01;
        *pixel = (hi << 1) | lo;
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ppu_at_line_start() -> Ppu {
        let mut p = Ppu::new();
        p.enabled = true;
        p.lcdc = 0x91;
        p.ly = 0;
        p.line_dot = 0;
        p.mode = mode::OAM_SCAN;
        p.coincidence = true;
        p.stat_line = false;
        p
    }

    fn zero_vram() -> [u8; 0x2000] {
        [0; 0x2000]
    }

    fn zero_oam() -> [u8; 0xA0] {
        [0; 0xA0]
    }

    fn tick_with(p: &mut Ppu, n: u32, vram: &[u8; 0x2000], oam: &[u8; 0xA0]) -> u8 {
        let mut irq = Interrupts::default();
        for _ in 0..n {
            p.tick_dot(&mut irq, vram, oam);
        }
        irq.settle_boundary();
        irq.if_ & 0x1F
    }

    fn tick(p: &mut Ppu, n: u32) -> u8 {
        tick_with(p, n, &zero_vram(), &zero_oam())
    }

    fn mode3_len(p: &mut Ppu, vram: &[u8; 0x2000], oam: &[u8; 0xA0]) -> u32 {
        tick_with(p, MODE2_DOTS, vram, oam);
        assert_eq!(p.mode, mode::DRAWING);
        let mut dots = 0;
        while p.mode == mode::DRAWING {
            tick_with(p, 1, vram, oam);
            dots += 1;
        }
        dots
    }

    fn set_tile_row(vram: &mut [u8; 0x2000], tile: usize, low: u8, high: u8) {
        let base = tile * 16;
        vram[base] = low;
        vram[base + 1] = high;
    }

    fn run_line(p: &mut Ppu, vram: &[u8; 0x2000], oam: &[u8; 0xA0]) {
        tick_with(p, DOTS_PER_LINE, vram, oam);
    }

    #[test]
    fn mode_sequence_within_visible_line() {
        let mut p = ppu_at_line_start();
        assert_eq!(p.mode, mode::OAM_SCAN);
        tick(&mut p, 79);
        assert_eq!(p.mode, mode::OAM_SCAN, "still OAM scan at dot 79");
        tick(&mut p, 1);
        assert_eq!(p.mode, mode::DRAWING, "mode 3 at dot 80");
        tick(&mut p, BASE_MODE3_DOTS - 1);
        assert_eq!(p.mode, mode::DRAWING, "still drawing at dot 251");
        tick(&mut p, 1);
        assert_eq!(p.mode, mode::HBLANK, "HBlank at dot 252");
    }

    #[test]
    fn ly_increments_at_line_end_and_wraps() {
        let mut p = ppu_at_line_start();
        assert_eq!(p.ly, 0);
        tick(&mut p, DOTS_PER_LINE);
        assert_eq!(p.ly, 1, "LY increments after 456 dots");
        tick(&mut p, DOTS_PER_LINE * (LAST_LINE as u32));
        assert_eq!(p.ly, 0, "LY wraps 153 -> 0");
    }

    #[test]
    fn entering_vblank_requests_vblank_irq() {
        let mut p = ppu_at_line_start();
        let irq = tick(&mut p, DOTS_PER_LINE * 144);
        assert_eq!(p.ly, 144);
        assert_eq!(p.mode, mode::VBLANK);
        assert!(irq & 0x01 != 0, "VBlank IRQ requested");
    }

    #[test]
    fn stat_mode0_rising_edge_fires_once() {
        let mut p = ppu_at_line_start();
        p.stat_enables = 0x08;
        let irq = tick(&mut p, MODE2_DOTS + BASE_MODE3_DOTS);
        assert_eq!(p.mode, mode::HBLANK);
        assert!(irq & 0x02 != 0, "STAT fires entering mode 0");
        let irq2 = tick(&mut p, 10);
        assert_eq!(irq2 & 0x02, 0, "no re-raise while in mode 0");
    }

    #[test]
    fn lyc_coincidence_fires_stat_once() {
        let mut p = ppu_at_line_start();
        p.stat_enables = 0x40;
        let mut irq = Interrupts::default();
        p.write_lyc(1, &mut irq);
        assert!(!p.coincidence, "LY=0 != LYC=1 yet");
        let irq = tick(&mut p, DOTS_PER_LINE);
        assert_eq!(p.ly, 1);
        assert!(p.coincidence);
        assert!(irq & 0x02 != 0, "STAT fires on LYC match");
    }

    #[test]
    fn lcd_off_resets_and_unblocks() {
        let mut p = ppu_at_line_start();
        tick(&mut p, MODE2_DOTS + 1);
        assert_eq!(p.mode, mode::DRAWING);
        assert!(p.vram_blocked());
        let mut irq = Interrupts::default();
        p.write_lcdc(0x00, &mut irq);
        assert_eq!(p.ly, 0, "LY reset on LCD off");
        assert_eq!(p.mode, mode::HBLANK, "mode reset on LCD off");
        assert!(!p.vram_blocked(), "VRAM accessible while LCD off");
        assert!(!p.oam_blocked(), "OAM accessible while LCD off");
    }

    #[test]
    fn vram_oam_blocking_by_mode() {
        let mut p = ppu_at_line_start();
        assert_eq!(p.mode, mode::OAM_SCAN);
        assert!(p.oam_blocked());
        assert!(!p.vram_blocked());
        tick(&mut p, MODE2_DOTS);
        assert_eq!(p.mode, mode::DRAWING);
        assert!(p.oam_blocked());
        assert!(p.vram_blocked());
        tick(&mut p, BASE_MODE3_DOTS);
        assert_eq!(p.mode, mode::HBLANK);
        assert!(!p.oam_blocked());
        assert!(!p.vram_blocked());
    }

    #[test]
    fn lcd_off_retains_coincidence_and_lyc_write_is_inert() {
        let mut p = ppu_at_line_start();
        let mut irq = Interrupts::default();
        p.stat_enables = 0x40;
        p.write_lyc(144, &mut irq);
        tick(&mut p, DOTS_PER_LINE * 144);
        assert_eq!(p.ly, 144);
        assert!(p.coincidence, "LY=144==LYC=144 -> coincident");
        p.write_lcdc(0x00, &mut irq);
        assert_eq!(p.ly, 0, "LY reset on LCD off");
        assert!(p.coincidence, "coincidence retained while LCD off");
        let mut irq2 = Interrupts::default();
        p.write_lyc(50, &mut irq2);
        assert_eq!(p.lyc, 50, "LYC value still stored while off");
        assert!(p.coincidence, "coincidence not recomputed while off");
        irq2.settle_boundary();
        assert_eq!(irq2.if_ & 0x02, 0, "no STAT raised by LYC write while off");
    }

    #[test]
    fn lcd_reenable_fires_stat_only_on_false_to_true() {
        let mut p = ppu_at_line_start();
        let mut irq = Interrupts::default();
        p.stat_enables = 0x40;

        p.write_lyc(0, &mut irq);
        assert!(p.coincidence && p.stat_line, "primed true");
        p.write_lcdc(0x00, &mut irq);
        let mut irq_a = Interrupts::default();
        p.write_lcdc(0x80, &mut irq_a);
        irq_a.settle_boundary();
        assert_eq!(irq_a.if_ & 0x02, 0, "true->true must not fire STAT");

        p.write_lyc(5, &mut irq);
        assert!(!p.coincidence && !p.stat_line, "now false");
        p.write_lcdc(0x00, &mut irq);
        p.write_lyc(0, &mut irq);
        let mut irq_b = Interrupts::default();
        p.write_lcdc(0x80, &mut irq_b);
        irq_b.settle_boundary();
        assert_eq!(irq_b.if_ & 0x02, 0x02, "false->true fires STAT once");
    }

    #[test]
    fn mode3_length_extends_by_scx_fine_scroll() {
        let mut p = ppu_at_line_start();
        p.write_scx(3);
        let len = mode3_len(&mut p, &zero_vram(), &zero_oam());
        assert_eq!(len, BASE_MODE3_DOTS + 3);
    }

    #[test]
    fn background_tile_renders_raw_color_indices() {
        let mut p = ppu_at_line_start();
        let mut vram = zero_vram();
        let oam = zero_oam();
        set_tile_row(&mut vram, 0, 0x55, 0x33);
        vram[0x1800] = 0;

        run_line(&mut p, &vram, &oam);

        assert_eq!(&p.framebuffer[0..8], &[0, 1, 2, 3, 0, 1, 2, 3]);
    }

    #[test]
    fn sprite_over_bg_mixing_honors_priority() {
        let mut vram = zero_vram();
        set_tile_row(&mut vram, 0, 0xFF, 0x00);
        set_tile_row(&mut vram, 1, 0x00, 0xFF);
        vram[0x1800] = 0;

        let mut oam = zero_oam();
        oam[0] = 16;
        oam[1] = 8;
        oam[2] = 1;
        oam[3] = 0x00;

        let mut sprite_wins = ppu_at_line_start();
        sprite_wins.write_lcdc(0x93, &mut Interrupts::default());
        run_line(&mut sprite_wins, &vram, &oam);
        assert_eq!(sprite_wins.framebuffer[0], 2, "sprite color wins without priority bit");

        oam[3] = 0x80;
        let mut bg_wins = ppu_at_line_start();
        bg_wins.write_lcdc(0x93, &mut Interrupts::default());
        run_line(&mut bg_wins, &vram, &oam);
        assert_eq!(bg_wins.framebuffer[0], 1, "BG color wins when OBJ priority is set");
    }

    #[test]
    fn window_triggers_at_wx_and_renders_window_tilemap() {
        let mut p = ppu_at_line_start();
        p.write_lcdc(0xF1, &mut Interrupts::default());
        p.write_wy(0);
        p.write_wx(7);

        let mut vram = zero_vram();
        let oam = zero_oam();
        set_tile_row(&mut vram, 0, 0xFF, 0x00);
        set_tile_row(&mut vram, 1, 0x00, 0xFF);
        vram[0x1800] = 0;
        vram[0x1C00] = 1;

        run_line(&mut p, &vram, &oam);

        assert_eq!(&p.framebuffer[0..8], &[2; 8]);
    }

    #[test]
    fn oam_scan_keeps_first_ten_sprites_per_line() {
        let mut p = ppu_at_line_start();
        let vram = zero_vram();
        let mut oam = zero_oam();
        for i in 0..11 {
            let base = i * 4;
            oam[base] = 16;
            oam[base + 1] = 8 + i as u8;
            oam[base + 2] = i as u8;
        }

        tick_with(&mut p, MODE2_DOTS, &vram, &oam);

        assert_eq!(p.scanline_sprite_count, 10);
        assert!(p.scanline_sprites[..10]
            .iter()
            .all(|sprite| sprite.oam_index < 10));
    }

    #[test]
    fn variable_mode3_still_pads_scanline_to_456_dots() {
        let mut p = ppu_at_line_start();
        p.write_scx(3);
        let vram = zero_vram();
        let oam = zero_oam();
        let mut counts = [0u32; 4];

        for _ in 0..DOTS_PER_LINE {
            counts[p.mode as usize] += 1;
            tick_with(&mut p, 1, &vram, &oam);
        }

        assert_eq!(counts[mode::OAM_SCAN as usize], MODE2_DOTS);
        assert_eq!(counts[mode::DRAWING as usize], BASE_MODE3_DOTS + 3);
        assert_eq!(counts[mode::HBLANK as usize], DOTS_PER_LINE - MODE2_DOTS - BASE_MODE3_DOTS - 3);
        assert_eq!(p.ly, 1);
        assert_eq!(p.line_dot, 0);
        assert_eq!(p.mode, mode::OAM_SCAN);
    }

    #[test]
    fn oam_scan_x0_sprites_consume_slots_per_pandocs() {
        // Pan Docs: the OAM scan selects by Y-coverage ONLY, so an X=0 (off-
        // screen) sprite STILL consumes one of the 10 per-line slots. Place 10
        // X=0 sprites earlier in OAM than a visible one; the visible sprite is
        // crowded out and is NOT buffered (it is the 11th Y-matching object).
        let mut p = ppu_at_line_start();
        let vram = zero_vram();
        let mut oam = zero_oam();
        for i in 0..10 {
            let base = i * 4;
            oam[base] = 16; // Y covers LY=0
            oam[base + 1] = 0; // X=0 -> off-screen but still selected by Y
            oam[base + 2] = i as u8;
        }
        // The 11th entry is a visible sprite at X=40 -- crowded out by the 10.
        oam[40] = 16;
        oam[41] = 40;
        oam[42] = 0xAB;

        tick_with(&mut p, MODE2_DOTS, &vram, &oam);

        assert_eq!(
            p.scanline_sprite_count, 10,
            "X=0 sprites consume slots (Y-only selection)"
        );
        assert!(
            p.scanline_sprites[..10].iter().all(|s| s.x == 0),
            "the 10 selected are the X=0 ones; the visible X=40 is crowded out"
        );
    }

    #[test]
    fn oam_scan_x0_sprite_is_skipped_at_fetch_not_drawn() {
        // An X=0 sprite that DID get a slot must NEVER produce a sprite fetch
        // (it is off-screen). Assert this THROUGHOUT drawing, not just at the
        // end -- a buggy impl could fetch, complete the 6-dot fetch, clear
        // pending_sprite, and still pass an end-only check.
        let mut p = ppu_at_line_start();
        let vram = zero_vram();
        let mut oam = zero_oam();
        oam[0] = 16; // Y covers LY=0
        oam[1] = 0; // X=0
        oam[2] = 0xCD;
        tick_with(&mut p, MODE2_DOTS, &vram, &oam);
        assert_eq!(p.scanline_sprite_count, 1, "X=0 sprite took a slot");

        // Step the entire drawing region one dot at a time; at no point may a
        // sprite fetch start or be in progress.
        for _ in 0..(DOTS_PER_LINE - MODE2_DOTS) {
            tick_with(&mut p, 1, &vram, &oam);
            assert!(p.pending_sprite.is_none(), "X=0 sprite must never be fetched");
            assert_eq!(p.sprite_fetch_ticks, 0, "no sprite fetch in progress");
            assert_eq!(p.sprite_idle_ticks, 0, "no sprite idle penalty incurred");
        }
    }

    #[test]
    fn sprite_fetch_preserves_bg_fetcher_column() {
        // A sprite fetch resets the BG fetcher's STEP but must NOT reset its
        // internal X-position counter (only window fetching does that). So BG
        // tile fetches resume in the same column after the sprite.
        let mut p = ppu_at_line_start();
        // Advance the fetcher's X-counter, then run a sprite fetch.
        p.bg_fetcher.fetcher_x = 5;
        p.bg_fetcher.reset_for_sprite();
        assert_eq!(
            p.bg_fetcher.fetcher_x, 5,
            "sprite-fetch reset preserves the BG fetcher X column"
        );
        assert_eq!(p.bg_fetcher.step, FetchStep::default(), "step reset to 1");
        // Contrast: a full reset (window/scanline) DOES clear fetcher_x.
        p.bg_fetcher.fetcher_x = 5;
        p.bg_fetcher.reset(false, false);
        assert_eq!(p.bg_fetcher.fetcher_x, 0, "full reset clears fetcher_x");
    }
}
