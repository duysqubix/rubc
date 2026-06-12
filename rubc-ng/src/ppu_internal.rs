use crate::golden::{GoldenInitialState, GoldenV2Reader, GoldenV2Row, GoldenVramState, Vram};
use crate::pixel_fifo::{decode_2bpp, FetchStep, FifoPixel, LineRenderState, SpriteOverlay};
use crate::time::Time;
use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PpuRegisterWrite {
    time: Time,
    addr: u16,
    value: u8,
}

#[derive(Clone, Debug)]
pub struct PpuInternal {
    lcdc: u8,
    scy: u8,
    scx: u8,
    wx: u8,
    wy: u8,
    vram: Vram,
    oam: [u8; 0xA0],
    fetcher_x: u16,
    current_tile_data_addr: Option<u16>,
    window_active: bool,
    window_line: u8,
    /// W8b·2b-fifo: per-line FIFO render state (rubc-d85o).
    fifo: LineRenderState,
    /// Latched once LY==WY matched on any line this frame (SameBoy
    /// display.c wy_triggered; cleared at frame start).
    fifo_window_y_condition: bool,
    /// The window's internal line counter; advances at each activation.
    fifo_window_line: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowRestart {
    pub trigger_x: usize,
    pub penalty_dots: u8,
    pub scx_low_bits_after_restart: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedSprite {
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub attr: u8,
    pub oam_index: u8,
    pub palette: SpritePalette,
    pub bg_priority: bool,
    pub size: SpriteSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpritePalette {
    Obp0,
    Obp1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteSize {
    Size8x8,
    Size8x16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpritePriorityMode {
    DmgXOrder,
    OamOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedDmgPixel {
    Bg,
    Obj(SpritePalette),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderedPpuPixel {
    pub raw_color: u8,
    pub source: LcdPixelSource,
    pub cgb_palette: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LcdPixelSource {
    Bg,
    Obj(SpritePalette),
}

/// One pixel shipped by the FIFO: the LCD column it lands on plus the
/// resolved raw color / source / CGB palette (palette lookup happens at the
/// output latch / CGB palette RAM, same as the direct renderer did).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FifoOutput {
    pub x: usize,
    pub pixel: RenderedPpuPixel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BgFetchSample {
    pub raw_tick: u64,
    pub ly: u16,
    pub stage: String,
    pub fetcher_x: u16,
    pub machine_addr: u16,
    pub machine_byte: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BgFetchDivergence {
    rom: String,
    index: usize,
    raw_tick: u64,
    ly: u16,
    stage: String,
    machine_addr: u16,
    golden_addr: u16,
    machine_byte: u8,
    golden_byte: u8,
}

impl fmt::Display for BgFetchDivergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "first BG fetch divergence for {} at fetch #{} tick {} LY {} {}: machine addr {:04X}, machine byte {:02X}, golden addr {:04X}, golden byte {:02X}",
            self.rom,
            self.index,
            self.raw_tick,
            self.ly,
            self.stage,
            self.machine_addr,
            self.machine_byte,
            self.golden_addr,
            self.golden_byte
        )
    }
}

impl std::error::Error for BgFetchDivergence {}

impl PpuInternal {
    pub fn from_golden_state(_initial: GoldenInitialState, state: GoldenVramState) -> Self {
        Self {
            lcdc: state.regs.lcdc,
            scy: state.regs.scy,
            scx: state.regs.scx,
            wx: state.regs.wx,
            wy: state.regs.wy,
            vram: state.vram,
            oam: state.oam,
            fetcher_x: 0,
            current_tile_data_addr: None,
            window_active: false,
            window_line: 0,
            fifo: LineRenderState::default(),
            fifo_window_y_condition: false,
            fifo_window_line: 0,
        }
    }

    pub fn for_test(
        lcdc: u8,
        scy: u8,
        scx: u8,
        wx: u8,
        wy: u8,
        vram: Vram,
        oam: [u8; 0xA0],
    ) -> Self {
        Self {
            lcdc,
            scy,
            scx,
            wx,
            wy,
            vram,
            oam,
            fetcher_x: 0,
            current_tile_data_addr: None,
            window_active: false,
            window_line: 0,
            fifo: LineRenderState::default(),
            fifo_window_y_condition: false,
            fifo_window_line: 0,
        }
    }

    pub fn trigger_window_for_test(&mut self, ly: u16, screen_x: usize) -> Option<WindowRestart> {
        if self.window_triggered(ly, screen_x) {
            self.window_active = true;
            self.window_line = ly.saturating_sub(u16::from(self.wy)) as u8;
            self.fetcher_x = 0;
            self.current_tile_data_addr = None;
            Some(WindowRestart {
                trigger_x: screen_x,
                penalty_dots: 6,
                scx_low_bits_after_restart: 0,
            })
        } else {
            None
        }
    }

    pub fn fetch_next_tile_for_test(&mut self, ly: u16) -> Result<BgFetchSample, String> {
        let addr = self.tilemap_addr(ly);
        let byte = self.vram.read(addr, 0)?;
        self.current_tile_data_addr = Some(self.tile_data_addr(byte, ly));
        let sample = BgFetchSample {
            raw_tick: 0,
            ly,
            stage: "GET_TILE_T1".to_owned(),
            fetcher_x: self.fetcher_x,
            machine_addr: addr,
            machine_byte: byte,
        };
        self.fetcher_x = self.fetcher_x.wrapping_add(1);
        Ok(sample)
    }

    pub fn select_sprites_for_test(lcdc: u8, oam: &[u8; 0xA0], ly: u8) -> Vec<SelectedSprite> {
        select_scanline_sprites(lcdc, oam, ly)
    }

    pub fn selected_scanline_sprites_for_test(&self, ly: u8) -> Vec<SelectedSprite> {
        select_scanline_sprites(self.lcdc, &self.oam, ly)
    }

    pub fn selected_sprite_for_test(
        y: u8,
        x: u8,
        tile: u8,
        attr: u8,
        oam_index: u8,
    ) -> SelectedSprite {
        let size = SpriteSize::Size8x8;
        SelectedSprite {
            y,
            x,
            tile,
            attr,
            oam_index,
            palette: if attr & 0x10 != 0 {
                SpritePalette::Obp1
            } else {
                SpritePalette::Obp0
            },
            bg_priority: attr & 0x80 != 0,
            size,
        }
    }

    pub fn resolve_dmg_obj_over_bg_for_test(
        bg_color: u8,
        bg_master_priority: bool,
        obj_color: u8,
        sprite: SelectedSprite,
    ) -> ResolvedDmgPixel {
        Self::resolve_dmg_obj_over_bg(bg_color, bg_master_priority, obj_color, sprite)
    }

    fn resolve_dmg_obj_over_bg(
        bg_color: u8,
        bg_master_priority: bool,
        obj_color: u8,
        sprite: SelectedSprite,
    ) -> ResolvedDmgPixel {
        if obj_color == 0 || (bg_color != 0 && (bg_master_priority || sprite.bg_priority)) {
            ResolvedDmgPixel::Bg
        } else {
            ResolvedDmgPixel::Obj(sprite.palette)
        }
    }

    pub fn sprite_fetch_order_for_test(
        sprites: &[SelectedSprite],
        mode: SpritePriorityMode,
    ) -> Vec<SelectedSprite> {
        let mut fetch = sprites.to_vec();
        match mode {
            SpritePriorityMode::DmgXOrder => {
                fetch.sort_by_key(|sprite| (sprite.x, sprite.oam_index))
            }
            SpritePriorityMode::OamOrder => fetch.sort_by_key(|sprite| sprite.oam_index),
        }
        fetch
    }

    pub fn write_register_at(&mut self, addr: u16, value: u8) {
        self.write_register(PpuRegisterWrite {
            time: Time::ZERO,
            addr,
            value,
        });
    }

    pub fn write_vram(&mut self, addr: u16, bank: u8, value: u8) {
        let _ = self.vram.write_for_test(addr & 0x1FFF, bank & 1, value);
    }

    pub fn write_oam(&mut self, offset: usize, value: u8) {
        if let Some(byte) = self.oam.get_mut(offset) {
            *byte = value;
        }
    }

    pub fn render_pixel(&self, cgb: bool, ly: u8, x: usize) -> RenderedPpuPixel {
        let (bg_color, bg_palette, bg_priority) = self.bg_or_window_pixel(cgb, ly, x);
        let mut pixel = RenderedPpuPixel {
            raw_color: bg_color,
            source: LcdPixelSource::Bg,
            cgb_palette: bg_palette,
        };

        if self.lcdc & 0x02 == 0 {
            return pixel;
        }

        let mode = if cgb {
            SpritePriorityMode::OamOrder
        } else {
            SpritePriorityMode::DmgXOrder
        };
        for sprite in Self::sprite_fetch_order_for_test(
            &select_scanline_sprites(self.lcdc, &self.oam, ly),
            mode,
        ) {
            let Some((obj_color, obj_palette)) = self.sprite_pixel(cgb, ly, x, sprite) else {
                continue;
            };
            if obj_color == 0 {
                continue;
            }
            if cgb {
                if bg_priority || (sprite.bg_priority && bg_color != 0) {
                    return pixel;
                }
                return RenderedPpuPixel {
                    raw_color: obj_color,
                    source: LcdPixelSource::Obj(sprite.palette),
                    cgb_palette: obj_palette,
                };
            }
            pixel.source = match Self::resolve_dmg_obj_over_bg(bg_color, false, obj_color, sprite) {
                ResolvedDmgPixel::Bg => LcdPixelSource::Bg,
                ResolvedDmgPixel::Obj(palette) => LcdPixelSource::Obj(palette),
            };
            if matches!(pixel.source, LcdPixelSource::Obj(_)) {
                pixel.raw_color = obj_color;
                pixel.cgb_palette = obj_palette;
                return pixel;
            }
        }

        pixel
    }

    // ---- W8b·2b-fifo: the real per-dot pixel FIFO (rubc-d85o) ------------
    //
    // Replaces the direct per-pixel formula (`render_pixel`) in the machine
    // frame loop. The direct formula computed absolute window coordinates
    // (map_x = x-(WX-7)) and skipped the FIFO restart / fetch-discard /
    // fine-X geometry; this is the hardware path: a BG fetcher feeding an
    // 8-pixel FIFO, window restart at WX == lcd_x+7, SCX&7 discard, and
    // sprite staging merged at the FIFO (TCAGBD pixel-FIFO chapter; SameBoy
    // Core/display.c; Pan Docs pixel_fifo.md; cross-checked against the
    // acid-proven rubc-core FIFO).

    /// Mode-3 line start: latch the WY condition for this line, select and
    /// X-sort the scanline sprites, and arm the line render state.
    pub fn begin_drawing(&mut self, ly: u8) {
        if ly == self.wy {
            self.fifo_window_y_condition = true;
        }
        let mut sprites = select_scanline_sprites(self.lcdc, &self.oam, ly);
        sprites.sort_by_key(|sprite| (sprite.x, sprite.oam_index));
        self.fifo = LineRenderState::begin(sprites, self.scx & 0x07);
    }

    /// Frame boundary (LY==144 line start): the window line counter and the
    /// WY latch reset (rubc-core ppu.rs resets both when entering VBlank).
    pub fn begin_frame_window_state(&mut self) {
        self.fifo_window_y_condition = false;
        self.fifo_window_line = 0;
    }

    /// One mode-3 drawing dot. Returns the pixel shipped to the LCD on this
    /// dot, if any. Dot order follows the hardware pipeline: sprite fetch
    /// stalls first, then the window compare, then the BG fetcher, then the
    /// FIFO shift (rubc-core phase_drawing_dot; SameBoy display.c render loop).
    pub fn fifo_dot(&mut self, cgb: bool, ly: u8) -> Option<FifoOutput> {
        if !self.fifo.active {
            return None;
        }
        if self.fifo.sprite_idle_ticks > 0 {
            self.fifo_clock_bg_fetcher(cgb, ly);
            self.fifo.sprite_idle_ticks -= 1;
            return None;
        }
        if self.fifo.pending_sprite.is_some() {
            self.fifo_advance_sprite_fetch(cgb, ly);
            return None;
        }
        self.fifo_maybe_start_window(cgb);
        if self.fifo_try_start_sprite_fetch() {
            self.fifo_advance_sprite_fetch(cgb, ly);
            return None;
        }
        self.fifo_clock_bg_fetcher(cgb, ly);
        let (shifted, output) = self.fifo_shift_pixel(cgb);
        if shifted {
            self.fifo_maybe_start_window(cgb);
        }
        output
    }

    /// Window activation compare (SameBoy display.c: WX == position+7; the
    /// DMG-only WX==position+6 late trigger applies a one-pixel desync).
    /// Activation clears the BG FIFO, restarts the fetcher in window mode
    /// with X=0, preloads the window line counter, and advances it.
    fn fifo_maybe_start_window(&mut self, cgb: bool) {
        if self.fifo.window_active {
            return;
        }
        if self.lcdc & 0x20 == 0 || self.lcdc & 0x01 == 0 || !self.fifo_window_y_condition {
            return;
        }
        let window_x = usize::from(self.wx.saturating_sub(7));
        let dmg_early_x = usize::from(self.wx.saturating_sub(6));
        let dmg_early = !cgb && self.fifo.lcd_x == dmg_early_x;
        if self.fifo.lcd_x != window_x && !dmg_early {
            return;
        }
        if dmg_early && self.fifo.lcd_x != window_x && self.fifo.lcd_x > 0 {
            self.fifo.lcd_x -= 1;
        }
        self.fifo.window_active = true;
        self.fifo.window_started_this_line = true;
        // WX<7: the compare fired before the first visible pixel; keep the
        // X=0 restart but discard the off-screen window columns.
        if self.fifo.lcd_x == 0 && self.wx < 6 {
            self.fifo.scx_discard = 7 - self.wx;
        }
        self.fifo.bg_fifo.clear();
        self.fifo.fetcher.reset(true, true);
        self.fifo.fetcher.y = self.fifo_window_line;
        self.fifo_window_line = self.fifo_window_line.wrapping_add(1);
    }

    /// Sprite trigger: the next X-ordered sprite whose trigger column has
    /// been reached pauses the BG pipeline for a 6-dot fetch. X=0 sprites
    /// consumed a scan slot but never fetch (Pan Docs OAM scan).
    fn fifo_try_start_sprite_fetch(&mut self) -> bool {
        if self.lcdc & 0x02 == 0 {
            return false;
        }
        while self.fifo.next_sprite < self.fifo.sprites.len()
            && self.fifo.sprites[self.fifo.next_sprite].x == 0
        {
            self.fifo.next_sprite += 1;
        }
        if self.fifo.next_sprite < self.fifo.sprites.len() {
            let sprite = self.fifo.sprites[self.fifo.next_sprite];
            if usize::from(sprite.x) <= self.fifo.lcd_x + 8 {
                self.fifo.next_sprite += 1;
                self.fifo.pending_sprite = Some(sprite);
                self.fifo.sprite_fetch_ticks = 6;
                // A sprite fetch resets+pauses the BG fetcher but keeps any
                // queued BG pixels (GBEDG sprite fetching).
                self.fifo.fetcher.reset_for_sprite();
                return true;
            }
        }
        false
    }

    fn fifo_advance_sprite_fetch(&mut self, cgb: bool, ly: u8) {
        if self.fifo.sprite_fetch_ticks > 0 {
            self.fifo.sprite_fetch_ticks -= 1;
        }
        if self.fifo.sprite_fetch_ticks != 0 {
            return;
        }
        if let Some(sprite) = self.fifo.pending_sprite.take() {
            self.fifo_load_sprite(cgb, ly, sprite);
            // The BG fetcher restarts while the FIFO keeps shifting; model
            // the residual stall as idle dots (rubc-core advance_sprite_fetch).
            let remaining = self.fifo.bg_fifo.len().min(6) as u8;
            self.fifo.sprite_idle_ticks = 6 - remaining;
        }
    }

    fn fifo_load_sprite(&mut self, cgb: bool, ly: u8, sprite: SelectedSprite) {
        let height = sprite_height(sprite.size);
        let mut row = ly.wrapping_add(16).wrapping_sub(sprite.y);
        if sprite.attr & 0x40 != 0 {
            row = (height - 1).wrapping_sub(row);
        }
        let tile = if height == 16 {
            (sprite.tile & 0xFE).wrapping_add(row / 8)
        } else {
            sprite.tile
        };
        let addr = u16::from(tile) * 16 + u16::from(row & 0x07) * 2;
        let bank = if cgb && sprite.attr & 0x08 != 0 { 1 } else { 0 };
        let low = self.vram.read(addr, bank).unwrap_or(0);
        let high = self.vram.read(addr.wrapping_add(1), bank).unwrap_or(0);
        let colors = decode_2bpp(low, high, sprite.attr & 0x20 != 0);
        // Sprites at X<8 enter clipped: only the right `x` columns land on
        // screen (SameBoy object_buffer staging at position+8).
        let first_visible = 8usize.saturating_sub(usize::from(sprite.x)).min(7);
        self.fifo.sprite_fifo.overlay_sprite_pixels(
            colors,
            first_visible,
            SpriteOverlay {
                bg_priority: sprite.bg_priority,
                palette: sprite.palette,
                cgb_palette: sprite.attr & 0x07,
                oam_index: sprite.oam_index,
                cgb_priority: cgb,
            },
        );
    }

    /// One BG fetcher dot (Pan Docs pixel_fifo.md: 2 dots per step; the push
    /// step retries until the BG FIFO is empty; the first data-high completion
    /// of a line restarts the fetcher — the discarded dummy fetch).
    fn fifo_clock_bg_fetcher(&mut self, cgb: bool, ly: u8) {
        if self.fifo.window_disable_pending && self.fifo.bg_fifo.is_empty() {
            if self.lcdc & 0x20 == 0 {
                let bg_x = (self.fifo.lcd_x as u8).wrapping_add(self.scx);
                self.fifo.window_active = false;
                self.fifo.window_disable_pending = false;
                self.fifo.fetcher.reset(false, true);
                self.fifo.fetcher.fetcher_x = (bg_x / 8) & 0x1F;
            } else {
                self.fifo.window_disable_pending = false;
            }
        }
        if self.fifo.fetcher.step == FetchStep::Push {
            if self.fifo.bg_fifo.is_empty() {
                let x_flip = cgb && self.fifo.fetcher.attr & 0x20 != 0;
                let colors = decode_2bpp(self.fifo.fetcher.low, self.fifo.fetcher.high, x_flip);
                let cgb_palette = self.fifo.fetcher.attr & 0x07;
                let bg_priority = cgb && self.fifo.fetcher.attr & 0x80 != 0;
                self.fifo
                    .bg_fifo
                    .push_bg_pixels(colors, cgb_palette, bg_priority);
                self.fifo.fetcher.fetcher_x = self.fifo.fetcher.fetcher_x.wrapping_add(1);
                self.fifo.fetcher.step = FetchStep::TileNo;
                self.fifo.fetcher.step_ticks = 0;
            }
            return;
        }
        self.fifo.fetcher.step_ticks += 1;
        if self.fifo.fetcher.step_ticks < 2 {
            return;
        }
        self.fifo.fetcher.step_ticks = 0;
        match self.fifo.fetcher.step {
            FetchStep::TileNo => {
                self.fifo.fetcher.y = if self.fifo.fetcher.window {
                    self.fifo.fetcher.y
                } else {
                    ly.wrapping_add(self.scy)
                };
                self.fifo.fetcher.scy_at_tile_no = self.scy;
                let (tile, attr) = self.fifo_fetch_tile_no(cgb);
                self.fifo.fetcher.tile = tile;
                self.fifo.fetcher.attr = attr;
                self.fifo.fetcher.step = FetchStep::TileDataLow;
            }
            FetchStep::TileDataLow => {
                // DMG re-samples SCY between TileNo and the data fetches;
                // CGB latches the TileNo row (rubc-core phase_bg_low_sample).
                if !self.fifo.fetcher.window && !cgb && self.scy != self.fifo.fetcher.scy_at_tile_no
                {
                    self.fifo.fetcher.y = ly.wrapping_add(self.scy);
                    let (tile, attr) = self.fifo_fetch_tile_no(cgb);
                    self.fifo.fetcher.tile = tile;
                    self.fifo.fetcher.attr = attr;
                }
                let addr = self.fifo_tile_data_addr(cgb, ly);
                self.fifo.fetcher.low = self.fifo_tile_data_byte(cgb, addr);
                self.fifo.fetcher.step = FetchStep::TileDataHigh;
            }
            FetchStep::TileDataHigh => {
                let addr = self.fifo_tile_data_addr(cgb, ly).wrapping_add(1);
                self.fifo.fetcher.high = self.fifo_tile_data_byte(cgb, addr);
                if self.fifo.fetcher.dummy_fetch_done {
                    self.fifo.fetcher.step = FetchStep::Push;
                } else {
                    self.fifo.fetcher.dummy_fetch_done = true;
                    self.fifo.fetcher.step = FetchStep::TileNo;
                }
            }
            FetchStep::Push => unreachable!("push handled before tick accounting"),
        }
    }

    fn fifo_fetch_tile_no(&self, cgb: bool) -> (u8, u8) {
        let map_base: u16 = if self.fifo.fetcher.window {
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
        let x_offset = if self.fifo.fetcher.window {
            self.fifo.fetcher.fetcher_x & 0x1F
        } else {
            self.fifo.fetcher.fetcher_x.wrapping_add(self.scx / 8) & 0x1F
        };
        let y_offset = 32 * ((u16::from(self.fifo.fetcher.y) / 8) & 0x1F);
        let offset = (y_offset + u16::from(x_offset)) & 0x03FF;
        let tile = self.vram.read(map_base + offset, 0).unwrap_or(0);
        let attr = if cgb {
            self.vram.read(map_base + offset, 1).unwrap_or(0)
        } else {
            0
        };
        (tile, attr)
    }

    fn fifo_tile_data_addr(&self, cgb: bool, ly: u8) -> u16 {
        // DMG samples SCY live during the data stages; window and CGB use the
        // row latched at TileNo (rubc-core fetch_bg_tile_data_addr).
        let y = if self.fifo.fetcher.window || cgb {
            self.fifo.fetcher.y
        } else {
            ly.wrapping_add(self.scy)
        };
        let mut row = u16::from(y & 0x07);
        if cgb && self.fifo.fetcher.attr & 0x40 != 0 {
            row = 7 - row;
        }
        if self.lcdc & 0x10 != 0 {
            u16::from(self.fifo.fetcher.tile) * 16 + row * 2
        } else {
            (0x1000_i32 + i32::from(self.fifo.fetcher.tile as i8) * 16 + i32::from(row) * 2) as u16
        }
    }

    fn fifo_tile_data_byte(&self, cgb: bool, addr: u16) -> u8 {
        let bank = if cgb && self.fifo.fetcher.attr & 0x08 != 0 {
            1
        } else {
            0
        };
        self.vram.read(addr, bank).unwrap_or(0)
    }

    /// Shift one pixel out of the FIFO. Returns (shifted, output): a discard
    /// (SCX&7 fine scroll) shifts without producing an LCD pixel and eats the
    /// matching OBJ FIFO slot (rubc-core shift_pixel).
    fn fifo_shift_pixel(&mut self, cgb: bool) -> (bool, Option<FifoOutput>) {
        let Some(bg) = self.fifo.bg_fifo.pop() else {
            return (false, None);
        };
        if self.fifo.scx_discard > 0 {
            self.fifo.scx_discard -= 1;
            let _ = self.fifo.sprite_fifo.pop();
            return (true, None);
        }
        let sprite = self.fifo.sprite_fifo.pop().unwrap_or_default();
        let pixel = if cgb {
            self.fifo_resolve_cgb(bg, sprite)
        } else {
            self.fifo_resolve_dmg(bg, sprite)
        };
        let x = self.fifo.lcd_x;
        self.fifo.lcd_x += 1;
        if self.fifo.lcd_x >= 160 {
            self.fifo.active = false;
        }
        (true, Some(FifoOutput { x, pixel }))
    }

    fn fifo_resolve_dmg(&self, bg: FifoPixel, sprite: FifoPixel) -> RenderedPpuPixel {
        let bg_color = if self.lcdc & 0x01 == 0 {
            0
        } else {
            bg.color & 0x03
        };
        let sprite_wins =
            sprite.occupied && sprite.color != 0 && !(sprite.bg_priority && bg_color != 0);
        if sprite_wins {
            RenderedPpuPixel {
                raw_color: sprite.color & 0x03,
                source: LcdPixelSource::Obj(sprite.palette.unwrap_or(SpritePalette::Obp0)),
                cgb_palette: sprite.cgb_palette,
            }
        } else {
            RenderedPpuPixel {
                raw_color: bg_color,
                source: LcdPixelSource::Bg,
                cgb_palette: bg.cgb_palette,
            }
        }
    }

    /// CGB BG/OBJ priority (Pan Docs table): LCDC.0 clear means OBJ always
    /// wins; otherwise BG color 0 loses; otherwise either priority bit keeps
    /// BG 1-3 in front.
    fn fifo_resolve_cgb(&self, bg: FifoPixel, sprite: FifoPixel) -> RenderedPpuPixel {
        let bg_color = bg.color & 0x03;
        let sprite_opaque = sprite.occupied && sprite.color != 0;
        let obj_master = self.lcdc & 0x01 == 0;
        let bg_has_priority = bg.bg_priority || sprite.bg_priority;
        let sprite_wins = sprite_opaque && (obj_master || bg_color == 0 || !bg_has_priority);
        if sprite_wins {
            RenderedPpuPixel {
                raw_color: sprite.color & 0x03,
                source: LcdPixelSource::Obj(sprite.palette.unwrap_or(SpritePalette::Obp0)),
                cgb_palette: sprite.cgb_palette,
            }
        } else {
            RenderedPpuPixel {
                raw_color: bg_color,
                source: LcdPixelSource::Bg,
                cgb_palette: bg.cgb_palette,
            }
        }
    }

    fn write_register(&mut self, write: PpuRegisterWrite) {
        match write.addr {
            0xFF40 => {
                let old = self.lcdc;
                self.lcdc = write.value;
                if write.value & 0x80 == 0 {
                    // LCD off: the line render state and frame window state
                    // reset (rubc-core write_lcdc LCD-off path).
                    self.fifo = LineRenderState::default();
                    self.fifo_window_y_condition = false;
                    self.fifo_window_line = 0;
                } else if self.fifo.active && (old ^ write.value) & 0x20 != 0 {
                    // LCDC.5 toggled mid-line: a disable takes effect once the
                    // BG FIFO drains (rubc-core window_disable_pending).
                    if old & 0x20 != 0 && self.fifo.fetcher.window {
                        self.fifo.window_disable_pending = true;
                    } else if write.value & 0x20 != 0 {
                        self.fifo.window_disable_pending = false;
                    }
                }
            }
            0xFF42 => self.scy = write.value,
            0xFF43 => self.scx = write.value,
            0xFF4A => self.wy = write.value,
            0xFF4B => self.wx = write.value,
            _ => {}
        }
    }

    fn sample_bg_fetch(&mut self, row: &GoldenV2Row) -> Result<Option<BgFetchSample>, String> {
        if row.kind != "ppu_internal" || row.event.as_deref() != Some("fetch") {
            return Ok(None);
        }
        let stage = row
            .state
            .as_deref()
            .ok_or_else(|| "fetch row missing state".to_owned())?;
        let ly = row.ly.ok_or_else(|| "fetch row missing LY".to_owned())?;
        let (addr, byte) = match stage {
            "GET_TILE_T1" => {
                let addr = self.tilemap_addr(ly);
                let byte = self.vram.read(addr, 0)?;
                self.current_tile_data_addr = Some(self.tile_data_addr(byte, ly));
                (addr, byte)
            }
            "GET_TILE_DATA_LOWER_T1" => {
                let addr = self
                    .current_tile_data_addr
                    .ok_or_else(|| "lower-data fetch before machine tile-id fetch".to_owned())?;
                (addr, self.vram.read(addr, 0)?)
            }
            "GET_TILE_DATA_HIGH_T1" => {
                let addr = self
                    .current_tile_data_addr
                    .ok_or_else(|| "high-data fetch before machine tile-id fetch".to_owned())?
                    .wrapping_add(1);
                let byte = self.vram.read(addr, 0)?;
                self.fetcher_x = self.fetcher_x.wrapping_add(1);
                self.current_tile_data_addr = None;
                (addr, byte)
            }
            _ => return Ok(None),
        };

        Ok(Some(BgFetchSample {
            raw_tick: row.raw_tick,
            ly,
            stage: stage.to_owned(),
            fetcher_x: self.fetcher_x,
            machine_addr: addr,
            machine_byte: byte,
        }))
    }

    fn window_triggered(&self, ly: u16, screen_x: usize) -> bool {
        self.lcdc & 0x20 != 0
            && ly >= u16::from(self.wy)
            && screen_x.saturating_add(7) >= usize::from(self.wx)
    }

    fn bg_or_window_pixel(&self, cgb: bool, ly: u8, x: usize) -> (u8, u8, bool) {
        if self.lcdc & 0x01 == 0 && !cgb {
            return (0, 0, false);
        }
        let window = self.window_triggered(u16::from(ly), x);
        let (map_base, map_x, map_y) = if window {
            let wx = usize::from(self.wx.saturating_sub(7));
            let x = x.saturating_sub(wx);
            let y = ly.saturating_sub(self.wy);
            (
                if self.lcdc & 0x40 != 0 {
                    0x1C00
                } else {
                    0x1800
                },
                x,
                usize::from(y),
            )
        } else {
            (
                if self.lcdc & 0x08 != 0 {
                    0x1C00
                } else {
                    0x1800
                },
                x.wrapping_add(usize::from(self.scx)),
                usize::from(ly.wrapping_add(self.scy)),
            )
        };
        let map_addr = map_base + ((map_y / 8) & 31) as u16 * 32 + ((map_x / 8) & 31) as u16;
        let tile = self.vram.read(map_addr, 0).unwrap_or(0);
        let attr = if cgb {
            self.vram.read(map_addr, 1).unwrap_or(0)
        } else {
            0
        };
        let mut fine_x = (map_x & 7) as u8;
        let mut fine_y = (map_y & 7) as u8;
        if attr & 0x20 != 0 {
            fine_x = 7 - fine_x;
        }
        if attr & 0x40 != 0 {
            fine_y = 7 - fine_y;
        }
        let bank = if cgb { (attr >> 3) & 1 } else { 0 };
        let addr = self.tile_data_addr_for(tile, fine_y);
        let lo = self.vram.read(addr, bank).unwrap_or(0);
        let hi = self.vram.read(addr.wrapping_add(1), bank).unwrap_or(0);
        (
            ((lo >> (7 - fine_x)) & 1) | (((hi >> (7 - fine_x)) & 1) << 1),
            attr & 7,
            attr & 0x80 != 0,
        )
    }

    fn sprite_pixel(
        &self,
        cgb: bool,
        ly: u8,
        x: usize,
        sprite: SelectedSprite,
    ) -> Option<(u8, u8)> {
        let sx = usize::from(sprite.x.wrapping_sub(8));
        let sy = sprite.y.wrapping_sub(16);
        if x < sx || x >= sx + 8 || ly < sy || ly >= sy.wrapping_add(sprite_height(sprite.size)) {
            return None;
        }
        let mut fine_x = (x - sx) as u8;
        let mut fine_y = ly.wrapping_sub(sy);
        if sprite.attr & 0x20 != 0 {
            fine_x = 7 - fine_x;
        }
        if sprite.attr & 0x40 != 0 {
            fine_y = sprite_height(sprite.size) - 1 - fine_y;
        }
        let tile = if sprite.size == SpriteSize::Size8x16 {
            (sprite.tile & !1).wrapping_add(u8::from(fine_y >= 8))
        } else {
            sprite.tile
        };
        let fine_y = fine_y & 7;
        let bank = if cgb { (sprite.attr >> 3) & 1 } else { 0 };
        let addr = u16::from(tile) * 16 + u16::from(fine_y) * 2;
        let lo = self.vram.read(addr, bank).ok()?;
        let hi = self.vram.read(addr.wrapping_add(1), bank).ok()?;
        Some((
            ((lo >> (7 - fine_x)) & 1) | (((hi >> (7 - fine_x)) & 1) << 1),
            sprite.attr & 7,
        ))
    }

    fn tile_data_addr_for(&self, tile_id: u8, fine_y: u8) -> u16 {
        let fine_y = u16::from(fine_y) * 2;
        if self.lcdc & 0x10 != 0 {
            u16::from(tile_id) * 16 + fine_y
        } else {
            (0x1000_i32 + i32::from(tile_id as i8) * 16 + i32::from(fine_y)) as u16
        }
    }

    fn tilemap_addr(&self, ly: u16) -> u16 {
        if self.window_active {
            self.window_tilemap_addr()
        } else {
            self.bg_tilemap_addr(ly)
        }
    }

    fn bg_tilemap_addr(&self, ly: u16) -> u16 {
        let base = if self.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };
        let y = self.scy.wrapping_add(ly as u8);
        let row = u16::from(y >> 3) * 32;
        let col = (u16::from(self.scx >> 3) + self.fetcher_x) & 31;
        base + row + col
    }

    fn window_tilemap_addr(&self) -> u16 {
        let base = if self.lcdc & 0x40 != 0 {
            0x1C00
        } else {
            0x1800
        };
        let row = u16::from(self.window_line >> 3) * 32;
        let col = self.fetcher_x & 31;
        base + row + col
    }

    fn tile_data_addr(&self, tile_id: u8, ly: u16) -> u16 {
        let fine_source_y = if self.window_active {
            self.window_line
        } else {
            self.scy.wrapping_add(ly as u8)
        };
        let fine_y = u16::from(fine_source_y & 7) * 2;
        if self.lcdc & 0x10 != 0 {
            u16::from(tile_id) * 16 + fine_y
        } else {
            (0x1000_i32 + i32::from(tile_id as i8) * 16 + i32::from(fine_y)) as u16
        }
    }
}

fn select_scanline_sprites(lcdc: u8, oam: &[u8; 0xA0], ly: u8) -> Vec<SelectedSprite> {
    let mut selected = Vec::with_capacity(10);
    let size = if lcdc & 0x04 != 0 {
        SpriteSize::Size8x16
    } else {
        SpriteSize::Size8x8
    };
    let height = match size {
        SpriteSize::Size8x8 => 8,
        SpriteSize::Size8x16 => 16,
    };

    for (index, sprite) in oam.chunks_exact(4).enumerate() {
        let y = sprite[0];
        let ly_plus_16 = ly.wrapping_add(16);
        if ly_plus_16 < y || ly_plus_16 >= y.wrapping_add(height) {
            continue;
        }
        selected.push(SelectedSprite {
            y,
            x: sprite[1],
            tile: if size == SpriteSize::Size8x16 {
                sprite[2] & !1
            } else {
                sprite[2]
            },
            attr: sprite[3],
            oam_index: index as u8,
            palette: if sprite[3] & 0x10 != 0 {
                SpritePalette::Obp1
            } else {
                SpritePalette::Obp0
            },
            bg_priority: sprite[3] & 0x80 != 0,
            size,
        });
        if selected.len() == 10 {
            break;
        }
    }
    selected
}

fn sprite_height(size: SpriteSize) -> u8 {
    match size {
        SpriteSize::Size8x8 => 8,
        SpriteSize::Size8x16 => 16,
    }
}

impl Default for SelectedSprite {
    fn default() -> Self {
        Self {
            y: 0,
            x: 0,
            tile: 0,
            attr: 0,
            oam_index: 0,
            palette: SpritePalette::Obp0,
            bg_priority: false,
            size: SpriteSize::Size8x8,
        }
    }
}

pub fn assert_bg_fetch_golden(path: impl AsRef<Path>) -> Result<(), BgFetchDivergence> {
    assert_bg_fetch_golden_inner(path.as_ref(), None)
}

pub fn assert_bg_fetch_golden_with_perturbation(
    path: impl AsRef<Path>,
    vram_addr: u16,
    value: u8,
) -> Result<(), BgFetchDivergence> {
    assert_bg_fetch_golden_inner(path.as_ref(), Some((vram_addr, value)))
}

fn assert_bg_fetch_golden_inner(
    path: &Path,
    perturb_vram: Option<(u16, u8)>,
) -> Result<(), BgFetchDivergence> {
    let rom = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<golden>")
        .to_owned();
    let initial = GoldenV2Reader::read_initial_state(path).map_err(|err| BgFetchDivergence {
        rom: rom.clone(),
        index: 0,
        raw_tick: 0,
        ly: 0,
        stage: err,
        machine_addr: 0,
        golden_addr: 0,
        machine_byte: 0,
        golden_byte: 0,
    })?;
    let mut vram_state =
        GoldenV2Reader::read_vram_state(path).map_err(|err| BgFetchDivergence {
            rom: rom.clone(),
            index: 0,
            raw_tick: 0,
            ly: 0,
            stage: err,
            machine_addr: 0,
            golden_addr: 0,
            machine_byte: 0,
            golden_byte: 0,
        })?;
    if let Some((addr, value)) = perturb_vram {
        vram_state
            .vram
            .write_for_test(addr, 0, value)
            .map_err(|err| BgFetchDivergence {
                rom: rom.clone(),
                index: 0,
                raw_tick: 0,
                ly: 0,
                stage: err,
                machine_addr: 0,
                golden_addr: 0,
                machine_byte: 0,
                golden_byte: 0,
            })?;
    }
    let writes = extract_register_writes(path).map_err(|err| BgFetchDivergence {
        rom: rom.clone(),
        index: 0,
        raw_tick: 0,
        ly: 0,
        stage: err,
        machine_addr: 0,
        golden_addr: 0,
        machine_byte: 0,
        golden_byte: 0,
    })?;
    let fetch_rows = GoldenV2Reader::open(path)
        .map_err(|err| BgFetchDivergence {
            rom: rom.clone(),
            index: 0,
            raw_tick: 0,
            ly: 0,
            stage: err,
            machine_addr: 0,
            golden_addr: 0,
            machine_byte: 0,
            golden_byte: 0,
        })?
        .filter_map(|row| match row {
            Ok(row) if row.kind == "ppu_internal" && row.event.as_deref() == Some("fetch") => {
                Some(Ok(row))
            }
            Ok(_) => None,
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| BgFetchDivergence {
            rom: rom.clone(),
            index: 0,
            raw_tick: 0,
            ly: 0,
            stage: err,
            machine_addr: 0,
            golden_addr: 0,
            machine_byte: 0,
            golden_byte: 0,
        })?;

    let mut ppu = PpuInternal::from_golden_state(initial, vram_state);
    let mut next_write = 0;
    for (index, row) in fetch_rows.iter().enumerate() {
        let now = Time::from_subphases(row.raw_tick);
        while writes
            .get(next_write)
            .is_some_and(|write| write.time.subphases() < now.subphases())
        {
            ppu.write_register(writes[next_write]);
            next_write += 1;
        }
        let Some(sample) = ppu.sample_bg_fetch(row).map_err(|err| BgFetchDivergence {
            rom: rom.clone(),
            index,
            raw_tick: row.raw_tick,
            ly: row.ly.unwrap_or(0),
            stage: err,
            machine_addr: 0,
            golden_addr: row.addr.unwrap_or(0),
            machine_byte: 0,
            golden_byte: row.byte.unwrap_or(0),
        })?
        else {
            continue;
        };
        let golden_addr = row.addr.unwrap_or(u16::MAX);
        let golden_byte = row.byte.unwrap_or(u8::MAX);
        if sample.machine_addr != golden_addr || sample.machine_byte != golden_byte {
            return Err(BgFetchDivergence {
                rom: rom.clone(),
                index,
                raw_tick: sample.raw_tick,
                ly: sample.ly,
                stage: sample.stage,
                machine_addr: sample.machine_addr,
                golden_addr,
                machine_byte: sample.machine_byte,
                golden_byte,
            });
        }
    }
    Ok(())
}

fn extract_register_writes(path: &Path) -> Result<Vec<PpuRegisterWrite>, String> {
    let mut writes = GoldenV2Reader::open(path)?
        .filter_map(|row| match row {
            Ok(row) if row.kind == "cpu" => match (row.addr, row.byte) {
                (Some(addr @ (0xFF40 | 0xFF42 | 0xFF43 | 0xFF4A | 0xFF4B)), Some(value)) => {
                    Some(Ok(PpuRegisterWrite {
                        time: Time::from_subphases(row.write_visible_tick.unwrap_or(row.raw_tick)),
                        addr,
                        value,
                    }))
                }
                _ => None,
            },
            Ok(_) => None,
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    writes.sort_by_key(|write| write.time);
    Ok(writes)
}
