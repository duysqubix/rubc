//! W8b·2b-fifo (rubc-d85o): the real per-dot pixel FIFO data structures.
//!
//! Hardware model provenance:
//! - TCAGBD (reference/docs/TCAGBD.txt) pixel-FIFO chapter: two FIFOs (BG +
//!   OBJ), fetcher pushes 8 pixels at a time, one pixel shifts out per dot.
//! - SameBoy Core/display.c: window trigger at WX == position+7 with fetcher
//!   restart, window line counter advanced at activation, fine-X (SCX&7)
//!   discard at line start, object staging that merges into the OBJ FIFO
//!   (transparent slots fill; CGB lower-OAM-index replaces).
//! - Pan Docs pixel_fifo.md: fetcher steps (tile no / data low / data high /
//!   push) at 2 dots each, push only into an empty BG FIFO, the first tile
//!   fetch of a line is discarded (12-dot mode-3 startup).
//!
//! Geometry is cross-checked against rubc-core's acid-proven FIFO renderer
//! (rubc-core/src/bus/ppu.rs) which passes dmg-acid2 + cgb-acid2 at 0 diff.

use crate::ppu_internal::{SelectedSprite, SpritePalette};

pub(crate) const FIFO_CAPACITY: usize = 8;

/// One staged pixel. BG pixels keep `palette = None`; sprite pixels carry the
/// DMG OBP selection and the OAM index used for CGB merge priority.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FifoPixel {
    pub color: u8,
    pub bg_priority: bool,
    pub occupied: bool,
    pub palette: Option<SpritePalette>,
    pub cgb_palette: u8,
    pub oam_index: u8,
}

/// An 8-slot shift register (TCAGBD: the FIFO holds up to 8 pixels; SameBoy
/// fifo.h GB_fifo_t). `push_bg_pixels` refills all 8; `pop` shifts one out.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PixelFifo {
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
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push_bg_pixels(&mut self, colors: [u8; FIFO_CAPACITY], cgb_palette: u8, bg_priority: bool) {
        self.len = FIFO_CAPACITY;
        for (slot, color) in self.pixels.iter_mut().zip(colors) {
            *slot = FifoPixel {
                color: color & 0x03,
                bg_priority,
                occupied: true,
                palette: None,
                cgb_palette,
                oam_index: 0,
            };
        }
    }

    /// Merge a fetched sprite row into the OBJ FIFO (SameBoy display.c object
    /// rendering: occupied opaque slots survive on DMG; on CGB a lower OAM
    /// index replaces a higher one).
    pub fn overlay_sprite_pixels(
        &mut self,
        colors: [u8; FIFO_CAPACITY],
        first_visible_pixel: usize,
        attrs: SpriteOverlay,
    ) {
        for (slot, color) in colors.iter().copied().skip(first_visible_pixel).enumerate() {
            if slot >= FIFO_CAPACITY {
                break;
            }
            if color == 0 {
                continue;
            }
            let existing = self.pixels[slot];
            if existing.occupied && (!attrs.cgb_priority || attrs.oam_index >= existing.oam_index) {
                continue;
            }
            self.pixels[slot] = FifoPixel {
                color: color & 0x03,
                bg_priority: attrs.bg_priority,
                occupied: true,
                palette: Some(attrs.palette),
                cgb_palette: attrs.cgb_palette,
                oam_index: attrs.oam_index,
            };
            self.len = self.len.max(slot + 1);
        }
    }

    pub fn pop(&mut self) -> Option<FifoPixel> {
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpriteOverlay {
    pub bg_priority: bool,
    pub palette: SpritePalette,
    pub cgb_palette: u8,
    pub oam_index: u8,
    /// CGB merge rule: lower OAM index replaces an occupied slot.
    pub cgb_priority: bool,
}

/// Pan Docs pixel_fifo.md fetcher steps; each takes 2 dots, Push retries every
/// dot until the BG FIFO is empty.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum FetchStep {
    #[default]
    TileNo,
    TileDataLow,
    TileDataHigh,
    Push,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BgFetcher {
    pub step: FetchStep,
    pub step_ticks: u8,
    pub fetcher_x: u8,
    pub tile: u8,
    /// CGB BG map attribute byte (VRAM bank 1). 0 on DMG.
    pub attr: u8,
    pub low: u8,
    pub high: u8,
    /// Pan Docs: the first tile fetch of a scanline is thrown away ("the
    /// fetcher is reset once it reaches the push step for the first time").
    pub dummy_fetch_done: bool,
    pub window: bool,
    /// Map Y latched at the TileNo step (window activations preload the
    /// window line counter here).
    pub y: u8,
    pub scy_at_tile_no: u8,
}

impl BgFetcher {
    /// Full reset (scanline start / window trigger): resets everything
    /// including the internal X-position counter.
    pub fn reset(&mut self, window: bool, dummy_fetch_done: bool) {
        *self = Self {
            window,
            dummy_fetch_done,
            ..Self::default()
        };
    }

    /// Sprite-fetch reset (GBEDG: a sprite fetch resets the BG fetcher to its
    /// first step and pauses it, preserving the X counter / window state).
    pub fn reset_for_sprite(&mut self) {
        self.step = FetchStep::default();
        self.step_ticks = 0;
        self.tile = 0;
        self.low = 0;
        self.high = 0;
    }
}

/// Everything the FIFO needs per scanline; rebuilt at each mode-3 start.
#[derive(Clone, Debug, Default)]
pub(crate) struct LineRenderState {
    pub active: bool,
    pub bg_fifo: PixelFifo,
    pub sprite_fifo: PixelFifo,
    pub fetcher: BgFetcher,
    pub lcd_x: usize,
    /// SCX&7 fine-scroll pixels withheld at line start (SameBoy display.c:
    /// pixels shipped while position_in_line is negative are discarded).
    pub scx_discard: u8,
    /// Scanline sprites sorted by (x, oam_index): the order their trigger X
    /// is reached. CGB priority is applied at merge time instead.
    pub sprites: Vec<SelectedSprite>,
    pub next_sprite: usize,
    pub pending_sprite: Option<SelectedSprite>,
    pub sprite_fetch_ticks: u8,
    pub sprite_idle_ticks: u8,
    pub window_active: bool,
    pub window_started_this_line: bool,
    pub window_disable_pending: bool,
}

impl LineRenderState {
    pub fn begin(sprites: Vec<SelectedSprite>, scx_discard: u8) -> Self {
        Self {
            active: true,
            sprites,
            scx_discard,
            ..Self::default()
        }
    }
}

/// Decode one 2bpp tile row into 8 left-to-right pixel colors.
pub(crate) fn decode_2bpp(low: u8, high: u8, x_flip: bool) -> [u8; FIFO_CAPACITY] {
    let mut colors = [0u8; FIFO_CAPACITY];
    for (i, color) in colors.iter_mut().enumerate() {
        let bit = if x_flip { i } else { 7 - i };
        *color = ((low >> bit) & 1) | (((high >> bit) & 1) << 1);
    }
    colors
}
