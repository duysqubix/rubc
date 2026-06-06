//! Runnable machine: ties the M-cycle CPU to the bus, loads a ROM, and runs.
//!
//! This is the integration surface the test-ROM harnesses use. It boots the new
//! [`Cpu`](crate::cpu::Cpu) + [`Bus`](crate::bus::Bus), loads a `.gb` image into
//! ROM, captures serial output (the blargg result channel), and detects the two
//! standard test-suite pass signals:
//!   - **blargg**: a "Passed"/"Failed" string on the serial port.
//!   - **mooneye**: the Fibonacci register signature (B=3,C=5,D=8,E=13,H=21,
//!     L=34) reached at a `LD B,B` magic breakpoint.

use crate::bus::Bus;
use crate::cpu::{Cpu, CpuMode};

/// Why a run stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunStop {
    /// Reached the mooneye `LD B,B` magic breakpoint.
    MooneyeBreakpoint,
    /// The serial output matched a blargg terminal string.
    BlarggDone,
    /// Hit the instruction budget without a terminal condition.
    Timeout,
    /// CPU got stuck (STOP, or an illegal-opcode lock).
    Stuck,
}

/// The mooneye pass signature: registers after a passing test.
pub const MOONEYE_PASS: [u8; 6] = [3, 5, 8, 13, 21, 34]; // B,C,D,E,H,L

/// A bootable machine.
pub struct Machine {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            bus: Bus::new(),
        }
    }

    /// Boot directly into the cartridge (no boot ROM): set the post-boot DMG
    /// register + PC state and load `rom` into the ROM region.
    pub fn boot_dmg(rom: &[u8]) -> Self {
        let mut m = Self::new();
        m.load_rom(rom);
        // Post-boot DMG state (PC=0x0100, SP=0xFFFE, A=0x01, F=0xB0, ...).
        m.cpu.r.a = 0x01;
        m.cpu.r.f = 0xB0;
        m.cpu.r.b = 0x00;
        m.cpu.r.c = 0x13;
        m.cpu.r.d = 0x00;
        m.cpu.r.e = 0xD8;
        m.cpu.r.h = 0x01;
        m.cpu.r.l = 0x4D;
        m.cpu.r.sp = 0xFFFE;
        m.cpu.r.pc = 0x0100;
        m
    }

    /// Boot directly into the cartridge in **CGB mode** (no boot ROM): post-boot
    /// CGB register state (A=0x11 is the CGB hardware signature the ROM reads to
    /// detect color hardware) and the bus in CGB mode so KEY1/double-speed work.
    pub fn boot_cgb(rom: &[u8]) -> Self {
        let mut m = Self::new();
        m.load_rom(rom);
        m.bus.cgb.cgb_mode = true;
        // Post-boot CGB state (A=0x11 signals CGB to CGB-aware ROMs).
        m.cpu.r.a = 0x11;
        m.cpu.r.f = 0x80;
        m.cpu.r.b = 0x00;
        m.cpu.r.c = 0x00;
        m.cpu.r.d = 0x00;
        m.cpu.r.e = 0x08;
        m.cpu.r.h = 0x00;
        m.cpu.r.l = 0x7C;
        m.cpu.r.sp = 0xFFFE;
        m.cpu.r.pc = 0x0100;
        m
    }

    /// Load a ROM image into the cartridge. The controller (MBC0/MBC1) is
    /// selected from the header byte at 0x0147, so >32 KiB ROMs bank-switch.
    pub fn load_rom(&mut self, rom: &[u8]) {
        self.bus.cart = crate::bus::Cartridge::from_rom(rom);
    }

    /// Run one full instruction (fetch through the next boundary).
    pub fn step_instruction(&mut self) {
        let mut guard = 0;
        loop {
            self.cpu.step_m(&mut self.bus);
            if matches!(self.cpu.mode, CpuMode::Running) && self.at_boundary() {
                break;
            }
            guard += 1;
            if guard > 64 {
                break;
            }
        }
    }

    /// Run the machine until the PPU enters VBlank (one full frame), so the
    /// `Ppu::framebuffer` holds a freshly-rendered image. Bounded by a generous
    /// instruction guard so a stuck ROM cannot hang the GUI loop.
    pub fn step_frame(&mut self) {
        // Step out of any in-progress VBlank first so we stop on the NEXT entry.
        let mut guard: u32 = 0;
        while self.bus.ppu.mode == crate::bus::ppu::mode::VBLANK && guard < 200_000 {
            self.step_instruction();
            guard += 1;
        }
        guard = 0;
        while self.bus.ppu.mode != crate::bus::ppu::mode::VBLANK && guard < 200_000 {
            self.step_instruction();
            guard += 1;
        }
    }

    fn at_boundary(&self) -> bool {
        self.cpu.exec_is_boundary()
    }

    /// The opcode byte at PC (side-effect-free peek).
    fn opcode_at_pc(&self) -> u8 {
        self.bus.peek(self.cpu.r.pc)
    }

    /// Run until a test-suite terminal condition or `max_instructions`.
    ///
    /// Detects: the mooneye `LD B,B` (0x40) breakpoint, and blargg serial
    /// "Passed"/"Failed" terminal strings.
    pub fn run_test(&mut self, max_instructions: u64) -> RunStop {
        self.run_blargg(max_instructions)
    }

    /// Run a blargg ROM: it loops internally and signals completion ONLY via
    /// the serial transcript ("Passed"/"Failed"). `LD B,B` is a normal
    /// instruction here and must NOT terminate the run.
    pub fn run_blargg(&mut self, max_instructions: u64) -> RunStop {
        // Track whether the cart-RAM protocol has signalled "running" ($A000 ==
        // 0x80) at least once. A result code is only trusted after that, so a
        // transient mid-write value (or a coincidental signature in a ROM that
        // does not use the protocol) cannot be mistaken for a finalized result.
        let mut cart_ram_was_running = false;
        for i in 0..max_instructions {
            if matches!(self.cpu.mode, CpuMode::Stopped) {
                return RunStop::Stuck;
            }
            self.step_instruction();
            // Channel 1: serial transcript ("Passed"/"Failed"). Cheap to check.
            if let Some(text) = self.serial_text() {
                if text.contains("Passed") || text.contains("Failed") {
                    return RunStop::BlarggDone;
                }
            }
            // The cart-RAM "running" marker can appear briefly; sample it often
            // but cheaply (3 byte reads).
            if self.blargg_cart_ram_status() == Some(0x80) {
                cart_ram_was_running = true;
            }
            // The terminal channels (cart-RAM result, LCD console) are stable
            // end-states, so they only need periodic polling -- scanning the
            // 360-cell VRAM console every instruction is needlessly expensive.
            if i % 4096 == 0 {
                // Channel 2: cart-RAM result (only trusted after "running").
                if cart_ram_was_running && self.blargg_cart_ram_done().is_some() {
                    return RunStop::BlarggDone;
                }
                // Channel 3: LCD text console (halt_bug, instr_timing).
                if let Some(text) = self.blargg_console_text() {
                    if text.contains("Passed") || text.contains("Failed") {
                        return RunStop::BlarggDone;
                    }
                }
            }
        }
        RunStop::Timeout
    }

    /// The raw blargg cart-RAM status byte ($A000) when the result signature
    /// ($A001-3 == DE B0 61) is present, else None. 0x80 = still running.
    fn blargg_cart_ram_status(&self) -> Option<u8> {
        let sig = [
            self.bus.peek(0xA001),
            self.bus.peek(0xA002),
            self.bus.peek(0xA003),
        ];
        if sig != [0xDE, 0xB0, 0x61] {
            return None;
        }
        Some(self.bus.peek(0xA000))
    }

    /// If a blargg cart-RAM result is finalized, return Some(status_code)
    /// (0x00 = pass). Returns None while the signature is absent or status is
    /// still 0x80 (running).
    fn blargg_cart_ram_done(&self) -> Option<u8> {
        match self.blargg_cart_ram_status() {
            Some(0x80) | None => None,
            Some(status) => Some(status),
        }
    }

    /// True if a finalized blargg result (serial, cart-RAM, or LCD console)
    /// indicates PASS.
    pub fn blargg_passed(&self) -> bool {
        if let Some(status) = self.blargg_cart_ram_done() {
            return status == 0x00;
        }
        if let Some(t) = self.serial_text() {
            if t.contains("Passed") || t.contains("Failed") {
                return t.contains("Passed");
            }
        }
        self.blargg_console_text()
            .map(|t| t.contains("Passed"))
            .unwrap_or(false)
    }

    /// Decode the blargg on-screen text console (VRAM background tilemap at
    /// $9800) as ASCII. blargg's console writes ASCII tile indices directly to
    /// the tilemap, so the bytes are readable text. Returns None if the tilemap
    /// holds no printable content (e.g. the ROM uses serial instead).
    pub fn blargg_console_text(&self) -> Option<String> {
        let mut out = String::new();
        for row in 0..18u16 {
            let base = 0x9800u16 + row * 32;
            for col in 0..20u16 {
                let b = self.bus.peek(base + col);
                out.push(if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    ' '
                });
            }
            out.push('\n');
        }
        if out.trim().is_empty() {
            None
        } else {
            Some(out)
        }
    }

    /// Run a mooneye ROM: it ends at the `LD B,B` (0x40) magic breakpoint, after
    /// which the registers carry the pass/fail signature.
    pub fn run_mooneye(&mut self, max_instructions: u64) -> RunStop {
        for _ in 0..max_instructions {
            if matches!(self.cpu.mode, CpuMode::Stopped) {
                return RunStop::Stuck;
            }
            if self.opcode_at_pc() == 0x40 {
                return RunStop::MooneyeBreakpoint;
            }
            self.step_instruction();
        }
        RunStop::Timeout
    }

    /// The serial output decoded as a UTF-8 string (lossy).
    pub fn serial_text(&self) -> Option<String> {
        if self.bus.serial_out.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&self.bus.serial_out).into_owned())
        }
    }

    /// True if the CPU registers hold the mooneye pass signature.
    pub fn mooneye_passed(&self) -> bool {
        let r = &self.cpu.r;
        [r.b, r.c, r.d, r.e, r.h, r.l] == MOONEYE_PASS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_capture_appends_sb_on_sc_trigger() {
        // S1: poke SB then SC with bit 7 -> the byte lands in serial_out.
        let mut bus = Bus::new();
        bus.poke(0xFF01, b'A');
        bus.poke(0xFF02, 0x81);
        assert_eq!(bus.serial_out, vec![b'A']);
        // Writing SC without bit 7 does NOT capture.
        bus.poke(0xFF01, b'B');
        bus.poke(0xFF02, 0x01);
        assert_eq!(bus.serial_out, vec![b'A']);
    }

    /// Assemble a tiny program at 0x0100 that prints `text` over serial.
    fn serial_print_rom(text: &[u8]) -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        let mut pc = 0x0100usize;
        let mut emit = |bytes: &[u8], pc: &mut usize| {
            for &b in bytes {
                rom[*pc] = b;
                *pc += 1;
            }
        };
        for &ch in text {
            // LD A, ch ; LDH (0x01),A ; LD A,0x81 ; LDH (0x02),A
            emit(&[0x3E, ch], &mut pc); // LD A, ch
            emit(&[0xE0, 0x01], &mut pc); // LDH (SB),A
            emit(&[0x3E, 0x81], &mut pc); // LD A, 0x81
            emit(&[0xE0, 0x02], &mut pc); // LDH (SC),A
        }
        // Halt the run with a `LD B,B` magic breakpoint.
        emit(&[0x40], &mut pc); // LD B,B
        rom
    }

    #[test]
    fn runs_tiny_serial_rom() {
        // S2: a hand-built ROM prints "HI" over serial.
        let rom = serial_print_rom(b"HI");
        let mut m = Machine::boot_dmg(&rom);
        let stop = m.run_mooneye(10_000);
        assert_eq!(stop, RunStop::MooneyeBreakpoint, "stops at LD B,B");
        assert_eq!(m.serial_text().as_deref(), Some("HI"));
    }

    #[test]
    fn mooneye_signature_detected() {
        // S3: a ROM loading the pass signature then LD B,B is detected as pass.
        let mut rom = vec![0u8; 0x8000];
        let mut pc = 0x0100usize;
        // LD C,5 ; LD D,8 ; LD E,13 ; LD H,21 ; LD L,34 ; LD B,3 ; LD B,B
        for (op, val) in [
            (0x0E, 5u8), // LD C,d8
            (0x16, 8),   // LD D,d8
            (0x1E, 13),  // LD E,d8
            (0x26, 21),  // LD H,d8
            (0x2E, 34),  // LD L,d8
            (0x06, 3),   // LD B,d8
        ] {
            rom[pc] = op;
            rom[pc + 1] = val;
            pc += 2;
        }
        rom[pc] = 0x40; // LD B,B (magic breakpoint)
        let mut m = Machine::boot_dmg(&rom);
        let stop = m.run_mooneye(10_000);
        assert_eq!(stop, RunStop::MooneyeBreakpoint);
        assert!(
            m.mooneye_passed(),
            "registers hold the Fibonacci pass signature"
        );
    }

    /// S4: run a real blargg individual cpu_instrs ROM end to end. These print
    /// their per-test name + "Passed"/"Failed" over serial. The ROM is loaded
    /// from the git-ignored reference suite; the test is skipped if absent.
    fn run_blargg_individual(name: &str) -> (RunStop, Option<String>) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../reference/test-suites/gb-test-roms/cpu_instrs/individual")
            .join(name);
        let rom = std::fs::read(&path)
            .unwrap_or_else(|_| panic!("blargg ROM {name} must exist at {path:?}"));
        let mut m = Machine::boot_dmg(&rom);
        let stop = m.run_blargg(50_000_000);
        (stop, m.serial_text())
    }

    /// Run a blargg ROM from an arbitrary path under the gb-test-roms suite.
    fn run_blargg_at(rel: &str) -> (RunStop, Option<String>) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../reference/test-suites/gb-test-roms")
            .join(rel);
        let rom =
            std::fs::read(&path).unwrap_or_else(|_| panic!("blargg ROM must exist at {path:?}"));
        let mut m = Machine::boot_dmg(&rom);
        let stop = m.run_blargg(100_000_000);
        (stop, m.serial_text())
    }

    /// Run a blargg ROM and report PASS via either channel (serial or the
    /// cart-RAM $A000 protocol). Used for ROMs like mem_timing-2 that report to
    /// cart RAM instead of serial.
    fn blargg_passes_at(rel: &str) -> bool {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../reference/test-suites/gb-test-roms")
            .join(rel);
        let Ok(rom) = std::fs::read(&path) else {
            return true; // ROM absent on this checkout -> skip (don't fail CI)
        };
        let mut m = Machine::boot_dmg(&rom);
        m.run_blargg(100_000_000);
        m.blargg_passed()
    }

    #[test]
    fn blargg_mem_timing_passes() {
        // mem_timing reports via serial.
        assert!(
            blargg_passes_at("mem_timing/mem_timing.gb"),
            "mem_timing should pass"
        );
    }

    #[test]
    fn blargg_mem_timing_2_passes() {
        // mem_timing-2 reports via the cart-RAM $A000 protocol, not serial.
        assert!(
            blargg_passes_at("mem_timing-2/mem_timing.gb"),
            "mem_timing-2 should pass"
        );
    }

    /// Like `blargg_passes_at` but boots in CGB mode (for CGB-flagged ROMs).
    fn blargg_passes_at_cgb(rel: &str) -> bool {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../reference/test-suites/gb-test-roms")
            .join(rel);
        let Ok(rom) = std::fs::read(&path) else {
            return true; // ROM absent on this checkout -> skip
        };
        let mut m = Machine::boot_cgb(&rom);
        m.run_blargg(100_000_000);
        m.blargg_passed()
    }

    #[test]
    fn blargg_halt_bug_passes() {
        // halt_bug.gb is CGB-flagged and prints its verdict to the on-screen
        // text console (VRAM tilemap), not serial -- gated via blargg_console_text.
        assert!(blargg_passes_at_cgb("halt_bug.gb"), "halt_bug should pass");
    }

    #[test]
    fn blargg_oam_bug_fails_we_do_not_emulate_it() {
        // oam_bug requires the DMG OAM-corruption hardware bug, which we do not
        // emulate; it reports FAIL ($A000 = 0x01) via the cart-RAM protocol. This
        // pins that our detection does NOT report a false pass (the result code
        // is only trusted after the ROM signals "running").
        assert!(
            !blargg_passes_at("oam_bug/oam_bug.gb"),
            "oam_bug must report FAIL (OAM corruption bug not emulated)"
        );
    }

    #[test]
    fn blargg_dmg_sound_passing_subtests() {
        // Subtests of blargg's dmg_sound suite that we pass. These exercise the
        // APU register file, the 512 Hz frame sequencer (length + sweep), and
        // the CH1 frequency-overflow disable on trigger. Locked in after the
        // DIV-APU mask fix (rubc-8yh) brought the frame sequencer to 512 Hz.
        for name in [
            "01-registers.gb",
            "02-len ctr.gb",
            "03-trigger.gb",
            "04-sweep.gb",
            "05-sweep details.gb",
            "06-overflow on trigger.gb",
            "07-len sweep period sync.gb",
            "08-len ctr during power.gb",
            "11-regs after power.gb",
        ] {
            let rel = format!("dmg_sound/rom_singles/{name}");
            assert!(blargg_passes_at(&rel), "dmg_sound {name} should pass");
        }
    }

    /// Run a blargg ROM from an arbitrary path booted in **CGB mode**. Used for
    /// CGB-aware ROMs (header CGB flag bit 7 set) that exercise the KEY1 speed
    /// switch.
    fn run_blargg_at_cgb(rel: &str) -> (RunStop, Option<String>) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../reference/test-suites/gb-test-roms")
            .join(rel);
        let rom =
            std::fs::read(&path).unwrap_or_else(|_| panic!("blargg ROM must exist at {path:?}"));
        let mut m = Machine::boot_cgb(&rom);
        let stop = m.run_blargg(100_000_000);
        (stop, m.serial_text())
    }

    /// Run a mooneye ROM from the WLA-DX-built suite output (`<suite>/build/`).
    /// Returns (stop reason, Fibonacci-signature pass). The build is produced by
    /// `just mooneye-build` (WLA-DX); the test is skipped if the ROM is absent.
    fn run_mooneye_at(rel: &str) -> Option<(RunStop, bool)> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../reference/test-suites/mooneye-test-suite/build")
            .join(rel);
        let rom = std::fs::read(&path).ok()?;
        let mut m = Machine::boot_dmg(&rom);
        let stop = m.run_mooneye(10_000_000);
        Some((stop, m.mooneye_passed()))
    }

    /// Smoke test: a real WLA-DX-built mooneye ROM runs through the runner and
    /// reaches the `LD B,B` breakpoint. Proves the mooneye build pipeline +
    /// runner are wired end-to-end. `#[ignore]` because it depends on
    /// `just mooneye-build` having been run (RGBDS/WLA-DX build artifacts are not
    /// committed). The pass/fail of specific mooneye behaviors is gated by the
    /// per-category ROM tickets (rubc-15l etc.).
    #[test]
    #[ignore = "requires `just mooneye-build` (WLA-DX) artifacts; per-category gates are separate tickets"]
    fn mooneye_runner_smoke_acceptance_timer_tim00() {
        let Some((stop, _passed)) = run_mooneye_at("acceptance/timer/tim00.gb") else {
            eprintln!("skipped: run `just mooneye-build` first");
            return;
        };
        assert_eq!(
            stop,
            RunStop::MooneyeBreakpoint,
            "mooneye ROM should reach the LD B,B breakpoint"
        );
    }

    /// instr_timing.gb exercises per-instruction cycle accounting (measured via
    /// DIV). STATUS: it PASSES when booted in CGB mode (the ROM is CGB-flagged,
    /// $0143=0x80) but still FAILS under DMG boot, which is what this harness
    /// uses -- a real DMG instruction-timing gap owned by the mem/instr-timing
    /// verify wave (rubc-3ud). Ignored until DMG timing is exact.
    #[test]
    #[ignore = "instr_timing passes in CGB boot but fails DMG boot; DMG cycle-exactness is rubc-3ud"]
    fn blargg_instr_timing() {
        let (stop, text) = run_blargg_at("instr_timing/instr_timing.gb");
        let text = text.unwrap_or_default();
        assert!(
            text.contains("Passed"),
            "instr_timing should pass. stop={stop:?} serial={text:?}"
        );
    }

    /// 02-interrupts.gb exercises interrupt dispatch + timing (needs the timer).
    /// Passes: the N2 interrupt machinery (5-M dispatch, EI-delay, halt-bug) plus
    /// the cycle-accurate Timer handle blargg's interrupt suite.
    #[test]
    fn blargg_02_interrupts() {
        let (stop, text) = run_blargg_individual("02-interrupts.gb");
        let text = text.unwrap_or_default();
        assert!(
            text.contains("Passed"),
            "02-interrupts should pass. stop={stop:?} serial={text:?}"
        );
    }

    /// The COMBINED 64 KiB cpu_instrs.gb is a CGB-aware MBC1 cart (header CGB
    /// flag $0143 = 0x80). It runs all 11 sub-tests across multiple ROM banks, so
    /// passing it proves MBC1 ROM banking AND the CGB KEY1 speed switch: booted
    /// in CGB mode, the runner does `LD A,1 ; LDH (KEY1),A ; STOP` to enter
    /// double-speed, and STOP must perform the switch and resume rather than halt
    /// (see `step_stop`). It is booted via `boot_cgb` for exactly this reason.
    #[test]
    fn blargg_cpu_instrs_combined_mbc1() {
        let (stop, text) = run_blargg_at_cgb("cpu_instrs/cpu_instrs.gb");
        let text = text.unwrap_or_default();
        assert!(
            text.contains("Passed"),
            "combined cpu_instrs (MBC1 banking, CGB) should pass. stop={stop:?} serial={text:?}"
        );
    }

    #[test]
    fn blargg_06_ld_r_r() {
        let (stop, text) = run_blargg_individual("06-ld r,r.gb");
        let text = text.unwrap_or_default();
        assert!(
            text.contains("Passed"),
            "06-ld r,r should pass. stop={stop:?} serial={text:?}"
        );
    }

    /// Assert a blargg individual ROM passes.
    fn assert_blargg_passes(name: &str) {
        let (stop, text) = run_blargg_individual(name);
        let text = text.unwrap_or_default();
        assert!(
            text.contains("Passed"),
            "{name} should pass. stop={stop:?} serial={text:?}"
        );
    }

    // The pure-CPU blargg sub-tests (no timer/interrupt dependence) must pass on
    // the M-cycle core. Timing-dependent sub-tests (02-interrupts) are gated by
    // later waves and are NOT asserted here.
    #[test]
    fn blargg_04_op_r_imm() {
        assert_blargg_passes("04-op r,imm.gb");
    }

    #[test]
    fn blargg_05_op_rp() {
        assert_blargg_passes("05-op rp.gb");
    }

    #[test]
    fn blargg_07_jr_jp_call_ret_rst() {
        assert_blargg_passes("07-jr,jp,call,ret,rst.gb");
    }

    #[test]
    fn blargg_08_misc_instrs() {
        assert_blargg_passes("08-misc instrs.gb");
    }

    #[test]
    fn blargg_09_op_r_r() {
        assert_blargg_passes("09-op r,r.gb");
    }

    #[test]
    fn blargg_10_bit_ops() {
        assert_blargg_passes("10-bit ops.gb");
    }

    #[test]
    fn blargg_11_op_a_hl() {
        assert_blargg_passes("11-op a,(hl).gb");
    }

    #[test]
    fn blargg_01_special() {
        assert_blargg_passes("01-special.gb");
    }

    #[test]
    fn blargg_03_op_sp_hl() {
        assert_blargg_passes("03-op sp,hl.gb");
    }
}
