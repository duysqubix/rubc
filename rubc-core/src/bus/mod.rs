//! M-cycle bus skeleton + `CpuBus` trait + the per-M-cycle invariant.
//!
//! ## The invariant (Oracle-locked, user-approved)
//!
//! Every CPU M-cycle, in `begin_cpu_m_cycle`, runs in EXACTLY this order:
//!   1. **OAM-DMA beat** — transfer one byte, record `conflict_byte_this_m` +
//!      `active_for_cpu_this_m` (so a conflicting CPU access this M-cycle sees
//!      the in-flight DMA byte).
//!   2. **4× `tick_cpu_t`** — timer / PPU / APU / serial / DMA advance. In CGB
//!      double-speed, PPU + APU advance every 2nd T (twice per M); the timer
//!      always advances 4 per M.
//!   3. **THEN** the latched access — `cpu_read_latched` / `cpu_write_latched`.
//!      This is **tick-THEN-sample**: the value is sampled at the END of the
//!      M-cycle, after peripherals have advanced.
//!   4. IRQs raised during steps 2–3 become visible at the NEXT instruction
//!      boundary (queued in `Interrupts::pending`, settled by `boundary`).
//!
//! Diagnostics OBSERVE only; nothing here is on a diag hot path that ticks.

pub mod cartridge;
pub mod flat;
pub mod sm83_vectors;
pub mod stubs;
pub mod timer;

pub use cartridge::Cartridge;
pub use flat::FlatBus;

use stubs::{ApuStub, CgbState, Interrupts, PpuStub};
use timer::Timer;

/// What the CPU calls each M-cycle. Each `*_m` method runs the full invariant.
pub trait CpuBus {
    /// Read one byte over one M-cycle (tick-THEN-sample).
    fn read_m(&mut self, addr: u16) -> u8;
    /// Write one byte over one M-cycle.
    fn write_m(&mut self, addr: u16, value: u8);
    /// Burn one M-cycle with no memory access (internal CPU work).
    fn idle_m(&mut self);

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
    /// Settle queued interrupts into IF at an instruction boundary. The CPU
    /// (which may be generic over `CpuBus`) calls this before polling
    /// `irq_pending_mask`, so IRQs raised mid-M-cycle become visible.
    fn boundary(&mut self);
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
    start_delay_m: u8,
    /// The byte transferred this M-cycle (what a conflicting CPU read sees).
    conflict_byte_this_m: Option<u8>,
    /// Whether DMA owns the bus for the CPU this M-cycle.
    active_for_cpu_this_m: bool,
}

/// The whole non-CPU machine. Owns all memory + tickable peripherals. The CPU
/// borrows this `&mut` for one `*_m` call at a time; no peripheral holds a
/// reference back, so the borrow checker stays happy.
pub struct Bus {
    // Memory regions (flat placeholders; banking lands in MBC/CGB waves).
    pub cart: Cartridge,
    pub wram: [u8; 0x2000],
    pub hram: [u8; 0x7F],
    pub vram: [u8; 0x2000],
    pub oam: [u8; 0xA0],
    pub io: [u8; 0x80],

    pub interrupts: Interrupts,
    pub timer: Timer,
    pub ppu: PpuStub,
    pub apu: ApuStub,
    pub cgb: CgbState,
    dma: OamDma,

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
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            io: [0; 0x80],
            interrupts: Interrupts::default(),
            timer: Timer::power_on(),
            ppu: PpuStub::default(),
            apu: ApuStub::default(),
            cgb: CgbState::default(),
            dma: OamDma::default(),
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

    /// Settle queued interrupts into IF. Call at each instruction boundary
    /// (before the CPU polls `irq_pending_mask`).
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

    /// Run one CPU M-cycle's worth of "the world advancing": OAM-DMA beat, then
    /// 4 T-cycle ticks. The latched access (or idle) happens AFTER this returns.
    fn begin_cpu_m_cycle(&mut self) {
        // (1) OAM DMA beat — before the ticks, before the CPU latch.
        self.oam_dma_beat();

        // (2) 4 CPU T-cycles. PPU/APU advance every 2nd T in double-speed.
        for _ in 0..4 {
            self.tick_cpu_t();
        }
    }

    fn tick_cpu_t(&mut self) {
        self.t_tick_count += 1;

        // Timer always advances every T.
        self.timer.tick_t(&mut self.interrupts);

        if self.cgb.double_speed {
            // PPU/APU advance every 2nd T (twice per M-cycle). PROVISIONAL: with
            // t_phase starting false, the toggle-to-true fires on T1/T3 (not
            // T2/T4). The T-phase PARITY is unverified — calibrate against CGB
            // dot-accurate tests before the CGB timing wave.
            self.cgb.t_phase = !self.cgb.t_phase;
            if self.cgb.t_phase {
                self.ppu.tick_dot(&mut self.interrupts);
                self.apu.tick_t();
            }
        } else {
            self.ppu.tick_dot(&mut self.interrupts);
            self.apu.tick_t();
        }
    }

    fn oam_dma_beat(&mut self) {
        self.dma.conflict_byte_this_m = None;
        self.dma.active_for_cpu_this_m = false;

        if self.dma.start_delay_m > 0 {
            self.dma.start_delay_m -= 1;
            if self.dma.start_delay_m == 0 {
                self.dma.active = true;
                self.dma.index = 0;
            }
            return;
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

    /// Schedule an OAM DMA from `0xXX00` (value = `XX`).
    pub fn start_oam_dma(&mut self, value: u8) {
        self.dma.source_hi = value;
        // PROVISIONAL: 2-M-cycle startup delay before the first transfer is not
        // yet ROM-calibrated. Verify against mooneye `acceptance/oam_dma/*`
        // (oam_dma_start, oam_dma_timing) in the OAM-DMA wave; adjust if it fails.
        self.dma.start_delay_m = 2;
        self.dma.active = false;
    }

    /// Side-effect-free read of the DMA source (no ticking).
    fn read_dma_source(&self, addr: u16) -> u8 {
        self.peek(addr)
    }

    // ---- latched access (sample at END of M) -------------------------------

    fn cpu_read_latched(&mut self, addr: u16) -> u8 {
        self.ticks_at_last_sample = self.t_tick_count;

        // OAM DMA bus conflict: CPU sees the in-flight byte; OAM blocked; HRAM ok.
        if self.dma.active_for_cpu_this_m {
            match addr {
                0xFF80..=0xFFFE => {} // HRAM accessible
                0xFE00..=0xFE9F => return 0xFF,
                _ => {
                    if let Some(b) = self.dma.conflict_byte_this_m {
                        return b;
                    }
                }
            }
        }
        self.peek(addr)
    }

    fn cpu_write_latched(&mut self, addr: u16, value: u8) {
        self.ticks_at_last_sample = self.t_tick_count;

        if self.dma.active_for_cpu_this_m {
            // During OAM DMA the CPU can write ONLY to HRAM; the DMA owns every
            // other bus, so all other writes (WRAM/VRAM/OAM/IO/IE) are dropped.
            if !matches!(addr, 0xFF80..=0xFFFE) {
                return;
            }
        }
        self.poke(addr, value);
    }

    /// Side-effect-free read (no tick). The decode is a flat placeholder; real
    /// banking/IO-side-effects land in later waves.
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cart.read(addr),
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize], // echo
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF0F => self.interrupts.if_ | 0xE0, // IF lives in interrupts, not io[]
            0xFF4D if self.cgb.cgb_mode => {
                // KEY1 (CGB only): bit 7 = current speed (1 = double), bit 0 =
                // armed switch, remaining bits read as 1.
                let speed = if self.cgb.double_speed { 0x80 } else { 0x00 };
                speed | (self.io[0x4D] & 0x01) | 0x7E
            }
            0xFF4D => 0xFF, // KEY1 in DMG mode: open-bus (no speed switch)
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.interrupts.ie,
            0xA000..=0xBFFF => self.cart.read_ram(addr),
            _ => 0xFF, // unmapped
        }
    }

    /// Side-effect-free write (no tick). Flat placeholder.
    pub fn poke(&mut self, addr: u16, value: u8) {
        match addr {
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = value,
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = value,
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = value,
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = value,
            0xFF04..=0xFF07 => self.timer.write(addr, value, &mut self.interrupts),
            0xFF46 => self.start_oam_dma(value),
            0xFF0F => self.interrupts.if_ = value | 0xE0, // IF lives in interrupts
            0xFF4D => {
                // KEY1: only the "prepare speed switch" bit (0) is writable, and
                // only in CGB mode. DMG ignores KEY1 entirely (no speed switch).
                if self.cgb.cgb_mode {
                    self.io[0x4D] = (self.io[0x4D] & !0x01) | (value & 0x01);
                }
            }
            0xFF02 => {
                // Serial control: writing bit 7 starts a transfer. With no link
                // partner, capture the SB byte (the blargg result channel) and
                // immediately clear bit 7 to signal transfer-complete, so a ROM
                // that polls SC for completion does not hang.
                if value & 0x80 != 0 {
                    self.serial_out.push(self.io[0x01]);
                    self.io[0x02] = value & !0x80;
                } else {
                    self.io[0x02] = value;
                }
            }
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize] = value,
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
        self.begin_cpu_m_cycle();
        self.cpu_read_latched(addr)
    }

    fn write_m(&mut self, addr: u16, value: u8) {
        self.begin_cpu_m_cycle();
        self.cpu_write_latched(addr, value);
    }

    fn idle_m(&mut self) {
        self.begin_cpu_m_cycle();
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

    #[test]
    fn roundtrip_wram_read_write() {
        // S4: borrow-safe round-trip through the CpuBus API.
        let mut bus = Bus::new();
        bus.write_m(0xC123, 0xAB);
        assert_eq!(bus.read_m(0xC123), 0xAB);
    }

    #[test]
    fn irq_visible_next_boundary() {
        // S5: an IRQ requested during a tick is NOT visible until boundary().
        let mut bus = Bus::new();
        bus.interrupts.ie = 0x04; // enable timer interrupt
        bus.interrupts.request(2); // timer IRQ requested mid-M
        assert_eq!(bus.irq_pending_mask(), 0x00, "not visible before boundary");
        bus.boundary();
        assert_eq!(bus.irq_pending_mask(), 0x04, "visible after boundary");
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

        // start_delay_m = 2: two M-cycles before the first transfer.
        bus.idle_m();
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
    fn settled_if_readable_via_read_m() {
        // request -> boundary -> read_m(FF0F) sees the settled bit.
        let mut bus = Bus::new();
        bus.interrupts.ie = 0x1F;
        bus.interrupts.request(0); // vblank, queued
        bus.boundary();
        assert_eq!(
            bus.read_m(0xFF0F) & 0x01,
            0x01,
            "settled IF visible via read_m"
        );
    }

    #[test]
    fn oam_dma_blocks_all_writes_except_hram() {
        // P0#2: during DMA only HRAM is writable; WRAM/VRAM/OAM/IO/IE dropped.
        let mut bus = Bus::new();
        bus.poke(0xFF46, 0xC0); // schedule DMA (start_delay=2)
        bus.idle_m();
        bus.idle_m(); // DMA now active
                      // Pre-seed targets so we can prove writes are dropped.
                      // (poke bypasses the DMA gate; write_m is the gated path.)
        bus.write_m(0xC500, 0xAA); // WRAM -> blocked
        bus.write_m(0x8000, 0xBB); // VRAM -> blocked
        bus.write_m(0xFE10, 0xCC); // OAM  -> blocked
        bus.write_m(0xFF40, 0xDD); // IO (LCDC) -> blocked
        bus.write_m(0xFFFF, 0xEE); // IE  -> blocked
        bus.write_m(0xFF85, 0x99); // HRAM -> allowed
        assert_eq!(bus.peek(0xC500), 0x00, "WRAM write blocked during DMA");
        assert_eq!(bus.peek(0x8000), 0x00, "VRAM write blocked during DMA");
        assert_eq!(bus.peek(0xFF40), 0x00, "IO write blocked during DMA");
        assert_eq!(bus.peek(0xFFFF), 0x00, "IE write blocked during DMA");
        assert_eq!(bus.peek(0xFE10), 0x00, "OAM write blocked during DMA");
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
}
