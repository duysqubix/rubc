//! M-cycle bus + `CpuBus` trait + the per-M-cycle access-placement invariant.
//!
//! ## The invariant (Oracle-locked; T-cycle migration rubc-td3)
//!
//! Every CPU access runs through `run_cpu_access`, in EXACTLY this order:
//!   1. **OAM-DMA beat** — transfer one byte, record `conflict_byte_this_m` +
//!      `active_for_cpu_this_m` (so a conflicting CPU access this M-cycle sees
//!      the in-flight DMA byte).
//!   2. **T-cycle ticks split around the access** — timer / PPU / APU advance.
//!      TAC commits mid-M (tick 2 → commit → tick 2), PPU-visible IO writes
//!      commit late-M (tick 3 → commit → tick 1), and every other write commits
//!      at end-of-M. A READ/idle ticks all 4 T then samples at end-of-M (the
//!      correct DMG placement; Oracle ses_16262cca4 confirmed reads must NOT
//!      move to T3). The timer always advances 4 per M; in CGB double-speed
//!      PPU+APU advance every 2nd T.
//!   3. IRQs raised during the ticks latch IF immediately. The CPU still polls
//!      and dispatches only at instruction boundaries.
//!
//! Diagnostics OBSERVE only; nothing here is on a diag hot path that ticks.

pub mod apu;
pub mod cartridge;
pub mod flat;
pub mod ppu;
pub mod serial;
pub mod sm83_vectors;
pub mod stubs;
pub mod timer;

pub use cartridge::Cartridge;
pub use flat::FlatBus;
pub use ppu::{CgbRenderState, DmgPalettes, Ppu};

use apu::Apu;
use serial::Serial;
pub use stubs::Button;
use stubs::{CgbState, Interrupts, Joypad};
use timer::Timer;

fn read_oam_word(oam: &[u8; 0xA0], index: usize) -> u16 {
    u16::from_le_bytes([oam[index], oam[index + 1]])
}

fn write_oam_word(oam: &mut [u8; 0xA0], index: usize, value: u16) {
    let [lo, hi] = value.to_le_bytes();
    oam[index] = lo;
    oam[index + 1] = hi;
}

/// What the CPU calls each M-cycle. Each `*_m` method runs the full invariant.
pub trait CpuBus {
    /// Read one byte over one M-cycle (tick-THEN-sample).
    fn read_m(&mut self, addr: u16) -> u8;
    /// Write one byte over one M-cycle.
    fn write_m(&mut self, addr: u16, value: u8);
    /// Burn one M-cycle with no memory access (internal CPU work).
    fn idle_m(&mut self);
    fn oam_bug_idu_m(&mut self, _addr: u16) {
        self.idle_m();
    }
    fn oam_bug_idu_glitch(&mut self, _addr: u16) {}

    /// Currently visible `IE & IF` (low 5 bits). Polled at instruction boundary.
    fn irq_pending_mask(&self) -> u8;
    /// The IE register (for the interrupt-dispatch IE re-check).
    fn ie(&self) -> u8;
    /// Clear one IF bit (during interrupt dispatch).
    fn clear_if_bit(&mut self, bit: u8);
    /// True if a CGB double-speed switch is armed (KEY1 $FF4D bit 0 set). STOP
    /// consults this: when armed, STOP performs the switch and resumes instead
    /// of halting the CPU.
    fn speed_switch_armed(&self) -> bool;
    /// Settle the CGB double-speed switch after a STOP: toggle the speed and
    /// clear the armed bit.
    fn finish_speed_switch(&mut self);
    /// Instruction-boundary hook before the CPU polls `irq_pending_mask`.
    /// Interrupt requests are already visible in IF; this normalizes IF's
    /// unused high bits for buses that model them.
    fn boundary(&mut self);
}

/// A single CPU bus access, used to drive the per-M-cycle timing in one place.
/// The CPU describes WHAT it wants (fetch/read/write/idle); the bus owns WHEN
/// the access lands within the M-cycle's 4 T-cycles. Some register classes land
/// before end-of-M when hardware-observable edges require sub-M placement.
enum CpuAccess {
    /// Burn one M-cycle with no memory access.
    Idle,
    /// Memory read (opcode fetch or operand/data read -- identical timing;
    /// sampled at end-of-M after all 4 T-cycles).
    Read {
        addr: u16,
    },
    /// Memory write.
    Write {
        addr: u16,
        value: u8,
    },
    OamBugIdu {
        addr: u16,
    },
}

#[derive(Clone, Copy)]
enum OamBugAccess {
    Read,
    Write,
}

fn is_ppu_visible_write(addr: u16) -> bool {
    matches!(
        addr,
        0xFF40..=0xFF43 // LCDC, STAT, SCY, SCX
            | 0xFF45 // LYC
            | 0xFF47..=0xFF4B // BGP, OBP0, OBP1, WY, WX
            | 0xFF68..=0xFF6B // CGB BCPS/BCPD/OCPS/OCPD
    )
}

/// Bus-observable fields the CPU merges into a flight record after an M-cycle.
/// The CPU supplies PC/opcode/regs; the bus supplies these. Side-effect-free.
#[cfg(feature = "flight-recorder")]
#[derive(Clone, Copy, Debug, Default)]
pub struct BusFlightFields {
    pub mcycle: u64,
    pub ie: u8,
    pub if_: u8,
    pub ly: u8,
    pub ppu_mode: u8,
    pub speed: u8,
    pub dma: u8,
}

/// OAM DMA state for the per-M-cycle beat.
#[derive(Default)]
struct OamDma {
    active: bool,
    source_hi: u8,
    index: u8,
    /// A freshly-written $FF46 source, armed but not yet started. The transfer
    /// begins after a 1 M-cycle startup delay; a restart keeps the old DMA
    /// running through the setup cycle, so we cannot clear `active` immediately.
    pending_source_hi: Option<u8>,
    start_delay_m: u8,
    /// The byte transferred this M-cycle (what a conflicting CPU read sees).
    conflict_byte_this_m: Option<u8>,
    /// Whether DMA owns the bus for the CPU this M-cycle.
    active_for_cpu_this_m: bool,
}

/// CGB VRAM DMA (HDMA1-5, $FF51-$FF55): copies ROM/RAM -> VRAM either all at
/// once (General-Purpose DMA, CPU halted) or $10 bytes per HBlank (HBlank DMA).
#[derive(Default)]
struct Hdma {
    /// Source address (HDMA1/2), low 4 bits forced to 0.
    source: u16,
    /// Destination offset within VRAM (HDMA3/4), masked to $0000-$1FF0.
    dest: u16,
    /// Remaining $10-byte blocks to transfer (0 = idle).
    remaining: u8,
    /// True while an HBlank-mode transfer is armed and in progress.
    hblank_active: bool,
    /// Previous PPU mode, to detect the rising edge into HBlank (mode 0).
    prev_ppu_mode: u8,
}

/// The whole non-CPU machine. Owns all memory + tickable peripherals. The CPU
/// borrows this `&mut` for one `*_m` call at a time; no peripheral holds a
/// reference back, so the borrow checker stays happy.
pub struct Bus {
    // Memory regions (flat placeholders; banking lands in MBC/CGB waves).
    pub cart: Cartridge,
    /// WRAM: 8 banks of 4 KiB. DMG uses banks 0-1 flat; CGB fixes bank 0 at
    /// C000-CFFF and selects banks 1-7 at D000-DFFF via SVBK ($FF70).
    pub wram: [[u8; 0x1000]; 8],
    /// Active high WRAM bank (SVBK $FF70 bits 0-2; 0 remaps to 1). DMG: always 1.
    pub svbk: u8,
    pub hram: [u8; 0x7F],
    /// VRAM: 2 banks of 8 KiB. DMG uses only bank 0; CGB selects via VBK ($FF4F).
    pub vram: [[u8; 0x2000]; 2],
    /// Active VRAM bank (VBK $FF4F bit 0). Always 0 in DMG mode.
    pub vbk: u8,
    /// CGB background palette RAM: 8 palettes x 4 colors x 2 bytes = 64 bytes,
    /// RGB555 little-endian. Addressed via BCPS ($FF68), data via BCPD ($FF69).
    pub bg_palette_ram: [u8; 64],
    /// CGB object palette RAM: same layout, via OCPS ($FF6A) / OCPD ($FF6B).
    pub obj_palette_ram: [u8; 64],
    /// BCPS ($FF68): bit 7 = auto-increment, bits 5-0 = bg_palette_ram index.
    bcps: u8,
    /// OCPS ($FF6A): bit 7 = auto-increment, bits 5-0 = obj_palette_ram index.
    ocps: u8,
    pub oam: [u8; 0xA0],
    pub io: [u8; 0x80],

    pub interrupts: Interrupts,
    pub joypad: Joypad,
    pub serial: Serial,
    pub timer: Timer,
    pub ppu: Ppu,
    pub apu: Apu,
    pub cgb: CgbState,
    dma: OamDma,
    hdma: Hdma,

    /// Test/diagnostic hook: total `tick_cpu_t` calls so far this run.
    t_tick_count: u64,
    /// Snapshot of `t_tick_count` taken at the last latched access (for tests
    /// asserting "4 ticks happened before the sample").
    ticks_at_last_sample: u64,

    /// Captured serial output. When the CPU writes SC ($FF02) with bit 7 set, the
    /// SB ($FF01) byte is appended here. This is the blargg test-result channel.
    pub serial_out: Vec<u8>,
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            cart: Cartridge::default(),
            wram: [[0; 0x1000]; 8],
            svbk: 1,
            hram: [0; 0x7F],
            vram: [[0; 0x2000]; 2],
            vbk: 0,
            bg_palette_ram: [0xFF; 64],
            obj_palette_ram: [0xFF; 64],
            bcps: 0,
            ocps: 0,
            oam: [0; 0xA0],
            io: [0; 0x80],
            interrupts: Interrupts::default(),
            joypad: Joypad::default(),
            serial: Serial::default(),
            timer: Timer::power_on(),
            ppu: Ppu::default(),
            apu: Apu::default(),
            cgb: CgbState::default(),
            dma: OamDma::default(),
            hdma: Hdma::default(),
            t_tick_count: 0,
            ticks_at_last_sample: 0,
            serial_out: Vec::new(),
        }
    }
}

impl Bus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a joypad button's pressed state. A fresh press of a selected-line
    /// button raises the joypad interrupt (IF bit 4).
    pub fn set_button(&mut self, button: Button, pressed: bool) {
        if self.joypad.set_button(button, pressed) {
            self.interrupts.request(4); // joypad interrupt
        }
    }

    /// Instruction-boundary hook (before the CPU polls `irq_pending_mask`).
    /// IF requests are immediate; this normalizes IF's unused high bits.
    pub fn boundary(&mut self) {
        self.interrupts.settle_boundary();
    }

    /// Number of T-cycle ticks recorded just before the most recent latched
    /// access. Tests assert this equals the count after the 4-tick burst.
    pub fn ticks_at_last_sample(&self) -> u64 {
        self.ticks_at_last_sample
    }

    pub fn total_ticks(&self) -> u64 {
        self.t_tick_count
    }

    /// Build a flight record from the current (post-M-cycle) bus state. The CPU
    /// fills in PC/opcode/regs; the bus contributes its observable fields. This
    /// is side-effect-free (no tick) and is the seam N2's `step_m` uses to feed
    /// `diag_record_mcycle!` AFTER a bus M-cycle completes.
    #[cfg(feature = "flight-recorder")]
    pub fn flight_fields(&self) -> BusFlightFields {
        BusFlightFields {
            // NOTE: mcycle numbering is 1-based after the first completed M-cycle
            // (t_tick_count is 4 by then). N2/trace alignment must account for this.
            mcycle: self.t_tick_count / 4,
            ie: self.interrupts.ie,
            if_: self.interrupts.if_,
            ly: self.ppu.ly,
            ppu_mode: self.ppu.mode,
            speed: self.cgb.double_speed as u8,
            dma: self.dma.active_for_cpu_this_m as u8,
        }
    }

    // ---- the invariant -----------------------------------------------------

    /// Drive one CPU M-cycle for a typed access, centralizing the per-M-cycle
    /// timing in ONE place.
    ///
    /// Writes land at the earliest timing required by the register class: TAC at
    /// T2 for timer-edge semantics, PPU-visible IO at T3 so the final dot of the
    /// M-cycle observes it, and all other writes at end-of-M. Reads/idle sample
    /// after all 4 T (end-of-M; Oracle ses_16262cca4).
    ///
    /// Ordering per M-cycle: OAM-DMA beat (T0) -> N pre-ticks -> access ->
    /// (4-N) post-ticks -> HBlank-HDMA step.
    fn run_cpu_access(&mut self, access: CpuAccess) -> u8 {
        // (1) OAM DMA beat happens once at the start of the M-cycle.
        self.oam_dma_beat();

        let result = match access {
            CpuAccess::Idle => {
                self.tick_t_times(4);
                0xFF
            }
            CpuAccess::Read { addr } => {
                // Reads sample after all 4 T (end-of-M): the correct DMG
                // placement that mem_timing / mem_timing-2 and the timer-backed
                // measurement loops calibrate against. instr_timing was fixed by
                // immediate IF latching, NOT read repositioning (Oracle
                // ses_16262cca4).
                self.tick_t_times(4);
                self.cpu_read_latched(addr)
            }
            CpuAccess::Write { addr, value } => {
                // TAC ($FF07) is the edge-producing timer register: a write can
                // create a falling edge on the selected timer input and start the
                // overflow->reload->IRQ chain. It commits mid-M (N=2: tick 2 ->
                // commit+eval -> tick 2) so the reload/IRQ lands at the right
                // sub-cycle (Oracle ses_164cf0305). rapid_toggle's `ldh (TAC),a`
                // is immediately followed by `dec bc`; end-of-M gives BC=$FFD8,
                // N=0 gives $FFDA, and the N=2 midpoint hits the correct $FFD9.
                //
                // PPU-visible IO commits at T3 so the final dot of the M-cycle
                // sees mid-mode-3 register changes. Every other write --
                // including DIV ($FF04), TIMA/TMA ($FF05/$FF06), VRAM/OAM, DMA,
                // HDMA, WRAM, HRAM, and VBK ($FF4F) -- stays end-of-M to keep the
                // memory/timer timing contracts intact.
                if addr == 0xFF07 {
                    self.tick_t_times(2);
                    self.cpu_write_latched(addr, value);
                    self.tick_t_times(2);
                } else if is_ppu_visible_write(addr) {
                    self.tick_t_times(3);
                    self.cpu_write_latched(addr, value);
                    self.tick_t_times(1);
                } else {
                    self.tick_t_times(4);
                    self.cpu_write_latched(addr, value);
                }
                0xFF
            }
            CpuAccess::OamBugIdu { addr } => {
                self.tick_t_times(4);
                self.ticks_at_last_sample = self.t_tick_count;
                self.corrupt_oam_for_bug(addr, OamBugAccess::Write);
                0xFF
            }
        };

        // (3) HBlank VRAM DMA: copy one $10 block on each fresh HBlank entry.
        self.hdma_hblank_step();
        result
    }

    /// Tick `n` CPU T-cycles (timer every T; PPU/APU per the speed rule).
    fn tick_t_times(&mut self, n: u32) {
        for _ in 0..n {
            self.tick_cpu_t();
        }
    }

    fn tick_cpu_t(&mut self) {
        self.t_tick_count += 1;

        let div_apu_before = self.div_apu_bit_high();
        self.timer.tick_t(&mut self.interrupts);
        self.serial
            .tick_t(self.timer.div_counter(), &mut self.interrupts);
        self.clock_div_apu_if_fell(div_apu_before);

        if self.cgb.double_speed {
            // PPU/APU advance every 2nd T (twice per M-cycle). PROVISIONAL: with
            // t_phase starting false, the toggle-to-true fires on T1/T3 (not
            // T2/T4). The T-phase PARITY is unverified — calibrate against CGB
            // dot-accurate tests before the CGB timing wave.
            self.cgb.t_phase = !self.cgb.t_phase;
            if self.cgb.t_phase {
                let palettes = DmgPalettes {
                    bgp: self.io[0x47],
                    obp0: self.io[0x48],
                    obp1: self.io[0x49],
                };
                let cgb = CgbRenderState {
                    enabled: self.cgb.cgb_mode,
                    bg_palette_ram: &self.bg_palette_ram,
                    obj_palette_ram: &self.obj_palette_ram,
                };
                self.ppu
                    .tick_dot(&mut self.interrupts, &self.vram, &self.oam, palettes, cgb);
                self.apu.tick_t();
            }
        } else {
            let palettes = DmgPalettes {
                bgp: self.io[0x47],
                obp0: self.io[0x48],
                obp1: self.io[0x49],
            };
            let cgb = CgbRenderState {
                enabled: self.cgb.cgb_mode,
                bg_palette_ram: &self.bg_palette_ram,
                obj_palette_ram: &self.obj_palette_ram,
            };
            self.ppu
                .tick_dot(&mut self.interrupts, &self.vram, &self.oam, palettes, cgb);
            self.apu.tick_t();
        }
    }

    fn div_apu_bit_high(&self) -> bool {
        // DIV-APU clocks on the falling edge of bit 4 (bit 5 in CGB double-
        // speed) of the VISIBLE DIV byte ($FF04 = div >> 8), i.e. bit 12 (13) of
        // the 16-bit internal counter -> a 512 Hz frame-sequencer step.
        let mask = if self.cgb.double_speed {
            0x2000
        } else {
            0x1000
        };
        self.timer.div_counter() & mask != 0
    }

    fn clock_div_apu_if_fell(&mut self, before: bool) {
        if before && !self.div_apu_bit_high() {
            self.apu.tick_div_apu();
        }
    }

    fn oam_dma_beat(&mut self) {
        self.dma.conflict_byte_this_m = None;
        self.dma.active_for_cpu_this_m = false;

        // Pending DMA from a fresh $FF46 write (Oracle ses_164ac8274): numbering
        // the write M-cycle as M=0, M=1 is a setup cycle (no transfer; a restart
        // lets the OLD DMA keep running), and the new transfer begins on M=2.
        if self.dma.pending_source_hi.is_some() {
            if self.dma.start_delay_m > 0 {
                self.dma.start_delay_m -= 1;
                // Do NOT return: an older DMA must still transfer during M=1.
            } else {
                self.dma.source_hi = self.dma.pending_source_hi.take().unwrap();
                self.dma.active = true;
                self.dma.index = 0;
            }
        }

        if !self.dma.active {
            return;
        }

        let src = ((self.dma.source_hi as u16) << 8) | self.dma.index as u16;
        let byte = self.read_dma_source(src);
        self.dma.conflict_byte_this_m = Some(byte);
        self.dma.active_for_cpu_this_m = true;

        let oam_index = self.dma.index as usize;
        if oam_index < self.oam.len() {
            self.oam[oam_index] = byte;
        }
        self.dma.index = self.dma.index.wrapping_add(1);
        if self.dma.index as usize >= self.oam.len() {
            self.dma.active = false;
        }
    }

    /// Schedule an OAM DMA from `0xXX00` (value = `XX`). The transfer starts
    /// after a 1 M-cycle setup delay (byte 0 on relative M=2). On a restart, the
    /// currently-active DMA keeps running through the setup cycle, so `active` is
    /// NOT cleared here -- the pending source replaces it on M=2.
    pub fn start_oam_dma(&mut self, value: u8) {
        self.dma.pending_source_hi = Some(value);
        self.dma.start_delay_m = 1;
        // $FF46 reads back the last value written (mooneye oam_dma/reg_read).
        self.io[0x46] = value;
    }

    /// Side-effect-free read of the DMA source (no ticking).
    fn read_dma_source(&self, addr: u16) -> u8 {
        // The OAM-DMA source address decoder folds the whole $E0-$FF source
        // page range onto WRAM (echo), so a DMA from $E0xx-$FFxx reads WRAM and
        // NOT OAM/IO. Pan Docs documents only $00-$DF, but mooneye oam_dma/
        // sources-GS pins the $E0/$FE/$FF behaviour: they read WRAM echo.
        if addr >= 0xE000 {
            // Fold $E000-$FFFF onto the WRAM echo. wram_index handles
            // $E000-$FDFF; $FE00-$FFFF continues the same echo (subtract the
            // $2000 echo offset) so a DMA source page of $FE reads WRAM $DE00.
            let echoed = addr - 0x2000;
            let (bank, off) = self.wram_index(echoed);
            return self.wram[bank][off];
        }
        self.peek(addr)
    }

    /// Whether a CPU access to `addr` targets the SAME bus the OAM DMA is
    /// currently driving. The DMA drives the bus its source page lives on:
    /// the video bus ($8000-$9FFF) for sources $80-$9F, else the external bus
    /// (ROM/SRAM/WRAM, $0000-$7FFF + $A000-$FDFF). Only same-bus accesses see the
    /// in-flight conflict byte; the other bus stays usable (Pan Docs OAM DMA bus
    /// conflicts). The IO/HRAM region ($FF00-$FFFF) is on neither memory bus, so
    /// the CPU can always access it during DMA (mooneye oam_dma/reg_read reads
    /// $FF46 while a DMA is in progress).
    fn dma_conflicts_with(&self, addr: u16) -> bool {
        // IO + HRAM are never on a DMA-driven memory bus.
        if matches!(addr, 0xFF00..=0xFFFF) {
            return false;
        }
        let dma_on_video_bus = matches!(self.dma.source_hi, 0x80..=0x9F);
        let addr_on_video_bus = matches!(addr, 0x8000..=0x9FFF);
        dma_on_video_bus == addr_on_video_bus
    }

    /// Handle a write to HDMA5 ($FF55): start a General-Purpose or HBlank VRAM
    /// DMA, or terminate an in-progress HBlank transfer.
    fn write_hdma5(&mut self, value: u8) {
        let blocks = (value & 0x7F) + 1;
        if value & 0x80 == 0 {
            // Bit 7 = 0. If an HBlank transfer is active, this terminates it.
            // Otherwise it is a General-Purpose DMA: copy everything at once.
            if self.hdma.hblank_active {
                self.hdma.hblank_active = false;
                // remaining stays as-is so reads report the leftover length.
                return;
            }
            self.hdma.remaining = blocks;
            self.hdma_run_gdma();
        } else {
            // Bit 7 = 1: arm an HBlank DMA. Blocks transfer $10 at a time, one
            // per HBlank, starting on the next HBlank entry.
            self.hdma.remaining = blocks;
            self.hdma.hblank_active = true;
        }
    }

    /// General-Purpose DMA: copy all remaining blocks at once. The CPU is halted
    /// for the duration; we model that by ticking the bus 8 M-cycles per block
    /// (the hardware transfer time) so peripherals advance correctly.
    fn hdma_run_gdma(&mut self) {
        while self.hdma.remaining > 0 {
            self.hdma_copy_block();
            // 8 M-cycles per $10-byte block (Transfer Timings, Pan Docs).
            for _ in 0..8 {
                for _ in 0..4 {
                    self.tick_cpu_t();
                }
            }
        }
    }

    /// Copy a single $10-byte block from source -> VRAM dest, advancing both
    /// pointers and decrementing the remaining-block count.
    fn hdma_copy_block(&mut self) {
        for _ in 0..0x10 {
            let byte = self.peek(self.hdma.source);
            // Dest is always within VRAM ($8000-$9FFF); wrap within the 8 KiB
            // bank. The running low bits advance per byte (unlike the start
            // address, whose low 4 bits were forced to 0 at register-write time).
            let off = (self.hdma.dest & 0x1FFF) as usize;
            self.vram[self.vbk as usize][off] = byte;
            self.hdma.source = self.hdma.source.wrapping_add(1);
            self.hdma.dest = self.hdma.dest.wrapping_add(1);
        }
        self.hdma.remaining = self.hdma.remaining.saturating_sub(1);
    }

    /// Drive HBlank DMA: copy one $10 block on the rising edge into PPU mode 0
    /// (HBlank) while a transfer is armed. Called once per M-cycle after the
    /// PPU has advanced.
    fn hdma_hblank_step(&mut self) {
        let mode = self.ppu.mode;
        let entered_hblank =
            mode == ppu::mode::HBLANK && self.hdma.prev_ppu_mode != ppu::mode::HBLANK;
        self.hdma.prev_ppu_mode = mode;

        if self.hdma.hblank_active
            && self.hdma.remaining > 0
            && entered_hblank
            && self.ppu.lcd_enabled()
        {
            self.hdma_copy_block();
            if self.hdma.remaining == 0 {
                self.hdma.hblank_active = false;
            }
        }
    }

    /// Map a WRAM address (0xC000-0xDFFF or its 0xE000-0xFDFF echo) to a
    /// (bank, offset) pair. C000-CFFF (echo E000-EFFF) is always bank 0; D000-
    /// DFFF (echo F000-FDFF) selects `svbk` (banks 1-7). DMG keeps svbk=1 so the
    /// low 8 KiB behave as a flat bank0+bank1 pair, exactly as before.
    #[inline]
    fn wram_index(&self, addr: u16) -> (usize, usize) {
        // Fold the echo region (E000-FDFF) down onto C000-DDFF.
        let a = if addr >= 0xE000 { addr - 0x2000 } else { addr };
        if a < 0xD000 {
            (0, (a - 0xC000) as usize)
        } else {
            (self.svbk as usize, (a - 0xD000) as usize)
        }
    }

    // ---- latched access (sample at END of M) -------------------------------

    fn cpu_read_latched(&mut self, addr: u16) -> u8 {
        self.ticks_at_last_sample = self.t_tick_count;
        self.corrupt_oam_for_bug(addr, OamBugAccess::Read);

        // OAM DMA bus conflict (Pan Docs: the CPU can still access the OTHER
        // bus during DMA). The DMA drives the bus its SOURCE is on -- the
        // external bus (ROM/SRAM/WRAM, $0000-$7FFF + $A000-$FDFF) or the video
        // bus ($8000-$9FFF). A CPU access conflicts only if it targets the SAME
        // bus as the DMA source; OAM is always blocked; HRAM is always free.
        if self.dma.active_for_cpu_this_m {
            if matches!(addr, 0xFE00..=0xFE9F) {
                return 0xFF; // OAM always blocked during DMA
            }
            if !matches!(addr, 0xFF80..=0xFFFE) && self.dma_conflicts_with(addr) {
                if let Some(b) = self.dma.conflict_byte_this_m {
                    return b;
                }
            }
        }
        // PPU mode blocking: VRAM is unreadable in mode 3, OAM in modes 2-3.
        // (Checked on the latched CPU path only, so DMA-source reads and
        // side-effect-free diagnostics via `peek` are unaffected.)
        match addr {
            0x8000..=0x9FFF if self.ppu.vram_blocked() => return 0xFF,
            0xFE00..=0xFE9F if self.ppu.oam_read_blocked() => return 0xFF,
            _ => {}
        }
        self.peek(addr)
    }

    fn cpu_write_latched(&mut self, addr: u16, value: u8) {
        self.ticks_at_last_sample = self.t_tick_count;
        self.corrupt_oam_for_bug(addr, OamBugAccess::Write);

        if self.dma.active_for_cpu_this_m {
            // During OAM DMA the CPU can only write to a bus the DMA is NOT
            // driving (or HRAM). A write to the same bus as the DMA source (or to
            // OAM) is dropped; writes to the other bus go through.
            let blocked = matches!(addr, 0xFE00..=0xFE9F)
                || (!matches!(addr, 0xFF80..=0xFFFE) && self.dma_conflicts_with(addr));
            if blocked {
                return;
            }
        }
        // PPU mode blocking: writes to VRAM (mode 3) / OAM (modes 2-3) are
        // dropped while the PPU owns them.
        match addr {
            0x8000..=0x9FFF if self.ppu.vram_write_blocked() => return,
            0xFE00..=0xFE9F if self.ppu.oam_write_blocked() => return,
            _ => {}
        }
        self.poke(addr, value);
    }

    fn corrupt_oam_for_bug(&mut self, addr: u16, access: OamBugAccess) {
        if self.cgb.cgb_mode || !matches!(addr, 0xFE00..=0xFEFF) {
            return;
        }
        let Some(row) = self.ppu.oam_bug_scan_row() else {
            return;
        };
        self.apply_oam_bug_corruption(row, access);
    }

    fn apply_oam_bug_corruption(&mut self, row: usize, access: OamBugAccess) {
        if row == 0 || row >= 20 {
            return;
        }

        let base = row * 8;
        let prev = base - 8;
        let a = read_oam_word(&self.oam, base);
        let b = read_oam_word(&self.oam, prev);
        let c = read_oam_word(&self.oam, prev + 4);
        let word0 = match access {
            OamBugAccess::Read => b | (a & c),
            OamBugAccess::Write => ((a ^ c) & (b ^ c)) ^ c,
        };
        write_oam_word(&mut self.oam, base, word0);
        for word in 1..4 {
            let copied = read_oam_word(&self.oam, prev + word * 2);
            write_oam_word(&mut self.oam, base + word * 2, copied);
        }
    }

    /// Side-effect-free read (no tick). The decode is a flat placeholder; real
    /// banking/IO-side-effects land in later waves.
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cart.read(addr),
            0x8000..=0x9FFF => self.vram[self.vbk as usize][(addr - 0x8000) as usize],
            0xC000..=0xFDFF => {
                let (bank, off) = self.wram_index(addr);
                self.wram[bank][off]
            }
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFF01 => self.serial.read_sb(),
            0xFF02 => self.serial.read_sc(self.cgb.cgb_mode),
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF0F => self.interrupts.if_ | 0xE0, // IF lives in interrupts, not io[]
            0xFF10..=0xFF26 | 0xFF30..=0xFF3F => self.apu.read_for_model(addr, self.cgb.cgb_mode),
            0xFF40 => self.ppu.read_lcdc(),
            0xFF41 => self.ppu.read_stat(),
            0xFF42 => self.ppu.read_scy(),
            0xFF43 => self.ppu.read_scx(),
            0xFF44 => self.ppu.read_ly(),
            0xFF45 => self.ppu.read_lyc(),
            0xFF4A => self.ppu.read_wy(),
            0xFF4B => self.ppu.read_wx(),
            0xFF4F if self.cgb.cgb_mode => self.vbk | 0xFE, // VBK: only bit 0 valid
            0xFF4D if self.cgb.cgb_mode => {
                // KEY1 (CGB only): bit 7 = current speed (1 = double), bit 0 =
                // armed switch, remaining bits read as 1.
                let speed = if self.cgb.double_speed { 0x80 } else { 0x00 };
                speed | (self.io[0x4D] & 0x01) | 0x7E
            }
            0xFF4D => 0xFF, // KEY1 in DMG mode: open-bus (no speed switch)
            0xFF4F => 0xFF, // VBK in DMG mode: open-bus (no VRAM banking)
            0xFF70 if self.cgb.cgb_mode => self.svbk | 0xF8, // SVBK: 3 bits valid
            0xFF70 => 0xFF, // SVBK in DMG mode: open-bus (no WRAM banking)
            0xFF6C if self.cgb.cgb_mode => (self.io[0x6C] & 0x01) | 0xFE, // OPRI bit0
            0xFF6C => 0xFF, // OPRI in DMG mode: open-bus
            0xFF55 if self.cgb.cgb_mode => {
                // HDMA5: bit 7 = 0 while an HBlank transfer is active, 1 when
                // idle / completed / manually stopped. bits 6-0 = remaining
                // length in $10-blocks, minus 1; a completed transfer reads 0xFF.
                if self.hdma.remaining == 0 {
                    0xFF
                } else if self.hdma.hblank_active {
                    (self.hdma.remaining - 1) & 0x7F // active: bit 7 clear
                } else {
                    0x80 | ((self.hdma.remaining - 1) & 0x7F) // stopped: bit 7 set
                }
            }
            0xFF51..=0xFF55 => 0xFF, // HDMA in DMG mode / write-only HDMA1-4
            0xFF68 if self.cgb.cgb_mode => self.bcps | 0x40, // BCPS: bit6 reads 1
            0xFF6A if self.cgb.cgb_mode => self.ocps | 0x40, // OCPS: bit6 reads 1
            0xFF69 if self.cgb.cgb_mode => {
                // BCPD: palette RAM is inaccessible during mode 3 (reads 0xFF).
                if self.ppu.cgb_palette_blocked() {
                    0xFF
                } else {
                    self.bg_palette_ram[(self.bcps & 0x3F) as usize]
                }
            }
            0xFF6B if self.cgb.cgb_mode => {
                if self.ppu.cgb_palette_blocked() {
                    0xFF
                } else {
                    self.obj_palette_ram[(self.ocps & 0x3F) as usize]
                }
            }
            0xFF68..=0xFF6B => 0xFF, // palette regs in DMG mode: open-bus
            0xFF00..=0xFF7F => self.read_io_masked(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.interrupts.ie,
            0xA000..=0xBFFF => self.cart.read_ram(addr),
            _ => 0xFF, // unmapped
        }
    }

    /// Read a generic IO register ($FF00-$FF7F not special-cased above), applying
    /// the per-register "unused bits read as 1" masks and returning 0xFF for
    /// unmapped addresses (mooneye unused_hwio). `idx` = addr - 0xFF00.
    fn read_io_masked(&self, addr: u16) -> u8 {
        let idx = (addr - 0xFF00) as usize;
        let raw = self.io[idx];
        // (or_mask): bits forced to 1 on read. 0x00 = all bits readable.
        let or_mask: u8 = match addr {
            // P1/JOYP ($FF00): bits 7-6 always read 1; bits 5-4 are the writable
            // line-select; bits 3-0 report the selected line's buttons, active
            // LOW (1 = pressed). The Joypad synthesizes the full register from
            // the logical button state + the selected line.
            0xFF00 => return self.joypad.read_p1(),
            0xFF42 | 0xFF43 | 0xFF4A | 0xFF4B => 0x00, // SCY/SCX/WY/WX: full (but PPU-owned)
            0xFF47..=0xFF49 => 0x00,                   // BGP/OBP0/OBP1: full
            // CGB-only registers, gated on cgb_mode:
            0xFF68 | 0xFF6A if self.cgb.cgb_mode => 0x40, // BCPS/OCPS: bit 6 reads 1 (unused_hwio-C)
            0xFF72 | 0xFF73 if self.cgb.cgb_mode => 0x00, // fully read/write
            0xFF75 if self.cgb.cgb_mode => 0x8F,          // only bits 6-4 writable
            0xFF76 | 0xFF77 if self.cgb.cgb_mode => return 0x00, // PCM12/34: read-only 0
            _ => return self.unmapped_io_or_raw(addr, raw),
        };
        raw | or_mask
    }

    /// For addresses not in the masked table: registers backed by `io[]` that we
    /// model return their raw byte; truly unmapped IO reads 0xFF.
    fn unmapped_io_or_raw(&self, addr: u16, raw: u8) -> u8 {
        // Addresses we DON'T implement read as open-bus 0xFF (mooneye
        // unused_hwio test_unmapped). Everything FF00-FF7F that isn't a real
        // register: FF03, FF08-FF0E, FF15, FF1F, FF27-FF2E, FF4C-FF4E, FF50-FF67
        // (minus 51-55 HDMA), FF69, FF6B, FF6D-FF6F, FF71, FF74, FF78-FF7F.
        let mapped = matches!(addr,
            0xFF00..=0xFF02 | 0xFF04..=0xFF07 | 0xFF0F
            | 0xFF10..=0xFF14 | 0xFF16..=0xFF1E | 0xFF20..=0xFF26 | 0xFF30..=0xFF3F
            | 0xFF40..=0xFF4B
        );
        // CGB-only mapped registers (when in CGB mode).
        let cgb_mapped = self.cgb.cgb_mode
            && matches!(addr, 0xFF4D | 0xFF4F | 0xFF51..=0xFF55 | 0xFF56
                | 0xFF68 | 0xFF6A | 0xFF6C | 0xFF70 | 0xFF72 | 0xFF73 | 0xFF75..=0xFF77);
        if mapped || cgb_mapped {
            raw
        } else {
            0xFF
        }
    }

    /// Side-effect-free write (no tick). Flat placeholder.
    pub fn poke(&mut self, addr: u16, value: u8) {
        match addr {
            0x8000..=0x9FFF => self.vram[self.vbk as usize][(addr - 0x8000) as usize] = value,
            0xC000..=0xFDFF => {
                let (bank, off) = self.wram_index(addr);
                self.wram[bank][off] = value;
            }
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = value,
            0xFF00 => self.joypad.write_p1(value), // P1/JOYP: only bits 5-4 (line select) writable
            0xFF01 => self.serial.write_sb(value),
            0xFF02 => {
                if value & 0x80 != 0 {
                    self.serial_out.push(self.serial.read_sb());
                }
                self.serial
                    .write_sc(value, self.cgb.cgb_mode, self.timer.div_counter());
            }
            0xFF04 => {
                let div_apu_before = self.div_apu_bit_high();
                self.timer.write(addr, value, &mut self.interrupts);
                self.serial
                    .div_changed(self.timer.div_counter(), &mut self.interrupts);
                self.clock_div_apu_if_fell(div_apu_before);
            }
            0xFF05..=0xFF07 => self.timer.write(addr, value, &mut self.interrupts),
            0xFF46 => self.start_oam_dma(value),
            0xFF0F => self.interrupts.if_ = value | 0xE0, // IF lives in interrupts
            0xFF10..=0xFF26 | 0xFF30..=0xFF3F => self.apu.write(addr, value, self.cgb.cgb_mode),
            0xFF40 => self.ppu.write_lcdc(value, &mut self.interrupts),
            0xFF41 => self.ppu.write_stat(value, &mut self.interrupts),
            0xFF42 => self.ppu.write_scy(value),
            0xFF43 => self.ppu.write_scx(value),
            0xFF44 => {} // LY is read-only
            0xFF45 => self.ppu.write_lyc(value, &mut self.interrupts),
            0xFF4A => self.ppu.write_wy(value),
            0xFF4B => self.ppu.write_wx(value),
            0xFF4F => {
                // VBK (CGB only): bit 0 selects the active 8 KiB VRAM bank.
                if self.cgb.cgb_mode {
                    self.vbk = value & 0x01;
                }
            }
            0xFF70 => {
                // SVBK (CGB only): bits 0-2 select the D000-DFFF WRAM bank;
                // a written 0 maps to bank 1.
                if self.cgb.cgb_mode {
                    let b = value & 0x07;
                    self.svbk = if b == 0 { 1 } else { b };
                }
            }
            0xFF51 => {
                // HDMA1: VRAM DMA source high byte (CGB only, write-only).
                if self.cgb.cgb_mode {
                    self.hdma.source = (self.hdma.source & 0x00FF) | ((value as u16) << 8);
                }
            }
            0xFF52 => {
                // HDMA2: source low byte; low 4 bits are forced to 0.
                if self.cgb.cgb_mode {
                    self.hdma.source = (self.hdma.source & 0xFF00) | ((value as u16) & 0xF0);
                }
            }
            0xFF53 => {
                // HDMA3: dest high byte; only bits 12-8 matter (dest is VRAM).
                if self.cgb.cgb_mode {
                    self.hdma.dest = (self.hdma.dest & 0x00FF) | (((value as u16) & 0x1F) << 8);
                }
            }
            0xFF54 => {
                // HDMA4: dest low byte; low 4 bits forced to 0.
                if self.cgb.cgb_mode {
                    self.hdma.dest = (self.hdma.dest & 0xFF00) | ((value as u16) & 0xF0);
                }
            }
            0xFF55 => {
                // HDMA5: start/stop a VRAM DMA transfer (CGB only).
                if self.cgb.cgb_mode {
                    self.write_hdma5(value);
                }
            }
            0xFF6C => {
                // OPRI (CGB only): object priority mode flag (bit 0).
                if self.cgb.cgb_mode {
                    self.io[0x6C] = value & 0x01;
                }
            }
            0xFF68 => {
                // BCPS (CGB only): bit 7 auto-increment, bits 5-0 = BG palette index.
                if self.cgb.cgb_mode {
                    self.bcps = value & 0xBF;
                }
            }
            0xFF6A => {
                // OCPS (CGB only): bit 7 auto-increment, bits 5-0 = OBJ palette index.
                if self.cgb.cgb_mode {
                    self.ocps = value & 0xBF;
                }
            }
            0xFF69 => {
                // BCPD (CGB only): write BG palette RAM at BCPS index (blocked
                // during mode 3), then auto-increment the index if BCPS bit 7 set.
                if self.cgb.cgb_mode {
                    if !self.ppu.cgb_palette_blocked() {
                        self.bg_palette_ram[(self.bcps & 0x3F) as usize] = value;
                    }
                    if self.bcps & 0x80 != 0 {
                        self.bcps = (self.bcps & 0x80) | (self.bcps.wrapping_add(1) & 0x3F);
                    }
                }
            }
            0xFF6B => {
                if self.cgb.cgb_mode {
                    if !self.ppu.cgb_palette_blocked() {
                        self.obj_palette_ram[(self.ocps & 0x3F) as usize] = value;
                    }
                    if self.ocps & 0x80 != 0 {
                        self.ocps = (self.ocps & 0x80) | (self.ocps.wrapping_add(1) & 0x3F);
                    }
                }
            }
            0xFF4D => {
                // KEY1: only the "prepare speed switch" bit (0) is writable, and
                // only in CGB mode. DMG ignores KEY1 entirely (no speed switch).
                if self.cgb.cgb_mode {
                    self.io[0x4D] = (self.io[0x4D] & !0x01) | (value & 0x01);
                }
            }
            // $FF00 (P1/JOYP) is handled by the joypad arm above; the rest of
            // the IO page falls through to the raw backing store.
            0xFF03..=0xFF7F => self.io[(addr - 0xFF00) as usize] = value,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,
            0xFFFF => self.interrupts.ie = value,
            0x0000..=0x7FFF => self.cart.write_reg(addr, value), // MBC control regs
            0xA000..=0xBFFF => self.cart.write_ram(addr, value),
            _ => {} // unmapped: ignored
        }
    }
}

impl CpuBus for Bus {
    fn read_m(&mut self, addr: u16) -> u8 {
        self.run_cpu_access(CpuAccess::Read { addr })
    }

    fn write_m(&mut self, addr: u16, value: u8) {
        self.run_cpu_access(CpuAccess::Write { addr, value });
    }

    fn idle_m(&mut self) {
        self.run_cpu_access(CpuAccess::Idle);
    }

    fn oam_bug_idu_m(&mut self, addr: u16) {
        self.run_cpu_access(CpuAccess::OamBugIdu { addr });
    }

    fn oam_bug_idu_glitch(&mut self, addr: u16) {
        self.corrupt_oam_for_bug(addr, OamBugAccess::Write);
    }

    fn irq_pending_mask(&self) -> u8 {
        self.interrupts.pending_mask()
    }

    fn ie(&self) -> u8 {
        self.interrupts.ie
    }

    fn clear_if_bit(&mut self, bit: u8) {
        self.interrupts.clear_bit(bit);
    }

    fn speed_switch_armed(&self) -> bool {
        // KEY1 ($FF4D) bit 0 = "prepare speed switch". DMG has no speed switch.
        self.cgb.cgb_mode && self.io[0x4D] & 0x01 != 0
    }

    fn finish_speed_switch(&mut self) {
        self.cgb.double_speed = !self.cgb.double_speed;
        self.cgb.t_phase = false;
        // Clear the armed bit; bit 7 now reflects the (toggled) current speed.
        self.io[0x4D] = if self.cgb.double_speed { 0x80 } else { 0x00 };
    }

    fn boundary(&mut self) {
        Bus::boundary(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_ticks_before_sample() {
        // S1: a read_m must tick the world 4 T-cycles BEFORE sampling the byte.
        let mut bus = Bus::new();
        bus.poke(0xC000, 0x42);
        let before = bus.total_ticks();
        let v = bus.read_m(0xC000);
        assert_eq!(v, 0x42, "value reads back from WRAM");
        // Exactly 4 ticks happened during this M-cycle, BEFORE the sample.
        assert_eq!(bus.ticks_at_last_sample(), before + 4);
        assert_eq!(bus.total_ticks(), before + 4);
    }

    #[test]
    fn ppu_visible_io_write_commits_at_t3() {
        let mut bus = Bus::new();
        let before = bus.total_ticks();
        bus.write_m(0xFF42, 0x37);
        assert_eq!(bus.ppu.read_scy(), 0x37);
        assert_eq!(bus.ticks_at_last_sample(), before + 3);
        assert_eq!(bus.total_ticks(), before + 4);
    }

    #[test]
    fn vbk_write_stays_end_of_m() {
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        let before = bus.total_ticks();
        bus.write_m(0xFF4F, 0x01);
        assert_eq!(bus.vbk, 0x01);
        assert_eq!(bus.ticks_at_last_sample(), before + 4);
        assert_eq!(bus.total_ticks(), before + 4);
    }

    #[test]
    fn tac_write_keeps_midpoint_commit() {
        let mut bus = Bus::new();
        let before = bus.total_ticks();
        bus.write_m(0xFF07, 0x05);
        assert_eq!(bus.peek(0xFF07) & 0x07, 0x05);
        assert_eq!(bus.ticks_at_last_sample(), before + 2);
        assert_eq!(bus.total_ticks(), before + 4);
    }

    #[test]
    fn idle_m_ticks_four_without_access() {
        let mut bus = Bus::new();
        let before = bus.total_ticks();
        bus.idle_m();
        assert_eq!(bus.total_ticks(), before + 4);
    }

    #[test]
    fn double_speed_halves_ppu_ticks() {
        // S2: in double-speed, PPU/APU advance every 2nd T (2/M); timer 4/M.
        let mut bus = Bus::new();
        bus.cgb.double_speed = true;
        let (t0, p0, a0) = (bus.total_ticks(), bus.ppu.dot_ticks, bus.apu.t_ticks);
        bus.idle_m(); // one M-cycle = 4 T
        assert_eq!(bus.total_ticks() - t0, 4, "bus ticks every T");
        assert_eq!(
            bus.ppu.dot_ticks - p0,
            2,
            "PPU ticks every 2nd T in double-speed"
        );
        assert_eq!(
            bus.apu.t_ticks - a0,
            2,
            "APU ticks every 2nd T in double-speed"
        );
    }

    #[test]
    fn normal_speed_ppu_ticks_every_t() {
        let mut bus = Bus::new();
        let p0 = bus.ppu.dot_ticks;
        bus.idle_m();
        assert_eq!(
            bus.ppu.dot_ticks - p0,
            4,
            "PPU ticks every T at normal speed"
        );
    }

    fn seed_oam_row(bus: &mut Bus, row: usize, words: [u16; 4]) {
        for (i, word) in words.into_iter().enumerate() {
            write_oam_word(&mut bus.oam, row * 8 + i * 2, word);
        }
    }

    fn oam_row_words(bus: &Bus, row: usize) -> [u16; 4] {
        [
            read_oam_word(&bus.oam, row * 8),
            read_oam_word(&bus.oam, row * 8 + 2),
            read_oam_word(&bus.oam, row * 8 + 4),
            read_oam_word(&bus.oam, row * 8 + 6),
        ]
    }

    #[test]
    fn dmg_oam_read_during_mode2_corrupts_current_row() {
        let mut bus = Bus::new();
        seed_oam_row(&mut bus, 0, [0x00F0, 0x1111, 0x0F0F, 0x3333]);
        seed_oam_row(&mut bus, 1, [0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD]);

        bus.idle_m();
        let _ = bus.read_m(0xFEA0);

        assert_eq!(
            oam_row_words(&bus, 1),
            [0x00F0 | (0xAAAA & 0x0F0F), 0x1111, 0x0F0F, 0x3333]
        );
    }

    #[test]
    fn dmg_oam_write_during_mode2_corrupts_current_row() {
        let mut bus = Bus::new();
        seed_oam_row(&mut bus, 0, [0x00F0, 0x1111, 0x0F0F, 0x3333]);
        seed_oam_row(&mut bus, 1, [0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD]);

        bus.idle_m();
        bus.write_m(0xFEA0, 0x12);

        assert_eq!(
            oam_row_words(&bus, 1),
            [
                ((0xAAAA ^ 0x0F0F) & (0x00F0 ^ 0x0F0F)) ^ 0x0F0F,
                0x1111,
                0x0F0F,
                0x3333
            ]
        );
    }

    #[test]
    fn cgb_oam_access_during_mode2_does_not_corrupt() {
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        seed_oam_row(&mut bus, 0, [0x00F0, 0x1111, 0x0F0F, 0x3333]);
        seed_oam_row(&mut bus, 1, [0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD]);

        bus.idle_m();
        let _ = bus.read_m(0xFEA0);
        bus.write_m(0xFEA0, 0x12);

        assert_eq!(oam_row_words(&bus, 1), [0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD]);
    }

    #[test]
    fn roundtrip_wram_read_write() {
        // S4: borrow-safe round-trip through the CpuBus API.
        let mut bus = Bus::new();
        bus.write_m(0xC123, 0xAB);
        assert_eq!(bus.read_m(0xC123), 0xAB);
    }

    #[test]
    fn irq_if_latch_is_immediate_but_dispatch_waits_for_boundary() {
        // S5: IF is a register latch, so an IRQ request is visible immediately;
        // CPU dispatch remains an instruction-boundary decision.
        use crate::cpu::{Cpu, CpuMode};

        let mut bus = Bus::new();
        let mut rom = vec![0u8; 0x8000];
        rom[0] = 0x3E;
        rom[1] = 0x42;
        bus.cart = Cartridge::from_rom(&rom);
        bus.interrupts.ie = 0x04; // enable timer interrupt

        let mut cpu = Cpu::new();
        cpu.ime = true;
        cpu.r.sp = 0xFFFE;

        cpu.step_m(&mut bus);
        assert_eq!(cpu.r.pc, 0x0001, "opcode fetch advanced PC");

        bus.interrupts.request(2);
        assert_eq!(
            bus.irq_pending_mask(),
            0x04,
            "IF latch is visible immediately, before boundary()"
        );

        for _ in 0..4 {
            assert!(
                !matches!(cpu.mode, CpuMode::InterruptDispatch { .. }),
                "dispatch must not start mid-instruction"
            );
            if cpu.exec_is_boundary() {
                break;
            }
            cpu.step_m(&mut bus);
        }

        assert_eq!(cpu.r.a, 0x42, "current instruction completed normally");
        assert_eq!(cpu.r.pc, 0x0002, "dispatch did not preempt LD A,d8");
        assert!(
            cpu.exec_is_boundary(),
            "dispatch waits for instruction boundary"
        );

        cpu.step_m(&mut bus);
        assert!(
            matches!(
                cpu.mode,
                CpuMode::InterruptDispatch {
                    bit: 2,
                    vector: 0x0050,
                    ..
                }
            ),
            "timer interrupt dispatch starts only from the next boundary"
        );
    }

    #[test]
    fn oam_dma_beat_records_conflict_then_blocks() {
        // S3: FF46 schedules DMA; after the start delay, each M transfers a byte,
        // records the conflict byte, and blocks OAM/general reads.
        let mut bus = Bus::new();
        // Seed source page 0xC0 (WRAM) with a known pattern.
        for i in 0..0xA0u16 {
            bus.poke(0xC000 + i, 0x10 + i as u8);
        }
        bus.poke(0xFF46, 0xC0); // start OAM DMA from 0xC000 (via poke -> start_oam_dma)

        // M=1 setup (no transfer), then M=2 transfers byte 0.
        bus.idle_m();
        // Next M transfers byte 0 -> OAM[0]; a conflicting general read sees it.
        let conflict = bus.read_m(0x4000); // ROM region, would normally read rom
        assert_eq!(conflict, 0x10, "conflicting read sees in-flight DMA byte");
        assert_eq!(bus.oam[0], 0x10, "DMA wrote OAM[0]");
        // HRAM stays accessible during DMA.
        bus.poke(0xFF80, 0x99);
        assert_eq!(bus.read_m(0xFF80), 0x99, "HRAM accessible during DMA");
    }

    #[test]
    fn oam_dma_first_byte_transfers_on_relative_m2() {
        // rubc-33y (Oracle ses_164ac8274): numbering the $FF46 write M-cycle as
        // M=0, the startup is 1 M-cycle: M=1 is setup (no transfer, OAM still
        // accessible), and DMA byte 0 transfers on M=2. Byte i transfers on M=2+i.
        let mut bus = Bus::new();
        for i in 0..0xA0u16 {
            bus.poke(0xC000 + i, 0x10 + i as u8);
        }
        // M=0: the FF46 write commits. poke models the write committing.
        bus.poke(0xFF46, 0xC0);
        // M=1: setup cycle -- no transfer yet, OAM[0] still its old value.
        bus.idle_m();
        assert_eq!(bus.oam[0], 0x00, "M=1 setup: no DMA transfer yet");
        // M=2: byte 0 transfers and the conflict byte is exposed THIS M-cycle.
        let conflict = bus.read_m(0x4000);
        assert_eq!(conflict, 0x10, "M=2: conflicting read sees DMA byte 0");
        assert_eq!(bus.oam[0], 0x10, "M=2: DMA wrote OAM[0]");
        // M=3: byte 1.
        bus.idle_m();
        assert_eq!(bus.oam[1], 0x11, "M=3: DMA wrote OAM[1]");
    }

    #[test]
    fn oam_dma_last_byte_then_inactive() {
        // Byte 159 transfers on relative M=161 (last active cycle); M=162 is
        // inactive (CPU sees real memory again).
        let mut bus = Bus::new();
        for i in 0..0xA0u16 {
            bus.poke(0xC000 + i, 0x10 + i as u8);
        }
        bus.poke(0xFF46, 0xC0); // M=0
        bus.idle_m(); // M=1 setup
                      // M=2..=M=161 transfer bytes 0..=159 (160 transfers).
        for _ in 0..159 {
            bus.idle_m();
        }
        // We are now at M=161 about to transfer byte 159 on the NEXT beat's read.
        let last = bus.read_m(0x4000); // M=161: byte 159 active
        assert_eq!(
            last,
            0x10 + 159,
            "M=161: last DMA conflict byte (index 159)"
        );
        assert_eq!(bus.oam[159], 0x10 + 159, "OAM[159] written");
        // M=162: DMA done, conflicting read sees real memory (ROM 0x4000).
        let after = bus.read_m(0x4000);
        assert_ne!(after, 0x10 + 159, "M=162: DMA inactive, real memory");
    }

    #[test]
    fn finish_speed_switch_toggles_double_speed() {
        let mut bus = Bus::new();
        assert!(!bus.cgb.double_speed);
        bus.finish_speed_switch();
        assert!(bus.cgb.double_speed, "KEY1 switch enables double-speed");
        bus.finish_speed_switch();
        assert!(!bus.cgb.double_speed, "second switch returns to normal");
    }

    #[test]
    fn key1_arm_then_switch_resumes_and_reads_back_speed() {
        // The CGB speed-switch idiom: write KEY1 bit 0 to arm, then STOP.
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        assert!(!bus.speed_switch_armed(), "not armed at power-on");
        // Arm via a KEY1 write ($FF4D bit 0).
        bus.poke(0xFF4D, 0x01);
        assert!(bus.speed_switch_armed(), "KEY1 bit 0 arms the switch");
        // STOP performs the switch: toggle speed + clear the armed bit.
        bus.finish_speed_switch();
        assert!(bus.cgb.double_speed, "switch enabled double-speed");
        assert!(!bus.speed_switch_armed(), "armed bit cleared after switch");
        // KEY1 now reads bit 7 = 1 (double speed), bit 0 = 0 (not armed).
        assert_eq!(bus.peek(0xFF4D) & 0x81, 0x80, "KEY1 reflects double speed");
    }

    #[test]
    fn key1_is_inert_in_dmg_mode() {
        // DMG has no speed switch: KEY1 writes are ignored, never armed, and the
        // register reads as open-bus 0xFF.
        let mut bus = Bus::new(); // cgb_mode defaults to false
        bus.poke(0xFF4D, 0x01);
        assert!(!bus.speed_switch_armed(), "DMG: KEY1 cannot arm a switch");
        assert_eq!(bus.peek(0xFF4D), 0xFF, "DMG: KEY1 reads open-bus 0xFF");
    }

    #[test]
    fn vbk_selects_vram_bank_in_cgb() {
        // CGB: VBK ($FF4F) bit 0 selects the active 8 KiB VRAM bank; the two
        // banks are independent storage.
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        // Bank 0: write a byte.
        bus.poke(0xFF4F, 0x00);
        bus.poke(0x8000, 0xAA);
        // Switch to bank 1: same address is independent storage.
        bus.poke(0xFF4F, 0x01);
        assert_eq!(bus.peek(0x8000), 0x00, "bank 1 is a distinct VRAM page");
        bus.poke(0x8000, 0xBB);
        assert_eq!(bus.peek(0x8000), 0xBB);
        // Back to bank 0: the original byte survives.
        bus.poke(0xFF4F, 0x00);
        assert_eq!(bus.peek(0x8000), 0xAA, "bank 0 retained its byte");
        // VBK reads back bit 0 with the upper bits set.
        bus.poke(0xFF4F, 0x01);
        assert_eq!(bus.peek(0xFF4F), 0xFF, "VBK reads bit0=1 | 0xFE");
    }

    #[test]
    fn vbk_is_inert_in_dmg_mode() {
        // DMG has no VRAM banking: VBK writes are ignored (bank stays 0) and the
        // register reads as open-bus 0xFF.
        let mut bus = Bus::new(); // cgb_mode defaults to false
        bus.poke(0x8000, 0xAA); // bank 0
        bus.poke(0xFF4F, 0x01); // ignored in DMG
        assert_eq!(bus.vbk, 0, "DMG: VBK write ignored, bank stays 0");
        assert_eq!(bus.peek(0x8000), 0xAA, "DMG: still reading bank 0");
        assert_eq!(bus.peek(0xFF4F), 0xFF, "DMG: VBK reads open-bus 0xFF");
    }

    #[test]
    fn svbk_selects_high_wram_bank_in_cgb() {
        // CGB: SVBK ($FF70) bits 0-2 select the D000-DFFF WRAM bank (0->1). C000-
        // CFFF is always bank 0; the high banks are independent storage.
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        // Bank 1 (default) at D000: write a byte.
        bus.poke(0xFF70, 0x01);
        bus.poke(0xD000, 0xAA);
        // Switch to bank 2: D000 is a distinct page.
        bus.poke(0xFF70, 0x02);
        assert_eq!(bus.peek(0xD000), 0x00, "bank 2 is a distinct WRAM page");
        bus.poke(0xD000, 0xBB);
        assert_eq!(bus.peek(0xD000), 0xBB);
        // Back to bank 1: original byte survives.
        bus.poke(0xFF70, 0x01);
        assert_eq!(bus.peek(0xD000), 0xAA, "bank 1 retained its byte");
        // C000-CFFF is always bank 0, unaffected by SVBK.
        bus.poke(0xC000, 0xCC);
        bus.poke(0xFF70, 0x05);
        assert_eq!(bus.peek(0xC000), 0xCC, "C000 is fixed bank 0");
        // SVBK 0 remaps to 1.
        bus.poke(0xFF70, 0x00);
        assert_eq!(bus.svbk, 1, "SVBK 0 remaps to bank 1");
        // Read-back: 3 bits valid, upper bits set.
        bus.poke(0xFF70, 0x03);
        assert_eq!(bus.peek(0xFF70), 0xFB, "SVBK reads bits0-2 | 0xF8");
    }

    #[test]
    fn svbk_is_inert_in_dmg_mode() {
        // DMG has no WRAM banking: SVBK writes ignored (stays 1), reads open-bus.
        let mut bus = Bus::new();
        bus.poke(0xD000, 0xAA);
        bus.poke(0xFF70, 0x03); // ignored in DMG
        assert_eq!(bus.svbk, 1, "DMG: SVBK write ignored, stays bank 1");
        assert_eq!(bus.peek(0xD000), 0xAA, "DMG: still reading bank 1");
        assert_eq!(bus.peek(0xFF70), 0xFF, "DMG: SVBK reads open-bus 0xFF");
    }

    #[test]
    fn wram_echo_region_mirrors_banks() {
        // The echo region E000-FDFF mirrors C000-FDFF (-0x2000): E000-EFFF maps
        // bank 0 (= C000-CFFF), F000-FDFF maps the svbk bank (= D000-DDFF).
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        bus.poke(0xFF70, 0x02); // svbk = bank 2 at D000
                                // Bank 0 via C000, read back via the E000 echo.
        bus.poke(0xC000, 0xC0);
        assert_eq!(bus.peek(0xE000), 0xC0, "E000 echoes C000 (bank 0)");
        // svbk bank via D000, read back via the F000 echo.
        bus.poke(0xD000, 0xD2);
        assert_eq!(bus.peek(0xF000), 0xD2, "F000 echoes D000 (svbk bank)");
        // Writing through the echo updates the underlying bank.
        bus.poke(0xE001, 0x11);
        assert_eq!(bus.peek(0xC001), 0x11, "echo write reaches bank 0");
        // The echo tail FDFF maps D000+0xDFF in the svbk bank.
        bus.poke(0xFDFF, 0x22);
        assert_eq!(bus.peek(0xDDFF), 0x22, "FDFF echoes DDFF (svbk bank)");
    }

    #[test]
    fn opri_is_cgb_only() {
        // OPRI ($FF6C) bit 0 is read/write in CGB, open-bus in DMG.
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        bus.poke(0xFF6C, 0x01);
        assert_eq!(bus.peek(0xFF6C) & 0x01, 0x01, "CGB: OPRI bit 0 stored");
        bus.poke(0xFF6C, 0x00);
        assert_eq!(bus.peek(0xFF6C) & 0x01, 0x00);
        let mut dmg = Bus::new();
        dmg.poke(0xFF6C, 0x01); // ignored
        assert_eq!(dmg.peek(0xFF6C), 0xFF, "DMG: OPRI reads open-bus 0xFF");
    }

    #[test]
    fn io_read_masks_force_unused_bits_high() {
        // Unused HWIO bits read back as 1 (mooneye unused_hwio). Spot-check a few
        // registers across the masked-read path.
        let mut bus = Bus::new();
        // P1 ($FF00): bits 7-6 read 1.
        bus.poke(0xFF00, 0x00);
        assert_eq!(bus.peek(0xFF00) & 0xC0, 0xC0, "P1 bits 7-6 read 1");
        // NR52 ($FF26): bits 6-4 read 1.
        bus.poke(0xFF26, 0x00);
        assert_eq!(bus.peek(0xFF26) & 0x70, 0x70, "NR52 bits 6-4 read 1");
        // NR30 ($FF1A): bits 6-0 read 1.
        bus.poke(0xFF1A, 0x00);
        assert_eq!(bus.peek(0xFF1A) & 0x7F, 0x7F, "NR30 bits 6-0 read 1");
    }

    #[test]
    fn unmapped_io_reads_open_bus_ff() {
        // Truly unmapped HWIO addresses read 0xFF (mooneye test_unmapped).
        let bus = Bus::new();
        for addr in [0xFF03u16, 0xFF08, 0xFF15, 0xFF1F, 0xFF27, 0xFF7F] {
            assert_eq!(bus.peek(addr), 0xFF, "unmapped {addr:#06X} reads 0xFF");
        }
    }

    /// S6: the bus diag seam feeds `diag_record_mcycle!` end-to-end. The CPU
    /// (here a stand-in) builds a record from the bus's post-M-cycle
    /// `flight_fields` + its own PC/opcode, records it, and a fatal dump
    /// preserves it.
    #[cfg(feature = "flight-recorder")]
    #[test]
    fn bus_flight_fields_feed_diag_recorder() {
        use crate::diag::{compiled_features, Diagnostics, FlightRecord, RunContext};

        let root = std::env::temp_dir().join(format!("rubc_n4_diag_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let ctx = RunContext::new(
            &root,
            "/g/x.gb",
            b"rom",
            compiled_features(),
            "DMG",
            "disabled",
        )
        .unwrap();
        let dir = ctx.dir.clone();
        let mut diag = Diagnostics::new(ctx, 64);

        let mut bus = Bus::new();
        bus.poke(0xC000, 0xAB);
        let value = bus.read_m(0xC000); // runs the invariant
        let f = bus.flight_fields(); // bus-observable fields AFTER the M-cycle
                                     // The CPU stand-in merges its own PC/opcode with the bus fields.
        let rec = FlightRecord {
            mcycle: f.mcycle,
            pc: 0x0150,
            opcode: 0x18,
            bus_addr: 0xC000,
            bus_kind: 1, // BusKind::Read
            bus_value: value,
            ie: f.ie,
            if_: f.if_,
            ly: f.ly,
            ppu_mode: f.ppu_mode,
            speed: f.speed,
            dma: f.dma,
            ..Default::default()
        };
        crate::diag_record_mcycle!(&mut diag, rec);
        diag.dump_on_error().unwrap();

        let tail = std::fs::read_to_string(dir.join("flight.tail.txt")).unwrap();
        assert!(tail.contains("pc=0150"), "recorded cycle present in dump");
        assert!(tail.contains("bus=rd@C000=AB"), "bus read captured");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn if_register_is_consistent_across_bus_and_interrupt_logic() {
        // P0#1: writing IF via the bus must affect irq_pending_mask, and
        // clear_if_bit must be visible via read_m. (Previously io[] vs
        // interrupts.if_ desynced these.)
        let mut bus = Bus::new();
        bus.interrupts.ie = 0x1F;
        // Write IF through the bus memory path.
        bus.poke(0xFF0F, 0x04); // request timer
        assert_eq!(
            bus.irq_pending_mask(),
            0x04,
            "bus IF write reaches interrupt logic"
        );
        // Read IF back through the bus: top 3 bits read as 1.
        assert_eq!(bus.peek(0xFF0F), 0xE4, "IF reads back with top bits set");
        // clear_if_bit must be observable via the bus read.
        bus.clear_if_bit(2);
        assert_eq!(bus.peek(0xFF0F) & 0x04, 0, "clear_if_bit visible via read");
        assert_eq!(bus.irq_pending_mask(), 0x00);
    }

    #[test]
    fn requested_if_readable_via_read_m() {
        let mut bus = Bus::new();
        bus.interrupts.ie = 0x1F;
        bus.interrupts.request(0);
        assert_eq!(
            bus.read_m(0xFF0F) & 0x01,
            0x01,
            "requested IF visible via read_m"
        );
    }

    #[test]
    fn oam_dma_blocks_same_bus_writes_except_hram() {
        // P0#2: during DMA the CPU can only write to a bus the DMA is NOT
        // driving. DMA from 0xC0 (WRAM) drives the EXTERNAL memory bus, so
        // WRAM/ROM/SRAM writes are dropped; the VIDEO bus (VRAM), the IO/HRAM
        // region ($FF00-$FFFF, on neither memory bus), and IE stay writable.
        // OAM is always blocked (Pan Docs OAM DMA bus conflicts).
        let mut bus = Bus::new();
        bus.poke(0xFF46, 0xC0); // schedule external-bus DMA (source = WRAM)
        bus.idle_m(); // M=1 setup
        bus.idle_m(); // M=2: DMA active
        bus.write_m(0xC500, 0xAA); // WRAM (external bus) -> blocked
        bus.write_m(0x8000, 0xBB); // VRAM (video bus) -> ALLOWED (other bus)
        bus.write_m(0xFE10, 0xCC); // OAM -> always blocked
        bus.write_m(0xFF42, 0xEE); // IO (SCY) -> ALLOWED (IO bus, not a DMA bus)
        bus.write_m(0xFFFF, 0xEE); // IE -> ALLOWED (IO bus)
        bus.write_m(0xFF85, 0x99); // HRAM -> allowed
        assert_eq!(
            bus.peek(0xC500),
            0x00,
            "WRAM write blocked (same bus as DMA)"
        );
        assert_eq!(
            bus.peek(0x8000),
            0xBB,
            "VRAM write allowed (video bus, DMA drives external bus)"
        );
        assert_eq!(
            bus.peek(0xFF42),
            0xEE,
            "IO (SCY) write allowed during DMA (not on a DMA memory bus)"
        );
        assert_eq!(bus.interrupts.ie, 0xEE, "IE write allowed during DMA");
        assert_eq!(
            bus.peek(0xFE10),
            0x00,
            "OAM write always blocked during DMA"
        );
        assert_eq!(bus.peek(0xFF85), 0x99, "HRAM write allowed during DMA");
    }

    #[test]
    fn read_m_observes_post_tick_state() {
        // P2#6: a functional check that read_m samples AFTER the ticks. The PPU
        // stub's dot_ticks changes during the 4-tick burst; flight_fields read
        // after read_m must reflect the post-tick LY/mode would-be state. Here
        // we prove the tick counter advanced and the value is the post-state by
        // mutating WRAM is irrelevant -- instead check the world moved forward
        // by exactly one M-cycle of dots before the byte was returned.
        let mut bus = Bus::new();
        bus.poke(0xC000, 0x77);
        let dots_before = bus.ppu.dot_ticks;
        let v = bus.read_m(0xC000);
        // The 4 dot-ticks of THIS M-cycle completed before the sample returned.
        assert_eq!(bus.ppu.dot_ticks, dots_before + 4);
        assert_eq!(v, 0x77);
        // ticks_at_last_sample captured the post-burst count, proving sample-last.
        assert_eq!(bus.ticks_at_last_sample(), bus.total_ticks());
    }

    #[test]
    fn timer_reload_quirk_survives_bus_tick_then_sample() {
        // P1a: prove the 4-T TIMA overflow->reload DELAY fires correctly through
        // the real bus path (tick 4 T-cycles per M-cycle, tick-THEN-sample). The
        // mid-reload TMA-copy quirk itself is NOT observable in this M-granular
        // model (see the companion test); the quirk is unit-tested in isolation by
        // bus::timer::tests::reload_cycle_write_quirks. Here we observe the reload
        // through TIMA (0 -> TMA) via idle_m M-cycles.
        let mut bus = Bus::new();
        // Prime: TIMA=0xFF, TMA=0x11, timer enabled on the fastest selector (bit 3).
        bus.timer.test_prime_for_overflow(0x11, 0b01);

        // First bus idle M-cycle ticks 4 T: the falling edge overflows TIMA -> 0,
        // scheduling Reload::Pending(4).
        bus.idle_m();
        assert_eq!(
            bus.timer.test_tima(),
            0x00,
            "TIMA reads 0 right after overflow"
        );

        // Walk M-cycles with idle_m until the reload fires, observed as TIMA going
        // 0 -> TMA (0x11). This proves the 4-T reload delay through the real bus
        // tick path. (The timer IRQ is queued in `interrupts.pending` and only
        // becomes visible via irq_pending_mask after a boundary, so we observe the
        // reload through TIMA, not the IRQ mask.)
        let mut reload_m = None;
        for m in 0..8 {
            bus.idle_m();
            if bus.timer.test_tima() == 0x11 {
                reload_m = Some(m);
                break;
            }
        }
        assert!(
            reload_m.is_some(),
            "timer reload (TIMA <- TMA) must fire after overflow via the bus"
        );
    }

    #[test]
    fn timer_reload_completes_within_the_m_cycle_via_bus() {
        // P1a companion: characterise the reload through the real bus. The reload
        // fires 4 T after overflow; because the bus invariant samples a CPU write
        // only AFTER the 4 T ticks of an M-cycle (tick-THEN-sample), the reload
        // always COMPLETES within those ticks and `ReloadedThisTick` is cleared
        // before the write is sampled. So a CPU TMA write cannot observe the
        // mid-reload copy quirk in this M-granular bus model.
        //
        // That sub-M-cycle write-during-reload timing is exactly what the mooneye
        // `tima_write_reloading` / `tma_write_reloading` ROMs check; it is a
        // DEFERRED gate (needs sub-instruction bus write placement, tracked for
        // the timing-ROM wave). The reload-cycle COPY QUIRK ITSELF is proven
        // correct in isolation by `bus::timer::tests::reload_cycle_write_quirks`.
        //
        // Here we assert the observable bus behaviour: TIMA reloads to TMA after
        // the delay and keeps counting; a TMA write lands as a normal TMA update
        // (not a mid-reload copy).
        let mut bus = Bus::new();
        bus.timer.test_prime_for_overflow(0x11, 0b01);
        bus.idle_m(); // overflow -> Pending(4)
        assert_eq!(bus.timer.test_tima(), 0x00);

        let mut trace = Vec::new();
        for _ in 0..8 {
            bus.write_m(0xFF06, 0x99);
            trace.push(bus.timer.test_tima());
        }
        // TIMA reloads to the OLD TMA (0x11) at the delay, then increments. The
        // 0x99 TMA write does NOT retro-copy into TIMA via the M-granular bus.
        assert!(
            trace.contains(&0x11),
            "TIMA must reload to the pre-write TMA via the bus. trace={trace:?}"
        );
        assert!(
            !trace.contains(&0x99),
            "M-granular bus cannot observe the mid-reload TMA-copy quirk. trace={trace:?}"
        );
    }

    #[test]
    fn hdma_general_purpose_copies_all_blocks_at_once() {
        // GDMA (HDMA5 bit 7 = 0): copy the whole length immediately, then HDMA5
        // reads back 0xFF (idle/complete).
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        // Source data in WRAM ($C000..). Fill 0x20 bytes (2 blocks).
        for i in 0..0x20u16 {
            bus.poke(0xC000 + i, (i as u8).wrapping_add(1));
        }
        // Source = $C000, dest = VRAM $8000 (offset 0).
        bus.poke(0xFF51, 0xC0);
        bus.poke(0xFF52, 0x00);
        bus.poke(0xFF53, 0x00);
        bus.poke(0xFF54, 0x00);
        // Length = 2 blocks ($20 bytes): value (len/0x10 - 1) = 1, bit 7 = 0.
        bus.poke(0xFF55, 0x01);
        // Transfer is immediate: HDMA5 reports complete.
        assert_eq!(bus.peek(0xFF55), 0xFF, "GDMA completes immediately");
        // VRAM bank 0 now holds the copied bytes.
        for i in 0..0x20u16 {
            assert_eq!(
                bus.vram[0][i as usize],
                (i as u8).wrapping_add(1),
                "GDMA byte {i} copied to VRAM"
            );
        }
    }

    #[test]
    fn hdma_general_purpose_is_inert_in_dmg_mode() {
        // DMG has no VRAM DMA: HDMA writes are ignored, HDMA5 reads open-bus.
        let mut bus = Bus::new(); // cgb_mode = false
        bus.poke(0xC000, 0xAB);
        bus.poke(0xFF51, 0xC0);
        bus.poke(0xFF52, 0x00);
        bus.poke(0xFF53, 0x00);
        bus.poke(0xFF54, 0x00);
        bus.poke(0xFF55, 0x00);
        assert_eq!(bus.peek(0xFF55), 0xFF, "DMG: HDMA5 reads open-bus");
        assert_eq!(bus.vram[0][0], 0x00, "DMG: no VRAM DMA occurred");
    }

    #[test]
    fn hdma_hblank_copies_one_block_per_hblank() {
        // HBlank DMA (HDMA5 bit 7 = 1): one $10 block transfers per HBlank entry.
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        // Source data: 0x20 bytes in WRAM.
        for i in 0..0x20u16 {
            bus.poke(0xC000 + i, (i as u8).wrapping_add(0x40));
        }
        bus.poke(0xFF51, 0xC0);
        bus.poke(0xFF52, 0x00);
        bus.poke(0xFF53, 0x00);
        bus.poke(0xFF54, 0x00);
        // Arm HBlank DMA, 2 blocks (value 1, bit 7 = 1).
        bus.poke(0xFF55, 0x81);
        // Nothing copied until an HBlank entry.
        assert_eq!(bus.vram[0][0], 0x00, "no copy before first HBlank");
        assert_eq!(bus.peek(0xFF55) & 0x80, 0x00, "transfer is active");

        // Drive M-cycles until the LCD reaches HBlank, then a block copies.
        let mut copied = false;
        for _ in 0..200 {
            bus.idle_m();
            if bus.vram[0][0] != 0 {
                copied = true;
                break;
            }
        }
        assert!(copied, "one block copied on entering HBlank");
        assert_eq!(bus.vram[0][0], 0x40, "first HBlank block byte copied");
    }

    #[test]
    fn hdma_hblank_can_be_terminated() {
        // Writing HDMA5 with bit 7 = 0 during an active HBlank transfer stops it;
        // HDMA5 then reads bit 7 = 1 with the remaining-block count in bits 6-0.
        let mut bus = Bus::new();
        bus.cgb.cgb_mode = true;
        bus.poke(0xFF51, 0xC0);
        bus.poke(0xFF52, 0x00);
        bus.poke(0xFF53, 0x00);
        bus.poke(0xFF54, 0x00);
        bus.poke(0xFF55, 0x84); // arm HBlank DMA, 5 blocks
        assert_eq!(bus.peek(0xFF55) & 0x80, 0x00, "active before stop");
        bus.poke(0xFF55, 0x00); // terminate
        assert_eq!(bus.peek(0xFF55) & 0x80, 0x80, "bit 7 set after termination");
    }

    #[test]
    fn div_apu_clocks_at_512hz() {
        // The DIV-APU frame sequencer steps on the falling edge of the visible
        // DIV bit 4 (bit 12 of the 16-bit counter) = every 8192 T-cycles
        // (512 Hz). Regression guard for the mask bug that ran it 256x too fast.
        let mut bus = Bus::new();
        let mask = 0x1000u16;
        let mut prev = bus.timer.div_counter() & mask != 0;
        let mut last = 0u64;
        let mut t = 0u64;
        let mut intervals = Vec::new();
        for _ in 0..60_000 {
            bus.tick_cpu_t();
            t += 1;
            let high = bus.timer.div_counter() & mask != 0;
            if prev && !high {
                intervals.push(t - last);
                last = t;
                if intervals.len() >= 4 {
                    break;
                }
            }
            prev = high;
        }
        assert_eq!(
            intervals,
            vec![8192, 8192, 8192, 8192],
            "DIV-APU must step every 8192 T-cycles (512 Hz)"
        );
    }

    #[test]
    fn joypad_reads_idle_when_no_input() {
        // P1/JOYP ($FF00): with no buttons pressed (no input wired), the low
        // nibble must read 1111 (active-low: 1 = not pressed) regardless of the
        // line-select bits the game writes. Bits 7-6 always read 1. Returning a
        // 0 low nibble signals "all buttons held" and stalls games (e.g.
        // Pokemon Crystal) that wait for no-input during boot.
        let mut bus = Bus::new();
        // Game selects the button line ($20) -> reads must still show idle.
        bus.poke(0xFF00, 0x20);
        assert_eq!(bus.peek(0xFF00), 0xEF, "button line selected, none pressed");
        // Select the d-pad line ($10).
        bus.poke(0xFF00, 0x10);
        assert_eq!(bus.peek(0xFF00), 0xDF, "d-pad line selected, none pressed");
        // Neither line selected (both bits high): low nibble still 1111.
        bus.poke(0xFF00, 0x30);
        assert_eq!(bus.peek(0xFF00), 0xFF, "no line selected, all bits 1");
    }

    #[test]
    fn joypad_reads_pressed_buttons_and_raises_irq() {
        let mut bus = Bus::new();
        bus.interrupts.ie = 0x10; // enable joypad interrupt (bit 4)

        // Select the action-button line (bit 5 = 0 selects action).
        bus.poke(0xFF00, 0x10); // bit5=0 (action selected), bit4=1 (dpad not)
                                // Press A -> action line bit 0 reads 0 (active-low). Fresh press of a
                                // selected-line button raises the joypad IRQ (IF bit 4).
        bus.set_button(Button::A, true);
        assert_eq!(bus.peek(0xFF00) & 0x0F, 0x0E, "A pressed -> bit0 low");
        assert_ne!(bus.interrupts.if_ & 0x10, 0, "joypad IRQ raised on press");

        // While the action line is selected, a d-pad press is NOT visible.
        bus.set_button(Button::Right, true);
        assert_eq!(
            bus.peek(0xFF00) & 0x0F,
            0x0E,
            "dpad press invisible while action line selected"
        );

        // Switch to the d-pad line (bit 4 = 0): Right now reads low, A hidden.
        bus.poke(0xFF00, 0x20); // bit4=0 (dpad selected), bit5=1 (action not)
        assert_eq!(bus.peek(0xFF00) & 0x0F, 0x0E, "Right pressed -> bit0 low");

        // Release everything -> idle.
        bus.set_button(Button::A, false);
        bus.set_button(Button::Right, false);
        assert_eq!(bus.peek(0xFF00) & 0x0F, 0x0F, "all released -> idle");
    }

    #[test]
    fn serial_internal_clock_completes_after_eight_slow_edges() {
        let mut bus = Bus::new();
        bus.interrupts.ie = 0x08;
        for _ in 0..52 {
            bus.tick_cpu_t();
        }
        bus.poke(0xFF01, 0x00);
        bus.poke(0xFF02, 0x81);

        for _ in 0..(8 * 512 - 1) {
            bus.tick_cpu_t();
        }
        assert_eq!(bus.peek(0xFF02) & 0x80, 0x80, "transfer still active");
        assert_eq!(
            bus.peek(0xFF01),
            0x7F,
            "seven disconnected-link bits shifted in"
        );
        assert_eq!(
            bus.interrupts.if_ & 0x08,
            0x00,
            "no serial IRQ before final edge"
        );

        bus.tick_cpu_t();

        assert_eq!(bus.peek(0xFF01), 0xFF, "disconnected link shifts in ones");
        assert_eq!(
            bus.peek(0xFF02) & 0x80,
            0x00,
            "transfer complete clears SC bit 7"
        );
        assert_eq!(
            bus.interrupts.if_ & 0x08,
            0x08,
            "serial IRQ requested on completion"
        );
    }

    #[test]
    fn serial_sc_read_masks_unused_bits() {
        let mut bus = Bus::new();
        bus.poke(0xFF02, 0x00);
        assert_eq!(bus.peek(0xFF02), 0x7E, "DMG SC unused bits read high");

        bus.poke(0xFF02, 0x02);
        assert_eq!(bus.peek(0xFF02), 0x7E, "DMG ignores CGB fast-clock bit");

        bus.cgb.cgb_mode = true;
        bus.poke(0xFF02, 0x02);
        assert_eq!(bus.peek(0xFF02), 0x7E, "CGB exposes SC bit 1");
    }

    #[test]
    fn serial_external_clock_waits_for_partner_clock() {
        let mut bus = Bus::new();
        bus.poke(0xFF01, 0xA5);
        bus.poke(0xFF02, 0x80);

        for _ in 0..(8 * 512 + 512) {
            bus.tick_cpu_t();
        }

        assert_eq!(
            bus.peek(0xFF01),
            0xA5,
            "external-clock transfer does not shift itself"
        );
        assert_eq!(
            bus.peek(0xFF02) & 0x80,
            0x80,
            "external-clock transfer remains active"
        );
        assert_eq!(
            bus.interrupts.if_ & 0x08,
            0x00,
            "no serial IRQ without partner clock"
        );
    }
}
