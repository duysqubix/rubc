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
const BASE_MODE3_DOTS: u32 = 172;
const LCD_ON_FIRST_LINE_DOTS: u32 = DOTS_PER_LINE - 4;
const LCD_ON_FIRST_MODE3_START: u32 = MODE2_DOTS;
const LCD_ON_FIRST_MODE3_PUBLIC_START: u32 = LCD_ON_FIRST_MODE3_START;
const LCD_ON_FIRST_MODE3_PUBLIC_END: u32 = LCD_ON_FIRST_MODE3_START + BASE_MODE3_DOTS - 1;
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

/// A fully-resolved framebuffer pixel: palette has already been applied by the
/// PPU at emission time (Oracle ses_1651a4026: palette selection is part of
/// rendering, not presentation). On DMG this is a 2-bit shade (post-BGP/OBP);
/// the `CgbRgb555` variant is reserved for the CGB color wave (rubc-5a0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePixel {
    /// Post-palette DMG shade, 0 (lightest) ..= 3 (darkest).
    DmgShade(u8),
    /// Post-palette CGB color, 15-bit RGB (bit15 unused). Reserved for rubc-5a0.
    CgbRgb555(u16),
}

impl Default for FramePixel {
    fn default() -> Self {
        FramePixel::DmgShade(0)
    }
}

impl FramePixel {
    /// Extract the DMG shade (0..=3). Panics on a CGB pixel -- mixed-mode
    /// callers must branch on the variant first.
    pub fn dmg_shade(self) -> u8 {
        match self {
            FramePixel::DmgShade(s) => s,
            FramePixel::CgbRgb555(_) => {
                panic!("dmg_shade() called on a CGB framebuffer pixel")
            }
        }
    }
}

/// Which DMG palette a FIFO pixel resolves through at emission time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DmgPaletteSource {
    #[default]
    Bg,
    Obp0,
    Obp1,
}

/// Live DMG palette registers, snapshotted from the bus each `tick_dot`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DmgPalettes {
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
}

/// Live CGB render state, snapshotted from the bus each `tick_dot`. When
/// `enabled` is false the PPU runs the DMG path and emits `DmgShade` pixels.
#[derive(Clone, Copy)]
pub struct CgbRenderState<'a> {
    pub enabled: bool,
    pub bg_palette_ram: &'a [u8; 64],
    pub obj_palette_ram: &'a [u8; 64],
}

/// Per-sprite attributes passed to `overlay_sprite_pixels` (bundled to keep the
/// argument count manageable).
#[derive(Clone, Copy)]
struct SpriteOverlay {
    bg_priority: bool,
    palette: DmgPaletteSource,
    cgb_palette: u8,
    oam_index: u8,
    cgb_priority: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct FifoPixel {
    color: u8,
    bg_priority: bool,
    occupied: bool,
    /// Palette this pixel resolves through (BG, or OBP0/OBP1 for sprites).
    palette: DmgPaletteSource,
    /// CGB palette number (0..=7). BG uses BCPD palettes, OBJ uses OCPD.
    cgb_palette: u8,
    /// OAM index of the sprite that produced this pixel (lower = higher CGB
    /// priority). Meaningless for BG pixels.
    oam_index: u8,
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

    fn push_bg_pixels(&mut self, colors: [u8; FIFO_CAPACITY], cgb_palette: u8, bg_priority: bool) {
        self.len = FIFO_CAPACITY;
        for (slot, color) in self.pixels.iter_mut().zip(colors) {
            *slot = FifoPixel {
                color: color & 0x03,
                // CGB BG attr bit 7 (master priority over OBJ); always false on DMG.
                bg_priority,
                occupied: true,
                palette: DmgPaletteSource::Bg,
                cgb_palette,
                oam_index: 0,
            };
        }
    }

    fn overlay_sprite_pixels(
        &mut self,
        colors: [u8; FIFO_CAPACITY],
        first_visible_pixel: usize,
        attrs: SpriteOverlay,
    ) {
        let SpriteOverlay {
            bg_priority,
            palette,
            cgb_palette,
            oam_index,
            cgb_priority,
        } = attrs;
        for (slot, color) in colors.iter().copied().skip(first_visible_pixel).enumerate() {
            if slot >= FIFO_CAPACITY {
                break;
            }
            if color == 0 {
                continue;
            }
            let existing = self.pixels[slot];
            if existing.occupied {
                // DMG: first sprite (by X/OAM fetch order) keeps the slot.
                // CGB: the lower OAM index wins, even if fetched later.
                if !cgb_priority || oam_index >= existing.oam_index {
                    continue;
                }
            }
            self.pixels[slot] = FifoPixel {
                color: color & 0x03,
                bg_priority,
                occupied: true,
                palette,
                cgb_palette,
                oam_index,
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

    fn push_front_bg_zero(&mut self) {
        let old_len = self.len.min(FIFO_CAPACITY - 1);
        for i in (0..old_len).rev() {
            self.pixels[i + 1] = self.pixels[i];
        }
        self.pixels[0] = FifoPixel {
            color: 0,
            bg_priority: false,
            occupied: true,
            palette: DmgPaletteSource::Bg,
            cgb_palette: 0,
            oam_index: 0,
        };
        self.len = (self.len + 1).min(FIFO_CAPACITY);
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
    /// CGB BG map attribute byte (from VRAM bank 1). 0 on DMG.
    attr: u8,
    low: u8,
    high: u8,
    dummy_fetch_done: bool,
    window: bool,
    /// BG map Y coordinate latched at the TileNo fetch step. Window activations
    /// set this from the window line counter before incrementing it for a later
    /// same-scanline reactivation.
    y: u8,
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
    /// Current internal render/access mode.
    pub mode: u8,
    /// Fully-resolved framebuffer: the PPU applies BGP/OBP at emission time, so
    /// each pixel is a post-palette `FramePixel` (DMG shade now; CGB RGB later).
    pub framebuffer: Box<[FramePixel; FRAMEBUFFER_PIXELS]>,

    /// DMG palette registers, snapshotted from the bus on each `tick_dot` so the
    /// pixel pipeline resolves shades against the value live at emission time.
    bgp: u8,
    obp0: u8,
    obp1: u8,
    /// CGB mode flag + palette RAM, snapshotted from the bus each `tick_dot`.
    /// When set, pixels resolve through RGB555 palette RAM (`CgbRgb555`); when
    /// clear, the DMG BGP/OBP path emits `DmgShade`.
    cgb_mode: bool,
    cgb_bg_palette_ram: [u8; 64],
    cgb_obj_palette_ram: [u8; 64],

    /// LCD master enable (LCDC bit 7).
    enabled: bool,
    first_line_after_lcd_on: bool,
    lcd_on_line1_coincidence_delay: bool,
    lcd_on_line1_delayed_mode2: bool,
    /// Full LCDC register (`$FF40`).
    lcdc: u8,
    /// Dot counter within the current scanline (0..456).
    line_dot: u32,
    /// LY compare register (`$FF45`).
    lyc: u8,
    /// STAT interrupt-source-select bits (3-6), stored as written.
    stat_enables: u8,
    /// CPU-visible STAT mode bits. These intentionally lag the internal render
    /// mode on visible-line edges without moving pixel/access timing.
    stat_read_mode: u8,
    /// Mode 0 STAT source level. This is separate from `stat_read_mode`: the
    /// public line-start mode-0 read window must not assert the mode-0 IRQ source.
    stat_mode0_level: bool,
    /// Single-dot mode 2 STAT source pulse. Mode 2 is not a level source here:
    /// holding it high for all OAM scan dots blocks later shared-line edges.
    stat_mode2_pulse: bool,
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
    window_glitch_x: Option<usize>,
    window_disable_pending: bool,
    drawing_dots: u32,
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            dot_ticks: 0,
            ly: 0,
            mode: mode::OAM_SCAN,
            framebuffer: Box::new([FramePixel::DmgShade(0); FRAMEBUFFER_PIXELS]),
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            cgb_mode: false,
            cgb_bg_palette_ram: [0xFF; 64],
            cgb_obj_palette_ram: [0xFF; 64],
            // LCD starts enabled with the post-boot LCDC ($91 = on, BG on, ...).
            enabled: true,
            first_line_after_lcd_on: false,
            lcd_on_line1_coincidence_delay: false,
            lcd_on_line1_delayed_mode2: false,
            lcdc: 0x91,
            line_dot: 0,
            lyc: 0,
            stat_enables: 0,
            stat_read_mode: mode::HBLANK,
            stat_mode0_level: false,
            stat_mode2_pulse: false,
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
            window_glitch_x: None,
            window_disable_pending: false,
            drawing_dots: 0,
        }
    }
}

impl Ppu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn oam_bug_scan_row(&self) -> Option<usize> {
        if !self.enabled || self.mode != mode::OAM_SCAN || self.ly > LAST_VISIBLE_LINE {
            return None;
        }
        let dot = self.line_dot.clamp(1, MODE2_DOTS);
        Some(((dot - 1) / 4) as usize)
    }

    // ---- the per-dot scheduler ---------------------------------------------

    /// Advance one dot (one T-cycle). Called 4x per M-cycle (or 2x in CGB
    /// double-speed) from the bus tick loop. Raises VBlank / STAT interrupts via
    /// `irq`. The bus owns VRAM/OAM and passes the current DMG VRAM bank + OAM.
    /// `palettes` is the live BGP/OBP0/OBP1 snapshot, applied at pixel emission.
    pub fn tick_dot(
        &mut self,
        irq: &mut Interrupts,
        vram: &[[u8; 0x2000]; 2],
        oam: &[u8; 0xA0],
        palettes: DmgPalettes,
        cgb: CgbRenderState,
    ) {
        self.bgp = palettes.bgp;
        self.obp0 = palettes.obp0;
        self.obp1 = palettes.obp1;
        self.cgb_mode = cgb.enabled;
        self.cgb_bg_palette_ram = *cgb.bg_palette_ram;
        self.cgb_obj_palette_ram = *cgb.obj_palette_ram;
        self.dot_ticks += 1;
        if !self.enabled {
            return;
        }

        self.line_dot += 1;
        if self.line_dot >= self.dots_this_line() {
            self.start_next_scanline(irq);
            self.update_stat_line(irq);
            return;
        }

        if self.ly <= LAST_VISIBLE_LINE {
            if self.lcd_on_line1_coincidence_delay && self.line_dot >= 4 {
                self.update_coincidence();
                self.lcd_on_line1_coincidence_delay = false;
            }

            self.update_visible_stat_read_mode();

            if self.line_dot == DOTS_PER_LINE - 4 && self.ly == LAST_VISIBLE_LINE && self.cgb_mode {
                self.stat_mode2_pulse = true;
            }

            match self.mode {
                mode::OAM_SCAN => {
                    self.tick_oam_scan(oam);
                    if self.line_dot >= MODE2_DOTS {
                        self.lcd_on_line1_delayed_mode2 = false;
                        self.enter_drawing(irq);
                    }
                }
                mode::DRAWING => self.tick_drawing(vram, irq),
                mode::HBLANK => {
                    if self.lcd_on_line1_delayed_mode2 && self.line_dot >= 4 {
                        self.begin_oam_scan();
                        self.set_mode(mode::OAM_SCAN, irq);
                        self.stat_read_mode = mode::OAM_SCAN;
                        self.stat_mode2_pulse = true;
                    } else if self.first_line_after_lcd_on
                        && self.ly == 0
                        && self.line_dot >= LCD_ON_FIRST_MODE3_START
                        && self.lcd_x == 0
                    {
                        self.enter_drawing(irq);
                    }
                }
                _ => {}
            }
        }

        self.update_stat_line(irq);
    }

    fn dots_this_line(&self) -> u32 {
        if self.first_line_after_lcd_on && self.ly == 0 {
            LCD_ON_FIRST_LINE_DOTS
        } else {
            DOTS_PER_LINE
        }
    }

    fn start_next_scanline(&mut self, irq: &mut Interrupts) {
        let was_lcd_on_first_line = self.first_line_after_lcd_on && self.ly == 0;
        self.line_dot = 0;
        self.ly = if self.ly >= LAST_LINE { 0 } else { self.ly + 1 };
        self.first_line_after_lcd_on = false;
        self.lcd_on_line1_delayed_mode2 = false;
        self.stat_mode0_level = false;
        if was_lcd_on_first_line {
            self.coincidence = false;
            self.lcd_on_line1_coincidence_delay = true;
        } else {
            self.update_coincidence();
        }

        if self.ly > LAST_VISIBLE_LINE {
            if self.ly == LAST_VISIBLE_LINE + 1 {
                self.window_y_condition = false;
                self.window_line_counter = 0;
            }
            self.set_mode(mode::VBLANK, irq);
            self.stat_read_mode = mode::VBLANK;
            if !self.cgb_mode && self.ly == LAST_VISIBLE_LINE + 1 {
                self.stat_mode2_pulse = true;
            }
        } else if was_lcd_on_first_line {
            self.set_mode(mode::HBLANK, irq);
            self.stat_read_mode = mode::HBLANK;
            self.lcd_on_line1_delayed_mode2 = true;
        } else {
            self.set_mode(mode::OAM_SCAN, irq);
            self.stat_read_mode = mode::HBLANK;
            self.stat_mode2_pulse = true;
        }
    }

    fn update_visible_stat_read_mode(&mut self) {
        if self.first_line_after_lcd_on && self.ly == 0 {
            self.stat_read_mode = match self.line_dot {
                0..=79 => mode::HBLANK,
                LCD_ON_FIRST_MODE3_PUBLIC_START..=LCD_ON_FIRST_MODE3_PUBLIC_END => mode::DRAWING,
                _ => mode::HBLANK,
            };
            return;
        }

        self.stat_read_mode = if self.line_dot < 4 {
            mode::HBLANK
        } else if self.line_dot < MODE2_DOTS + 4 {
            mode::OAM_SCAN
        } else if self.line_dot <= self.public_mode3_end_dot() {
            mode::DRAWING
        } else {
            mode::HBLANK
        };
    }

    fn public_mode3_end_dot(&self) -> u32 {
        let mut len = BASE_MODE3_DOTS + u32::from(self.scx & 0x07);
        let mut seen_tiles = [false; 32];
        let mut sprite_penalty = 0u32;
        for sprite in self.scanline_sprites[..self.scanline_sprite_count]
            .iter()
            .copied()
        {
            if self.lcdc & 0x02 == 0 {
                break;
            }
            if sprite.x >= 168 {
                continue;
            }
            sprite_penalty += 6;
            let left = sprite.x.wrapping_sub(8).wrapping_add(self.scx);
            let tile = ((left / 8) & 0x1F) as usize;
            if !seen_tiles[tile] {
                seen_tiles[tile] = true;
                sprite_penalty += u32::from(5u8.saturating_sub(left & 0x07));
            }
        }
        len += sprite_penalty / 4 * 4;
        MODE2_DOTS + 3 + len
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
        self.window_glitch_x = None;
        self.window_disable_pending = false;
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
        self.window_glitch_x = None;
        self.window_disable_pending = false;
        self.drawing_dots = 0;
        self.set_mode(mode::DRAWING, irq);
    }

    fn tick_drawing(&mut self, vram: &[[u8; 0x2000]; 2], irq: &mut Interrupts) {
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
        self.maybe_push_window_glitch_pixel();
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
        self.bg_fifo.clear();
        self.sprite_fifo.clear();
        self.pending_sprite = None;
        self.sprite_fetch_ticks = 0;
        self.sprite_idle_ticks = 0;
        self.window_glitch_x = None;
        self.window_disable_pending = false;
        self.stat_mode0_level = true;
        self.set_mode(mode::HBLANK, irq);
    }

    fn clock_bg_fetcher(&mut self, vram: &[[u8; 0x2000]; 2]) {
        if self.window_disable_pending && self.bg_fifo.is_empty() {
            if self.lcdc & 0x20 == 0 {
                self.stop_window_fetcher_for_bg_resume();
            } else {
                self.window_disable_pending = false;
            }
        }

        if self.bg_fetcher.step == FetchStep::Push {
            if self.bg_fifo.is_empty() {
                let x_flip = self.cgb_mode && self.bg_fetcher.attr & 0x20 != 0;
                let colors = if self.lcdc & 0x01 == 0 && !self.cgb_mode {
                    // DMG: LCDC bit 0 clear blanks the BG. In CGB this bit only
                    // affects OBJ master priority, not BG visibility.
                    [0; FIFO_CAPACITY]
                } else {
                    decode_2bpp(self.bg_fetcher.low, self.bg_fetcher.high, x_flip)
                };
                // CGB BG attr: bits 2-0 = palette, bit 7 = BG-to-OBJ priority.
                let cgb_palette = self.bg_fetcher.attr & 0x07;
                let bg_priority = self.cgb_mode && self.bg_fetcher.attr & 0x80 != 0;
                self.bg_fifo
                    .push_bg_pixels(colors, cgb_palette, bg_priority);
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
                // Latch map/window Y at TileNo; DMG data fetches may resample
                // SCY later for low/high row-mixing, while CGB-D+ keeps this Y.
                self.bg_fetcher.y = if self.bg_fetcher.window {
                    self.bg_fetcher.y
                } else {
                    self.ly.wrapping_add(self.scy)
                };

                let (tile, attr) = self.fetch_bg_tile_no(vram);
                self.bg_fetcher.tile = tile;
                self.bg_fetcher.attr = attr;
                self.bg_fetcher.step = FetchStep::TileDataLow;
            }
            FetchStep::TileDataLow => {
                let addr = self.fetch_bg_tile_data_addr();
                self.bg_fetcher.low = read_vram(&vram[self.bg_tile_bank()], addr);
                self.bg_fetcher.step = FetchStep::TileDataHigh;
            }
            FetchStep::TileDataHigh => {
                let addr = self.fetch_bg_tile_data_addr() + 1;
                self.bg_fetcher.high = read_vram(&vram[self.bg_tile_bank()], addr);
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

    /// CGB tile data VRAM bank for the current BG tile (attr bit 3). Always 0 on DMG.
    fn bg_tile_bank(&self) -> usize {
        if self.cgb_mode && self.bg_fetcher.attr & 0x08 != 0 {
            1
        } else {
            0
        }
    }

    fn fetch_bg_tile_no(&self, vram: &[[u8; 0x2000]; 2]) -> (u8, u8) {
        let map_base = if self.bg_fetcher.window {
            if self.lcdc & 0x40 != 0 {
                0x1C00
            } else {
                0x1800
            }
        } else if self.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };

        let x_offset = if self.bg_fetcher.window {
            self.bg_fetcher.fetcher_x & 0x1F
        } else {
            self.bg_fetcher.fetcher_x.wrapping_add(self.scx / 8) & 0x1F
        };
        let y_offset = if self.bg_fetcher.window {
            32 * ((self.bg_fetcher.y as usize / 8) & 0x1F)
        } else {
            // Use the Y latched at TileNo (set just before this call).
            32 * ((self.bg_fetcher.y as usize / 8) & 0x1F)
        };
        let offset = (y_offset + x_offset as usize) & 0x03FF;
        let tile = read_vram(&vram[0], map_base + offset);
        // CGB stores the tile attribute at the same tilemap offset in bank 1.
        let attr = if self.cgb_mode {
            read_vram(&vram[1], map_base + offset)
        } else {
            0
        };
        (tile, attr)
    }

    fn fetch_bg_tile_data_addr(&self) -> usize {
        // DMG samples SCY during the B/0/1 fetcher stages, so data low/high can
        // come from different tile rows after a mid-mode-3 SCY write. CGB-D+
        // samples SCY only at B; keep the existing TileNo latch for CGB so
        // cgb-acid-hell's sub-dot SCY race remains stable.
        let y = if self.bg_fetcher.window || self.cgb_mode {
            self.bg_fetcher.y
        } else {
            self.ly.wrapping_add(self.scy)
        };
        let mut row = (y & 0x07) as usize;
        // CGB BG attr bit 6 = vertical flip within the 8-pixel tile row.
        if self.cgb_mode && self.bg_fetcher.attr & 0x40 != 0 {
            row = 7 - row;
        }

        if self.lcdc & 0x10 != 0 {
            self.bg_fetcher.tile as usize * 16 + row * 2
        } else {
            let signed_tile = self.bg_fetcher.tile as i8 as i16;
            (0x1000i16 + signed_tile * 16 + (row * 2) as i16) as usize
        }
    }

    fn maybe_start_window(&mut self) {
        if self.window_active {
            return;
        }
        if self.lcdc & 0x20 == 0 || self.lcdc & 0x01 == 0 || !self.window_y_condition {
            return;
        }

        let window_x = self.wx.saturating_sub(7) as usize;
        if self.lcd_x != window_x {
            return;
        }

        self.window_active = true;
        self.window_started_this_line = true;
        self.window_glitch_x = None;
        self.bg_fifo.clear();
        self.bg_fetcher.reset(true, true);
        self.bg_fetcher.y = self.window_line_counter;
        self.window_line_counter = self.window_line_counter.wrapping_add(1);
    }

    fn stop_window_fetcher_for_bg_resume(&mut self) {
        let bg_x = (self.lcd_x as u8).wrapping_add(self.scx);
        self.window_active = false;
        self.window_disable_pending = false;
        self.bg_fetcher.reset(false, true);
        self.bg_fetcher.fetcher_x = (bg_x / 8) & 0x1F;
    }

    fn maybe_push_window_glitch_pixel(&mut self) {
        if self.window_glitch_x != Some(self.lcd_x) {
            return;
        }
        self.bg_fifo.push_front_bg_zero();
        self.window_glitch_x = None;
    }

    fn try_start_sprite_fetch(&mut self) -> bool {
        if self.lcdc & 0x02 == 0 {
            return false;
        }

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

    fn advance_sprite_fetch(&mut self, vram: &[[u8; 0x2000]; 2]) {
        if self.sprite_fetch_ticks > 0 {
            self.sprite_fetch_ticks -= 1;
        }
        if self.sprite_fetch_ticks != 0 {
            return;
        }

        if let Some(sprite) = self.pending_sprite.take() {
            self.load_sprite_fifo(vram, sprite);
            let remaining = self.bg_fifo.len().min(6) as u8;
            self.sprite_idle_ticks = 6 - remaining;
        }
    }

    fn load_sprite_fifo(&mut self, vram: &[[u8; 0x2000]; 2], sprite: ScanlineSprite) {
        // CGB OBJ tile data VRAM bank (OAM attr bit 3). Always bank 0 on DMG.
        let bank = if self.cgb_mode && sprite.attr & 0x08 != 0 {
            1
        } else {
            0
        };
        let addr = self.sprite_tile_data_addr(sprite);
        let low = read_vram(&vram[bank], addr);
        let high = read_vram(&vram[bank], addr + 1);
        let x_flip = sprite.attr & 0x20 != 0;
        let colors = decode_2bpp(low, high, x_flip);
        let first_visible = 8usize.saturating_sub(sprite.x as usize).min(7);
        // DMG OBJ palette select: attr bit 4 chooses OBP1 over OBP0.
        let palette = if sprite.attr & 0x10 != 0 {
            DmgPaletteSource::Obp1
        } else {
            DmgPaletteSource::Obp0
        };
        // CGB OBJ palette number = OAM attr bits 2-0.
        let cgb_palette = sprite.attr & 0x07;
        self.sprite_fifo.overlay_sprite_pixels(
            colors,
            first_visible,
            SpriteOverlay {
                bg_priority: sprite.attr & 0x80 != 0,
                palette,
                cgb_palette,
                oam_index: sprite.oam_index,
                cgb_priority: self.cgb_mode,
            },
        );
    }

    fn sprite_tile_data_addr(&self, sprite: ScanlineSprite) -> usize {
        let height = self.sprite_height();
        let mut row = self.ly.wrapping_add(16).wrapping_sub(sprite.y);
        if sprite.attr & 0x40 != 0 {
            // Y-flip mirrors the row within the sprite. `row` can momentarily
            // exceed `height` for objects being fetched off their covered range
            // (mid-mode-3 OAM changes); wrap to match hardware index behavior.
            row = (height - 1).wrapping_sub(row);
        }

        let tile = if height == 16 {
            (sprite.tile & 0xFE).wrapping_add(row / 8)
        } else {
            sprite.tile
        };
        tile as usize * 16 + (row as usize & 0x07) * 2
    }

    fn sprite_height(&self) -> u8 {
        if self.lcdc & 0x04 != 0 {
            16
        } else {
            8
        }
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

        let sprite_pixel = self.sprite_fifo.pop().unwrap_or_default();

        let pixel = if self.cgb_mode {
            self.resolve_cgb_pixel(bg_pixel, sprite_pixel)
        } else {
            self.resolve_dmg_pixel(bg_pixel, sprite_pixel)
        };

        if self.ly <= LAST_VISIBLE_LINE && self.lcd_x < SCREEN_WIDTH {
            let index = self.ly as usize * SCREEN_WIDTH + self.lcd_x;
            self.framebuffer[index] = pixel;
        }
        self.lcd_x += 1;

        if self.lcd_x >= SCREEN_WIDTH {
            self.finish_drawing(irq);
        }
        true
    }

    /// DMG pixel resolution: priority by OBJ attr bit 7, palette through
    /// BGP/OBP0/OBP1, emitting a post-palette `DmgShade`.
    fn resolve_dmg_pixel(&self, bg_pixel: FifoPixel, sprite_pixel: FifoPixel) -> FramePixel {
        let bg_color = if self.lcdc & 0x01 == 0 {
            0
        } else {
            bg_pixel.color & 0x03
        };
        let sprite_wins = sprite_pixel.occupied
            && sprite_pixel.color != 0
            && !(sprite_pixel.bg_priority && bg_color != 0);

        let shade = if sprite_wins {
            let obp = match sprite_pixel.palette {
                DmgPaletteSource::Obp1 => self.obp1,
                _ => self.obp0,
            };
            (obp >> (sprite_pixel.color * 2)) & 0x03
        } else {
            (self.bgp >> (bg_color * 2)) & 0x03
        };
        FramePixel::DmgShade(shade)
    }

    /// CGB pixel resolution: the BG/OBJ priority table (Pan Docs), then a
    /// 15-bit RGB lookup into BCPD/OCPD palette RAM, emitting `CgbRgb555`.
    fn resolve_cgb_pixel(&self, bg_pixel: FifoPixel, sprite_pixel: FifoPixel) -> FramePixel {
        let bg_color = bg_pixel.color & 0x03;
        let sprite_opaque = sprite_pixel.occupied && sprite_pixel.color != 0;

        // CGB BG/OBJ priority (Pan Docs table):
        //  - LCDC bit 0 clear  => OBJ always wins (master priority).
        //  - else if BG color 0 => OBJ wins.
        //  - else if BG-attr.7 or OAM-attr.7 set => BG colors 1-3 win.
        //  - else OBJ wins.
        let obj_master = self.lcdc & 0x01 == 0;
        let bg_has_priority = bg_pixel.bg_priority || sprite_pixel.bg_priority;
        let sprite_wins = sprite_opaque && (obj_master || bg_color == 0 || !bg_has_priority);

        if sprite_wins {
            let rgb = self.cgb_color(
                &self.cgb_obj_palette_ram,
                sprite_pixel.cgb_palette,
                sprite_pixel.color,
            );
            FramePixel::CgbRgb555(rgb)
        } else {
            let rgb = self.cgb_color(&self.cgb_bg_palette_ram, bg_pixel.cgb_palette, bg_color);
            FramePixel::CgbRgb555(rgb)
        }
    }

    /// Read a 15-bit RGB555 color from a CGB palette RAM block: palette 0..=7,
    /// color 0..=3, two little-endian bytes per color.
    fn cgb_color(&self, palette_ram: &[u8; 64], palette: u8, color: u8) -> u16 {
        let base = (palette as usize & 0x07) * 8 + (color as usize & 0x03) * 2;
        let lo = palette_ram[base] as u16;
        let hi = palette_ram[base + 1] as u16;
        (lo | (hi << 8)) & 0x7FFF
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
        self.stat_mode2_pulse = false;
    }

    fn stat_line_level(&self) -> bool {
        let e = self.stat_enables;
        let mode0 = (e & 0x08) != 0 && self.stat_mode0_level;
        let mode1 = (e & 0x10) != 0 && self.mode == mode::VBLANK;
        let mode2 = (e & 0x20) != 0 && self.stat_mode2_pulse;
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
        let old_lcdc = self.lcdc;
        self.lcdc = value;
        self.enabled = value & 0x80 != 0;

        if self.mode == mode::DRAWING && (old_lcdc ^ value) & 0x20 != 0 {
            if old_lcdc & 0x20 != 0 && value & 0x20 == 0 && self.bg_fetcher.window {
                self.window_disable_pending = true;
            } else if value & 0x20 != 0 {
                self.window_disable_pending = false;
            }
        }

        if was_on && !self.enabled {
            // LCD off: PPU stops, LY resets, mode -> 0, dot counter resets.
            // VRAM/OAM become fully accessible (see `vram_blocked`/`oam_blocked`).
            // The LYC coincidence flag is retained while the comparison clock is
            // stopped (mooneye stat_lyc_onoff); do not recompute it or clear the
            // STAT line here.
            self.ly = 0;
            self.line_dot = 0;
            self.mode = mode::HBLANK;
            self.first_line_after_lcd_on = false;
            self.lcd_on_line1_coincidence_delay = false;
            self.lcd_on_line1_delayed_mode2 = false;
            self.stat_read_mode = mode::HBLANK;
            self.stat_mode0_level = false;
            self.stat_mode2_pulse = false;
            self.lcd_x = 0;
            self.drawing_dots = 0;
            self.bg_fifo.clear();
            self.sprite_fifo.clear();
        } else if !was_on && self.enabled {
            // LCD on: restart the frame from the top. The first scanline after
            // enable begins in mode 0 (HBlank), not mode 2 -- the OAM-scan phase
            // is skipped on the very first line (mooneye stat_lyc_onoff). The
            // scheduler advances into mode 2 on the next line boundary.
            self.ly = 0;
            self.line_dot = 0;
            self.mode = mode::HBLANK;
            self.first_line_after_lcd_on = true;
            self.lcd_on_line1_coincidence_delay = false;
            self.lcd_on_line1_delayed_mode2 = false;
            self.stat_read_mode = mode::HBLANK;
            self.stat_mode0_level = false;
            self.stat_mode2_pulse = false;
            self.lcd_x = 0;
            self.drawing_dots = 0;
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
        let mode = if self.enabled {
            self.stat_read_mode
        } else {
            mode::HBLANK
        };
        0x80 | self.stat_enables | ((self.coincidence as u8) << 2) | mode
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
        if self.mode == mode::DRAWING && self.window_started_this_line {
            let window_x = self.wx.saturating_sub(7) as usize;
            self.window_glitch_x =
                (window_x > self.lcd_x && window_x < SCREEN_WIDTH).then_some(window_x);
        }
    }

    // ---- VRAM / OAM access gating -------------------------------------------

    /// Whether the LCD is currently enabled (LCDC bit 7). HBlank DMA only
    /// advances while the display is on.
    pub fn lcd_enabled(&self) -> bool {
        self.enabled
    }

    /// VRAM (`$8000-$9FFF`) is inaccessible during mode 3 (returns 0xFF / writes
    /// dropped). When the LCD is off, VRAM is always accessible.
    pub fn vram_blocked(&self) -> bool {
        self.enabled && self.drawing_access_blocked()
    }

    pub fn vram_write_blocked(&self) -> bool {
        self.vram_blocked()
            && !(self.mode == mode::DRAWING
                && !self.first_line_after_lcd_on
                && self.drawing_dots < 4)
    }

    /// OAM (`$FE00-$FE9F`) is inaccessible during modes 2 and 3. When the LCD is
    /// off, OAM is always accessible.
    pub fn oam_blocked(&self) -> bool {
        self.enabled && (self.mode == mode::OAM_SCAN || self.drawing_access_blocked())
    }

    pub fn oam_read_blocked(&self) -> bool {
        self.oam_blocked() || (self.enabled && self.lcd_on_line1_delayed_mode2)
    }

    pub fn oam_write_blocked(&self) -> bool {
        self.oam_blocked()
            && !(self.mode == mode::OAM_SCAN && self.line_dot < 4)
            && !(self.mode == mode::DRAWING
                && !self.first_line_after_lcd_on
                && self.drawing_dots < 4)
    }

    fn drawing_access_blocked(&self) -> bool {
        self.mode == mode::DRAWING || self.stat_read_mode == mode::DRAWING
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
        p.stat_read_mode = mode::HBLANK;
        p.stat_mode0_level = false;
        p.stat_mode2_pulse = false;
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

    /// Identity DMG palettes (0xE4): each raw color N maps to shade N, so the
    /// existing raw-index framebuffer assertions remain meaningful.
    const IDENTITY_PALETTES: DmgPalettes = DmgPalettes {
        bgp: 0xE4,
        obp0: 0xE4,
        obp1: 0xE4,
    };

    fn tick_with(p: &mut Ppu, n: u32, vram: &[u8; 0x2000], oam: &[u8; 0xA0]) -> u8 {
        let mut irq = Interrupts::default();
        // Tests exercise the DMG path: bank 0 holds the data, bank 1 is empty,
        // and CGB rendering is disabled so the DMG shade pipeline runs.
        let mut banks = [[0u8; 0x2000]; 2];
        banks[0] = *vram;
        let zero = [0u8; 64];
        let cgb = CgbRenderState {
            enabled: false,
            bg_palette_ram: &zero,
            obj_palette_ram: &zero,
        };
        for _ in 0..n {
            p.tick_dot(&mut irq, &banks, oam, IDENTITY_PALETTES, cgb);
        }
        irq.settle_boundary();
        irq.if_ & 0x1F
    }

    fn tick(p: &mut Ppu, n: u32) -> u8 {
        tick_with(p, n, &zero_vram(), &zero_oam())
    }

    fn tick_cgb(p: &mut Ppu, n: u32) -> u8 {
        let mut irq = Interrupts::default();
        let banks = [[0u8; 0x2000]; 2];
        let oam = zero_oam();
        let zero = [0u8; 64];
        let cgb = CgbRenderState {
            enabled: true,
            bg_palette_ram: &zero,
            obj_palette_ram: &zero,
        };
        for _ in 0..n {
            p.tick_dot(&mut irq, &banks, &oam, IDENTITY_PALETTES, cgb);
        }
        irq.settle_boundary();
        irq.if_ & 0x1F
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
    fn line144_dmg_mode2_pulse_shares_vblank_bucket() {
        let mut p = ppu_at_line_start();
        p.ly = LAST_VISIBLE_LINE;
        p.line_dot = DOTS_PER_LINE - 1;
        p.mode = mode::HBLANK;
        p.stat_read_mode = mode::HBLANK;
        p.stat_enables = 0x20;
        let irq = tick(&mut p, 1);
        assert_eq!(p.ly, LAST_VISIBLE_LINE + 1);
        assert_eq!(p.read_stat() & 0x03, mode::VBLANK);
        assert_eq!(irq & 0x03, 0x03);
        assert_eq!(tick(&mut p, 1) & 0x02, 0);
    }

    #[test]
    fn line144_cgb_mode2_pulse_precedes_vblank_bucket() {
        let mut p = ppu_at_line_start();
        p.ly = LAST_VISIBLE_LINE;
        p.line_dot = DOTS_PER_LINE - 5;
        p.mode = mode::HBLANK;
        p.stat_read_mode = mode::HBLANK;
        p.stat_enables = 0x20;
        p.cgb_mode = true;
        assert_eq!(tick_cgb(&mut p, 1) & 0x03, 0x02);
        assert_eq!(tick_cgb(&mut p, 3) & 0x03, 0);
        let irq = tick_cgb(&mut p, 1);
        assert_eq!(p.ly, LAST_VISIBLE_LINE + 1);
        assert_eq!(irq & 0x03, 0x01);
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
    fn stat_read_mode_lags_internal_mode3_by_four_dots() {
        let mut p = ppu_at_line_start();
        tick(&mut p, MODE2_DOTS);
        assert_eq!(p.mode, mode::DRAWING);
        assert_eq!(p.read_stat() & 0x03, mode::OAM_SCAN);
        tick(&mut p, 3);
        assert_eq!(p.read_stat() & 0x03, mode::OAM_SCAN);
        tick(&mut p, 1);
        assert_eq!(p.read_stat() & 0x03, mode::DRAWING);
    }

    #[test]
    fn stat_mode2_source_pulses_at_line_start_not_public_mode2_read() {
        let mut p = ppu_at_line_start();
        p.line_dot = DOTS_PER_LINE - 1;
        p.mode = mode::HBLANK;
        p.stat_read_mode = mode::HBLANK;
        p.stat_enables = 0x20;
        assert_eq!(tick(&mut p, 1) & 0x02, 0x02);
        assert_eq!(p.read_stat() & 0x03, mode::HBLANK);
        assert_eq!(tick(&mut p, 3) & 0x02, 0);
        assert_eq!(tick(&mut p, 1) & 0x02, 0);
        assert_eq!(p.read_stat() & 0x03, mode::OAM_SCAN);
        assert_eq!(tick(&mut p, 1) & 0x02, 0);
        assert!(!p.stat_line);
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
        assert_eq!(p.read_stat() & 0x03, mode::DRAWING);
        assert!(p.oam_blocked(), "access follows public mode 3 tail");
        assert!(p.vram_blocked(), "access follows public mode 3 tail");
        tick(&mut p, 4);
        assert_eq!(p.read_stat() & 0x03, mode::HBLANK);
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
    fn lcd_on_starts_first_line_in_mode0() {
        // mooneye stat_lyc_onoff: after enabling the LCD, the first scanline
        // starts in mode 0 (HBlank), not mode 2 (OAM scan). STAT therefore reads
        // mode bits = 0 immediately after LCD-on.
        let mut p = ppu_at_line_start();
        let mut irq = Interrupts::default();
        p.write_lcdc(0x00, &mut irq); // LCD off
        p.write_lcdc(0x80, &mut irq); // LCD on
        assert_eq!(
            p.mode,
            mode::HBLANK,
            "first line after LCD enable is mode 0"
        );
        assert_eq!(
            p.read_stat() & 0x03,
            0,
            "STAT mode bits read 0 after enable"
        );
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

        let shades: Vec<u8> = p.framebuffer[0..8]
            .iter()
            .map(|px| px.dmg_shade())
            .collect();
        assert_eq!(shades, vec![0, 1, 2, 3, 0, 1, 2, 3]);
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
        assert_eq!(
            sprite_wins.framebuffer[0].dmg_shade(),
            2,
            "sprite color wins without priority bit"
        );

        oam[3] = 0x80;
        let mut bg_wins = ppu_at_line_start();
        bg_wins.write_lcdc(0x93, &mut Interrupts::default());
        run_line(&mut bg_wins, &vram, &oam);
        assert_eq!(
            bg_wins.framebuffer[0].dmg_shade(),
            1,
            "BG color wins when OBJ priority is set"
        );
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

        let shades: Vec<u8> = p.framebuffer[0..8]
            .iter()
            .map(|px| px.dmg_shade())
            .collect();
        assert_eq!(shades, vec![2; 8]);
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
        assert_eq!(
            counts[mode::HBLANK as usize],
            DOTS_PER_LINE - MODE2_DOTS - BASE_MODE3_DOTS - 3
        );
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
            assert!(
                p.pending_sprite.is_none(),
                "X=0 sprite must never be fetched"
            );
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
