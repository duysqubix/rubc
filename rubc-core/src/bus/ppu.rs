//! DMG PPU **mode scheduler** (ticket rubc-9d4).
//!
//! This drives the PPU's per-dot state machine: modes 0/1/2/3 across 456 dots
//! per scanline and 154 scanlines per frame, the LY/LYC compare, the STAT
//! register, and the VBlank + STAT interrupts. It also reports VRAM/OAM access
//! blocking by mode.
//!
//! **Scope:** timing + registers + interrupts only. There is NO pixel output /
//! framebuffer yet -- the pixel FIFO (BG/window/sprite fetch) is the separate
//! `rubc-fde` wave. Consequently a few timing details are intentionally
//! approximated here and deferred (each marked with a TODO):
//!   - Mode 3 length is fixed at 172 dots. On hardware it is 172..=289 depending
//!     on SCX, the window, and sprites -- that variability is a FIFO-wave
//!     concern (`rubc-fde`).
//!   - The LY=153 -> LY=0 early-read quirk (LY reads 0 a few dots into line 153)
//!     is not modelled; LY simply sequences 0..=153 then wraps.
//!   - The "first frame after LCD enable is shorter / mode 2 timing differs"
//!     behaviour is not modelled; enabling the LCD restarts cleanly from LY=0.
//!
//! Reference: Pan Docs `Rendering.md`, `STAT.md`, `LCDC.md`,
//! `Accessing_VRAM_and_OAM.md`; GBEDG `ppu/index.md`.

use super::stubs::Interrupts;

/// Dots per scanline (mode 2 + mode 3 + mode 0 always sum to this).
const DOTS_PER_LINE: u32 = 456;
/// Mode 2 (OAM scan) duration in dots.
const MODE2_DOTS: u32 = 80;
/// Mode 3 (drawing) duration in dots. Fixed for now; see module docs / rubc-fde.
const MODE3_DOTS: u32 = 172;
/// Last visible scanline (LY 0..=143 are visible; 144..=153 are VBlank).
const LAST_VISIBLE_LINE: u8 = 143;
/// Last scanline before LY wraps back to 0.
const LAST_LINE: u8 = 153;

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

/// The DMG PPU mode scheduler.
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
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            dot_ticks: 0,
            ly: 0,
            mode: mode::OAM_SCAN,
            // LCD starts enabled with the post-boot LCDC ($91 = on, BG on, ...).
            enabled: true,
            lcdc: 0x91,
            line_dot: 0,
            lyc: 0,
            stat_enables: 0,
            coincidence: true, // LY=0, LYC=0 at power-on
            stat_line: false,
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
    /// `irq`.
    pub fn tick_dot(&mut self, irq: &mut Interrupts) {
        self.dot_ticks += 1;
        if !self.enabled {
            return;
        }

        self.line_dot += 1;
        if self.line_dot >= DOTS_PER_LINE {
            // End of scanline: advance LY (wrapping 153 -> 0).
            self.line_dot = 0;
            self.ly = if self.ly >= LAST_LINE { 0 } else { self.ly + 1 };
            self.update_coincidence();
        }

        self.update_mode(irq);
        self.update_stat_line(irq);
    }

    /// Recompute the current mode from (LY, line_dot) and raise VBlank on the
    /// HBlank/VBlank entry into line 144.
    fn update_mode(&mut self, irq: &mut Interrupts) {
        let new_mode = if self.ly > LAST_VISIBLE_LINE {
            mode::VBLANK
        } else if self.line_dot < MODE2_DOTS {
            mode::OAM_SCAN
        } else if self.line_dot < MODE2_DOTS + MODE3_DOTS {
            mode::DRAWING
        } else {
            mode::HBLANK
        };

        if new_mode != self.mode {
            self.mode = new_mode;
            // VBlank interrupt fires once, when the PPU first enters mode 1
            // (i.e. at the start of line 144).
            if new_mode == mode::VBLANK {
                irq.request(INT_VBLANK);
            }
        }
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
            // NOTE: the LYC coincidence flag is RETAINED while the comparison
            // clock is stopped (mooneye stat_lyc_onoff) -- do NOT recompute it,
            // and do NOT clear the STAT line here.
            self.ly = 0;
            self.line_dot = 0;
            self.mode = mode::HBLANK;
        } else if !was_on && self.enabled {
            // LCD on: restart the frame from the top.
            // TODO(rubc-9d4 lcdon wave): the first line after enable starts in
            // mode 0 (not mode 2) and has special shorter timing. We restart in
            // mode 2 for now; lcdon_timing-GS is gated to that wave.
            self.ly = 0;
            self.line_dot = 0;
            self.mode = mode::OAM_SCAN;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A PPU with the LCD on and a clean schedule from LY=0, dot 0.
    fn ppu_at_line_start() -> Ppu {
        let mut p = Ppu::new();
        p.enabled = true;
        p.ly = 0;
        p.line_dot = 0;
        p.mode = mode::OAM_SCAN;
        p
    }

    /// Tick `n` dots and return the settled IF mask (the bits the PPU
    /// requested over those dots, as they would become visible at a boundary).
    fn tick(p: &mut Ppu, n: u32) -> u8 {
        let mut irq = Interrupts::default();
        for _ in 0..n {
            p.tick_dot(&mut irq);
        }
        irq.settle_boundary();
        irq.if_ & 0x1F
    }

    #[test]
    fn mode_sequence_within_visible_line() {
        let mut p = ppu_at_line_start();
        // Dot 0..79 = mode 2 (OAM scan).
        assert_eq!(p.mode, mode::OAM_SCAN);
        tick(&mut p, 79);
        assert_eq!(p.mode, mode::OAM_SCAN, "still OAM scan at dot 79");
        // Dot 80 = enter mode 3 (drawing).
        tick(&mut p, 1);
        assert_eq!(p.mode, mode::DRAWING, "mode 3 at dot 80");
        // Dot 80+172 = 252 = enter mode 0 (HBlank).
        tick(&mut p, MODE3_DOTS - 1);
        assert_eq!(p.mode, mode::DRAWING, "still drawing at dot 251");
        tick(&mut p, 1);
        assert_eq!(p.mode, mode::HBLANK, "HBlank at dot 252");
    }

    #[test]
    fn ly_increments_at_line_end_and_wraps() {
        let mut p = ppu_at_line_start();
        assert_eq!(p.ly, 0);
        tick(&mut p, DOTS_PER_LINE); // one full line
        assert_eq!(p.ly, 1, "LY increments after 456 dots");
        // Run to the end of the frame: from LY=1, 152 more lines -> LY wraps 0.
        tick(&mut p, DOTS_PER_LINE * (LAST_LINE as u32)); // lines 1..=153 then wrap
        assert_eq!(p.ly, 0, "LY wraps 153 -> 0");
    }

    #[test]
    fn entering_vblank_requests_vblank_irq() {
        let mut p = ppu_at_line_start();
        // Advance to the start of line 144 (144 full lines of dots).
        let irq = tick(&mut p, DOTS_PER_LINE * 144);
        assert_eq!(p.ly, 144);
        assert_eq!(p.mode, mode::VBLANK);
        assert!(irq & 0x01 != 0, "VBlank IRQ requested");
    }

    #[test]
    fn stat_mode0_rising_edge_fires_once() {
        let mut p = ppu_at_line_start();
        p.stat_enables = 0x08; // mode 0 (HBlank) interrupt enabled
        // Advance into HBlank (dot 252).
        let irq = tick(&mut p, MODE2_DOTS + MODE3_DOTS);
        assert_eq!(p.mode, mode::HBLANK);
        assert!(irq & 0x02 != 0, "STAT fires entering mode 0");
        // Staying in HBlank must NOT re-raise (blocking / edge-triggered).
        let irq2 = tick(&mut p, 10);
        assert_eq!(irq2 & 0x02, 0, "no re-raise while in mode 0");
    }

    #[test]
    fn lyc_coincidence_fires_stat_once() {
        let mut p = ppu_at_line_start();
        p.stat_enables = 0x40; // LYC interrupt enabled
        let mut irq = Interrupts::default();
        p.write_lyc(1, &mut irq); // match line 1
        assert!(!p.coincidence, "LY=0 != LYC=1 yet");
        // Advance one full line -> LY=1 == LYC.
        let irq = tick(&mut p, DOTS_PER_LINE);
        assert_eq!(p.ly, 1);
        assert!(p.coincidence);
        assert!(irq & 0x02 != 0, "STAT fires on LYC match");
    }

    #[test]
    fn lcd_off_resets_and_unblocks() {
        let mut p = ppu_at_line_start();
        // Get into mode 3 (VRAM blocked).
        tick(&mut p, MODE2_DOTS + 1);
        assert_eq!(p.mode, mode::DRAWING);
        assert!(p.vram_blocked());
        // Turn the LCD off.
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
        // Mode 2: OAM blocked, VRAM accessible.
        assert_eq!(p.mode, mode::OAM_SCAN);
        assert!(p.oam_blocked());
        assert!(!p.vram_blocked());
        // Mode 3: both blocked.
        tick(&mut p, MODE2_DOTS);
        assert_eq!(p.mode, mode::DRAWING);
        assert!(p.oam_blocked());
        assert!(p.vram_blocked());
        // Mode 0: neither blocked.
        tick(&mut p, MODE3_DOTS);
        assert_eq!(p.mode, mode::HBLANK);
        assert!(!p.oam_blocked());
        assert!(!p.vram_blocked());
    }

    #[test]
    fn lcd_off_retains_coincidence_and_lyc_write_is_inert() {
        // mooneye stat_lyc_onoff: while the LCD is off the LYC comparison clock
        // is stopped, so the coincidence flag is RETAINED and LYC writes do not
        // recompute it or raise STAT.
        let mut p = ppu_at_line_start();
        let mut irq = Interrupts::default();
        p.stat_enables = 0x40; // LYC int enabled
        // Reach a NONZERO matching line: LY=144, LYC=144 -> coincident.
        p.write_lyc(144, &mut irq);
        tick(&mut p, DOTS_PER_LINE * 144); // advance to LY=144
        assert_eq!(p.ly, 144);
        assert!(p.coincidence, "LY=144==LYC=144 -> coincident");
        // Turn LCD off: LY resets to 0, but coincidence must be RETAINED (not
        // recomputed -- a recompute against LY=0,LYC=144 would clear it).
        p.write_lcdc(0x00, &mut irq);
        assert_eq!(p.ly, 0, "LY reset on LCD off");
        assert!(p.coincidence, "coincidence RETAINED while LCD off (not recomputed)");
        // Writing LYC while off does NOT recompute or raise STAT.
        let mut irq2 = Interrupts::default();
        p.write_lyc(50, &mut irq2);
        assert_eq!(p.lyc, 50, "LYC value still stored while off");
        assert!(p.coincidence, "coincidence NOT recomputed while off");
        irq2.settle_boundary();
        assert_eq!(irq2.if_ & 0x02, 0, "no STAT raised by LYC write while off");
    }

    #[test]
    fn lcd_reenable_fires_stat_only_on_false_to_true() {
        // STAT rising-edge across an LCD off/on cycle:
        //   retained TRUE -> recomputed TRUE  = NO IRQ (true->true)
        //   retained FALSE -> recomputed TRUE = ONE IRQ (false->true)
        let mut p = ppu_at_line_start();
        let mut irq = Interrupts::default();
        p.stat_enables = 0x40; // LYC int enabled

        // --- Case A: true -> true must NOT fire ---
        // LY=0, LYC=0 coincident (stat_line already true).
        p.write_lyc(0, &mut irq);
        assert!(p.coincidence && p.stat_line, "primed: coincident + stat_line true");
        p.write_lcdc(0x00, &mut irq); // off: coincidence + stat_line retained true
        assert!(p.coincidence, "retained true while off");
        let mut irq_a = Interrupts::default();
        p.write_lcdc(0x80, &mut irq_a); // on: LY=0==LYC=0 still true -> true->true
        irq_a.settle_boundary();
        assert_eq!(irq_a.if_ & 0x02, 0, "true->true on re-enable must NOT fire STAT");

        // --- Case B: false -> true must fire exactly once ---
        // Make the retained line FALSE: set LYC to a non-matching value while on,
        // then advance off the match so coincidence is false before turning off.
        p.write_lyc(5, &mut irq); // LY=0 != 5 -> coincidence false, stat_line false
        assert!(!p.coincidence && !p.stat_line, "now false");
        p.write_lcdc(0x00, &mut irq); // off: retained false
        // On re-enable LY=0; set LYC=0 first is inert while off, so do it, then on.
        p.write_lyc(0, &mut irq); // inert while off (LY stays whatever; recompute on enable)
        let mut irq_b = Interrupts::default();
        p.write_lcdc(0x80, &mut irq_b); // on: LY=0==LYC=0 -> false->true
        irq_b.settle_boundary();
        assert_eq!(irq_b.if_ & 0x02, 0x02, "false->true on re-enable fires STAT once");
    }
}
