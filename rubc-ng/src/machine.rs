use crate::apu::Apu;
use crate::bus_intent::{CpuBusIntent, IntentOutcome};
use crate::cartridge::Cartridge;
use crate::cpu::scheduler::{Time as CpuTime, CPU_ACCESS_END_OFFSET};
use crate::cpu::{Cpu, CpuBus, CpuMode};
use crate::model::GbModel;
use crate::output_latch::{LcdOutputLatch, LcdPaletteSource, OutputRawPixel, PaletteWrite};
use crate::ppu_internal::PpuInternal;
use crate::ppu_public::{PpuPublic, PpuRegisterWrite};
use crate::time::{ClockSpine, Time, DMG_DOTS_PER_FRAME, DMG_DOTS_PER_LINE};
use crate::timer::Timer;
use crate::timing::TimingTable;

const VBLANK_IRQ: u8 = 0x01;
const STAT_IRQ: u8 = 0x02;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRecord {
    pub time: Time,
    pub cpu_t: u64,
    pub ppu_dot: u64,
    pub intent: CpuBusIntent,
    pub outcome: IntentOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStopNg {
    MooneyeBreakpoint,
    BlarggDone,
    Timeout,
    Stuck,
}

pub const MOONEYE_PASS: [u8; 6] = [3, 5, 8, 13, 21, 34];

pub struct MachineNg {
    model: GbModel,
    cpu: Cpu,
    bus: MachineBus,
}

#[derive(Clone, Debug)]
struct ScheduledWrite {
    at: CpuTime,
    addr: u16,
    value: u8,
}

#[derive(Clone, Debug)]
struct MachineBus {
    model: GbModel,
    cart: Cartridge,
    vram: [[u8; 0x2000]; 2],
    wram: [[u8; 0x1000]; 8],
    oam: [u8; 0xA0],
    hram: [u8; 0x7F],
    io: [u8; 0x80],
    ie: u8,
    if_: u8,
    vram_bank: u8,
    wram_bank: u8,
    serial_output: String,
    spine: ClockSpine,
    timer: Timer,
    table: TimingTable,
    ppu_public: PpuPublic,
    ppu_internal: PpuInternal,
    output_latch: LcdOutputLatch,
    framebuffer: Vec<u8>,
    cpu_now: CpuTime,
    scheduled_writes: Vec<ScheduledWrite>,
    last_ppu_dot: u64,
    frame_counter: u64,
    apu: Apu,
    key1_prepare: bool,
    double_speed: bool,
    hdma: Hdma,
}

#[derive(Clone, Debug, Default)]
struct Hdma {
    src_high: u8,
    src_low: u8,
    dst_high: u8,
    dst_low: u8,
    remaining_blocks: u8,
    active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OamBugAccess {
    Read,
    ReadIncDec,
    Write,
}

fn read_oam_word(oam: &[u8; 0xA0], offset: usize) -> u16 {
    u16::from(oam[offset]) | (u16::from(oam[offset + 1]) << 8)
}

fn write_oam_word(oam: &mut [u8; 0xA0], offset: usize, value: u16) {
    oam[offset] = value as u8;
    oam[offset + 1] = (value >> 8) as u8;
}

impl Hdma {
    fn source(&self) -> u16 {
        (u16::from(self.src_high) << 8) | u16::from(self.src_low & 0xF0)
    }

    fn destination(&self) -> u16 {
        0x8000 | (u16::from(self.dst_high & 0x1F) << 8) | u16::from(self.dst_low & 0xF0)
    }

    fn status(&self) -> u8 {
        if self.active {
            0x80 | self.remaining_blocks.saturating_sub(1)
        } else {
            0xFF
        }
    }
}

impl MachineNg {
    pub fn from_rom(model: GbModel, rom: &[u8]) -> Result<Self, String> {
        Self::boot_for_model(model, rom)
    }

    pub fn boot_dmg(rom: &[u8]) -> Result<Self, String> {
        let mut machine = Self::boot_for_model(GbModel::DmgB, rom)?;
        machine.cpu.r.a = 0x01;
        machine.cpu.r.f = 0xB0;
        machine.cpu.r.b = 0x00;
        machine.cpu.r.c = 0x13;
        machine.cpu.r.d = 0x00;
        machine.cpu.r.e = 0xD8;
        machine.cpu.r.h = 0x01;
        machine.cpu.r.l = 0x4D;
        machine.cpu.r.sp = 0xFFFE;
        machine.cpu.r.pc = 0x0100;
        Ok(machine)
    }

    pub fn boot_cgb(rom: &[u8]) -> Result<Self, String> {
        if rom.get(0x0143).is_some_and(|flag| flag & 0x80 != 0) {
            Self::boot_cgb_native(rom)
        } else {
            Self::boot_dmg(rom)
        }
    }

    pub fn boot_cgb_native(rom: &[u8]) -> Result<Self, String> {
        let mut machine = Self::boot_for_model(GbModel::CgbE, rom)?;
        machine.cpu.r.a = 0x11;
        machine.cpu.r.f = 0x80;
        machine.cpu.r.b = 0x00;
        machine.cpu.r.c = 0x00;
        machine.cpu.r.d = 0x00;
        machine.cpu.r.e = 0x08;
        machine.cpu.r.h = 0x00;
        machine.cpu.r.l = 0x7C;
        machine.cpu.r.sp = 0xFFFE;
        machine.cpu.r.pc = 0x0100;
        Ok(machine)
    }

    fn boot_for_model(model: GbModel, rom: &[u8]) -> Result<Self, String> {
        if rom.is_empty() {
            return Err("ROM must contain at least one byte".to_owned());
        }
        Ok(Self {
            model,
            cpu: Cpu::new(),
            bus: MachineBus::new(model, rom),
        })
    }

    pub fn model(&self) -> GbModel {
        self.model
    }

    pub fn spine(&self) -> &ClockSpine {
        &self.bus.spine
    }

    pub fn timer(&self) -> &Timer {
        &self.bus.timer
    }

    pub fn read_io(&self, addr: u16) -> Option<u8> {
        self.bus.read_io(addr)
    }

    pub fn write_io(&mut self, addr: u16, value: u8) -> bool {
        self.bus.write_io(addr, value)
    }

    pub fn serial_output(&self) -> &str {
        &self.bus.serial_output
    }

    pub fn blargg_passed(&self) -> bool {
        if let Some(status) = self.blargg_cart_ram_done() {
            return status == 0x00;
        }
        if self.bus.serial_output.contains("Passed") || self.bus.serial_output.contains("Failed") {
            return self.bus.serial_output.contains("Passed");
        }
        self.blargg_console_text()
            .is_some_and(|text| text.contains("Passed"))
    }

    pub fn blargg_cart_text(&self) -> String {
        (0..512u16)
            .map(|i| self.bus.read_visible(0xA000 + i))
            .take_while(|&b| b != 0)
            .map(|b| {
                if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect()
    }

    pub fn blargg_console_text(&self) -> Option<String> {
        let mut out = String::new();
        for row in 0..18u16 {
            let base = 0x9800u16 + row * 32;
            for col in 0..20u16 {
                let b = self.bus.read_visible(base + col);
                out.push(if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    ' '
                });
            }
            out.push('\n');
        }
        (!out.trim().is_empty()).then_some(out)
    }

    fn blargg_cart_ram_status(&self) -> Option<u8> {
        let sig = [
            self.bus.read_visible(0xA001),
            self.bus.read_visible(0xA002),
            self.bus.read_visible(0xA003),
        ];
        if sig == [0xDE, 0xB0, 0x61] {
            Some(self.bus.read_visible(0xA000))
        } else {
            None
        }
    }

    fn blargg_cart_ram_done(&self) -> Option<u8> {
        match self.blargg_cart_ram_status() {
            Some(0x80) | None => None,
            Some(status) => Some(status),
        }
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.bus.framebuffer
    }

    pub fn run_steps(&mut self, steps: usize) -> Vec<StepRecord> {
        (0..steps).map(|_| self.step()).collect()
    }

    pub fn step(&mut self) -> StepRecord {
        let before = self.bus.spine.clone();
        self.cpu.step_m(&mut self.bus);
        StepRecord {
            time: before.now,
            cpu_t: before.cpu_t,
            ppu_dot: before.ppu_dot,
            intent: CpuBusIntent::Idle,
            outcome: before.apply_cpu_intent(CpuBusIntent::Idle, &self.bus.table),
        }
    }

    pub fn step_instruction(&mut self) {
        let mut guard = 0;
        loop {
            self.cpu.step_m(&mut self.bus);
            if matches!(self.cpu.mode, CpuMode::Running) && self.cpu.exec_is_boundary() {
                break;
            }
            guard += 1;
            if guard > 96 {
                break;
            }
        }
    }

    pub fn run_blargg(&mut self, max_instructions: u64) -> RunStopNg {
        let mut cart_ram_was_running = false;
        for i in 0..max_instructions {
            if matches!(self.cpu.mode, CpuMode::Stopped) {
                return RunStopNg::Stuck;
            }
            self.step_instruction();
            if self.bus.serial_output.contains("Passed")
                || self.bus.serial_output.contains("Failed")
            {
                return RunStopNg::BlarggDone;
            }
            if self.blargg_cart_ram_status() == Some(0x80) {
                cart_ram_was_running = true;
            }
            if i % 4096 == 0 {
                if cart_ram_was_running && self.blargg_cart_ram_done().is_some() {
                    return RunStopNg::BlarggDone;
                }
                if let Some(text) = self.blargg_console_text() {
                    if text.contains("Passed") || text.contains("Failed") {
                        return RunStopNg::BlarggDone;
                    }
                }
            }
        }
        RunStopNg::Timeout
    }

    pub fn run_mooneye(&mut self, max_instructions: u64) -> RunStopNg {
        for _ in 0..max_instructions {
            if matches!(self.cpu.mode, CpuMode::Stopped) {
                return RunStopNg::Stuck;
            }
            if self.opcode_at_pc() == 0x40 {
                return RunStopNg::MooneyeBreakpoint;
            }
            self.step_instruction();
        }
        RunStopNg::Timeout
    }

    pub fn mooneye_passed(&self) -> bool {
        let r = &self.cpu.r;
        [r.b, r.c, r.d, r.e, r.h, r.l] == MOONEYE_PASS
    }

    fn opcode_at_pc(&self) -> u8 {
        self.bus.read_visible(self.cpu.r.pc)
    }

    pub fn run_frames(&mut self, frames: u64, max_instructions: u64) {
        let target = self.bus.frame_counter.saturating_add(frames);
        for _ in 0..max_instructions {
            if self.bus.frame_counter >= target {
                break;
            }
            self.step_instruction();
        }
    }
}

impl MachineBus {
    fn new(model: GbModel, rom: &[u8]) -> Self {
        let spine = ClockSpine::new();
        let table = TimingTable::for_model(model);
        let mut bus = Self {
            model,
            cart: Cartridge::from_rom(rom),
            vram: [[0; 0x2000]; 2],
            wram: [[0; 0x1000]; 8],
            oam: [0; 0xA0],
            hram: [0; 0x7F],
            io: [0; 0x80],
            ie: 0,
            if_: 0xE1,
            vram_bank: 0,
            wram_bank: 1,
            serial_output: String::new(),
            timer: Timer::power_on(&spine),
            ppu_public: PpuPublic::new(model, Time::ZERO, 0),
            ppu_internal: PpuInternal::for_test(
                0x91,
                0,
                0,
                7,
                0,
                crate::golden::Vram {
                    bank0: [0; 0x2000],
                    bank1: [0; 0x2000],
                },
                [0; 0xA0],
            ),
            output_latch: LcdOutputLatch::dmg_default(),
            framebuffer: vec![0; 160 * 144],
            cpu_now: CpuTime(0),
            scheduled_writes: Vec::new(),
            last_ppu_dot: 0,
            frame_counter: 0,
            apu: Apu::default(),
            key1_prepare: false,
            double_speed: false,
            hdma: Hdma::default(),
            spine,
            table,
        };
        bus.io[0x40] = 0x91;
        bus.io[0x41] = 0x80;
        bus.io[0x47] = 0xFC;
        bus.output_latch.apply_write(PaletteWrite {
            time: Time::ZERO,
            source: LcdPaletteSource::Bg,
            value: 0xFC,
        });
        bus
    }

    fn read_io(&self, addr: u16) -> Option<u8> {
        if !(0xFF00..=0xFFFF).contains(&addr) {
            return None;
        }
        Some(match addr {
            0xFF04..=0xFF07 => self.timer.read(addr)?,
            0xFF0F => self.if_ | 0xE0,
            0xFF10..=0xFF3F => self.apu.read_for_model(addr, self.model.is_cgb()),
            0xFF40 => self.io[0x40],
            0xFF41 => self.ppu_stat(),
            0xFF42 | 0xFF43 | 0xFF45 | 0xFF47..=0xFF4B => self.io[(addr - 0xFF00) as usize],
            0xFF44 => self.ly(),
            0xFF4D if self.model.is_cgb() => {
                0x7E | if self.double_speed { 0x80 } else { 0x00 } | u8::from(self.key1_prepare)
            }
            0xFF4D => 0xFF,
            0xFF4F => 0xFE | self.vram_bank,
            0xFF51..=0xFF55 if self.model.is_cgb() => match addr {
                0xFF51 => self.hdma.src_high,
                0xFF52 => self.hdma.src_low & 0xF0,
                0xFF53 => self.hdma.dst_high & 0x1F,
                0xFF54 => self.hdma.dst_low & 0xF0,
                0xFF55 => self.hdma.status(),
                _ => unreachable!(),
            },
            0xFF70 => 0xF8 | self.wram_bank,
            0xFFFF => self.ie,
            _ => self.io[(addr - 0xFF00) as usize],
        })
    }

    fn write_io(&mut self, addr: u16, value: u8) -> bool {
        if !(0xFF00..=0xFFFF).contains(&addr) {
            return false;
        }
        self.write_visible(addr, value);
        true
    }

    fn read_visible(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cart.read(addr),
            0x8000..=0x9FFF => self.vram[self.vram_bank as usize][(addr - 0x8000) as usize],
            0xA000..=0xBFFF => self.cart.read_ram(addr),
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize],
            0xD000..=0xDFFF => self.wram[self.selected_wram_bank()][(addr - 0xD000) as usize],
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize],
            0xF000..=0xFDFF => self.wram[self.selected_wram_bank()][(addr - 0xF000) as usize],
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0xFF,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
            0xFF00..=0xFF7F => self.read_io(addr).unwrap_or(0xFF),
        }
    }

    fn write_visible(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x7FFF => self.cart.write_reg(addr, value),
            0x8000..=0x9FFF => self.vram[self.vram_bank as usize][(addr - 0x8000) as usize] = value,
            0xA000..=0xBFFF => self.cart.write_ram(addr, value),
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize] = value,
            0xD000..=0xDFFF => {
                self.wram[self.selected_wram_bank()][(addr - 0xD000) as usize] = value
            }
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize] = value,
            0xF000..=0xFDFF => {
                self.wram[self.selected_wram_bank()][(addr - 0xF000) as usize] = value
            }
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = value,
            0xFEA0..=0xFEFF => {}
            0xFF04..=0xFF07 => {
                self.timer.write(addr, value);
            }
            0xFF0F => self.if_ = 0xE0 | (value & 0x1F),
            0xFF10..=0xFF3F => self.apu.write(addr, value, self.model.is_cgb()),
            0xFF46 => self.oam_dma(value),
            0xFF40..=0xFF4B => self.write_ppu_register(addr, value),
            0xFF4D if self.model.is_cgb() => {
                self.key1_prepare = (value & 1) != 0;
                self.io[0x4D] = value & 1;
            }
            0xFF4D => {}
            0xFF4F => self.vram_bank = value & 1,
            0xFF51..=0xFF55 if self.model.is_cgb() => self.write_hdma_register(addr, value),
            0xFF70 => self.wram_bank = (value & 0x07).max(1),
            0xFFFF => self.ie = value,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,
            _ => self.io[(addr - 0xFF00) as usize] = value,
        }
        if addr == 0xFF02 && value == 0x81 {
            self.serial_output.push(self.io[0x01] as char);
            self.io[0x02] = 0x01;
        } else if (0xFF00..=0xFF7F).contains(&addr)
            && !matches!(addr, 0xFF04..=0xFF07 | 0xFF10..=0xFF3F | 0xFF40..=0xFF55 | 0xFF70)
        {
            self.io[(addr - 0xFF00) as usize] = value;
        }
    }

    fn write_hdma_register(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF51 => self.hdma.src_high = value,
            0xFF52 => self.hdma.src_low = value & 0xF0,
            0xFF53 => self.hdma.dst_high = value & 0x1F,
            0xFF54 => self.hdma.dst_low = value & 0xF0,
            0xFF55 => self.start_or_stop_hdma(value),
            _ => unreachable!(),
        }
    }

    fn start_or_stop_hdma(&mut self, value: u8) {
        if self.hdma.active && value & 0x80 == 0 {
            self.hdma.active = false;
            return;
        }
        self.hdma.remaining_blocks = (value & 0x7F) + 1;
        self.hdma.active = value & 0x80 != 0;
        if !self.hdma.active {
            while self.hdma.remaining_blocks != 0 {
                self.copy_hdma_block();
            }
        }
    }

    fn copy_hdma_block(&mut self) {
        let src = self.hdma.source();
        let dst = self.hdma.destination();
        for i in 0..0x10u16 {
            let value = self.read_visible(src.wrapping_add(i));
            let offset = (dst - 0x8000 + i) as usize & 0x1FFF;
            self.vram[self.vram_bank as usize][offset] = value;
        }
        let next_src = src.wrapping_add(0x10);
        let next_dst = 0x8000 | ((dst - 0x8000 + 0x10) & 0x1FFF);
        self.hdma.src_high = (next_src >> 8) as u8;
        self.hdma.src_low = (next_src & 0xF0) as u8;
        self.hdma.dst_high = ((next_dst >> 8) as u8) & 0x1F;
        self.hdma.dst_low = (next_dst & 0xF0) as u8;
        self.hdma.remaining_blocks = self.hdma.remaining_blocks.saturating_sub(1);
        if self.hdma.remaining_blocks == 0 {
            self.hdma.active = false;
        }
    }

    fn write_ppu_register(&mut self, addr: u16, value: u8) {
        let old_lcdc = self.io[0x40];
        self.io[(addr - 0xFF00) as usize] = value;
        self.ppu_public.write_register(PpuRegisterWrite {
            time: self.spine.now,
            addr,
            value,
        });
        match addr {
            0xFF40 if old_lcdc & 0x80 == 0 && value & 0x80 != 0 => {
                self.spine.ppu_dot = 4;
                self.spine.line_dot = 4;
                self.spine.frame_dot = 4;
            }
            0xFF40 if value & 0x80 == 0 => {
                self.spine.ppu_dot = 0;
                self.spine.line_dot = 0;
                self.spine.frame_dot = 0;
            }
            0xFF47 => self.output_latch.apply_write(PaletteWrite {
                time: self.spine.now,
                source: LcdPaletteSource::Bg,
                value,
            }),
            0xFF48 => self.output_latch.apply_write(PaletteWrite {
                time: self.spine.now,
                source: LcdPaletteSource::Obp0,
                value,
            }),
            0xFF49 => self.output_latch.apply_write(PaletteWrite {
                time: self.spine.now,
                source: LcdPaletteSource::Obp1,
                value,
            }),
            _ => {}
        }
    }

    fn selected_wram_bank(&self) -> usize {
        usize::from(self.wram_bank.max(1))
    }
    fn ly(&self) -> u8 {
        ((self.spine.ppu_dot / DMG_DOTS_PER_LINE) % 154) as u8
    }
    fn ppu_mode(&self) -> u8 {
        let ly = self.ly();
        let dot = self.spine.ppu_dot % DMG_DOTS_PER_LINE;
        if ly >= 144 {
            1
        } else if dot < 80 {
            2
        } else if dot < 252 {
            3
        } else {
            0
        }
    }
    fn ppu_stat(&self) -> u8 {
        let lyc = u8::from(self.ly() == self.io[0x45]);
        0x80 | (self.io[0x41] & 0x78) | (lyc << 2) | self.ppu_mode()
    }

    fn oam_dma(&mut self, source_hi: u8) {
        self.io[0x46] = source_hi;
        let base = u16::from(source_hi) << 8;
        for i in 0..0xA0u16 {
            self.oam[i as usize] = self.read_visible(base.wrapping_add(i));
        }
    }

    fn oam_bug_scan_row(&self) -> Option<usize> {
        if self.model.is_cgb()
            || self.io[0x40] & 0x80 == 0
            || self.ppu_mode() != 2
            || self.ly() >= 144
        {
            return None;
        }
        let dot = self.spine.ppu_dot % DMG_DOTS_PER_LINE;
        Some((dot / 4) as usize)
    }

    fn corrupt_oam_for_bug(&mut self, addr: u16, access: OamBugAccess) {
        if !matches!(addr, 0xFE00..=0xFEFF) {
            return;
        }
        let Some(row) = self.oam_bug_scan_row() else {
            return;
        };
        self.apply_oam_bug_corruption(row, access);
    }

    fn apply_oam_bug_corruption(&mut self, row: usize, access: OamBugAccess) {
        if row >= 20 {
            return;
        }
        if access == OamBugAccess::ReadIncDec {
            self.apply_oam_bug_read_inc_dec_corruption(row);
            return;
        }
        if row == 0 {
            return;
        }
        let base = row * 8;
        let prev = base - 8;
        let a = read_oam_word(&self.oam, base);
        let b = read_oam_word(&self.oam, prev);
        let c = read_oam_word(&self.oam, prev + 4);
        let word0 = match access {
            OamBugAccess::Read => b | (a & c),
            OamBugAccess::ReadIncDec => unreachable!(),
            OamBugAccess::Write => ((a ^ c) & (b ^ c)) ^ c,
        };
        write_oam_word(&mut self.oam, base, word0);
        for word in 1..4 {
            let copied = read_oam_word(&self.oam, prev + word * 2);
            write_oam_word(&mut self.oam, base + word * 2, copied);
        }
    }

    fn apply_oam_bug_read_inc_dec_corruption(&mut self, row: usize) {
        if row == 0 {
            return;
        }
        if (4..=18).contains(&row) {
            let base = row * 8;
            let prev = base - 8;
            let prev_prev = base - 16;
            let a = read_oam_word(&self.oam, prev_prev);
            let b = read_oam_word(&self.oam, prev);
            let c = read_oam_word(&self.oam, base);
            let d = read_oam_word(&self.oam, prev + 4);
            let word0 = (b & (a | c | d)) | (a & c & d);
            write_oam_word(&mut self.oam, prev, word0);
            let mut copied_row = [0; 8];
            copied_row.copy_from_slice(&self.oam[prev..prev + 8]);
            self.oam[base..base + 8].copy_from_slice(&copied_row);
            self.oam[prev_prev..prev_prev + 8].copy_from_slice(&copied_row);
        }
        self.apply_oam_bug_corruption(row, OamBugAccess::Read);
    }

    fn tick_one_subphase(&mut self) {
        let old_cpu_t = self.spine.cpu_t;
        let old_ppu_dot = self.spine.ppu_dot;
        let ppu_divisor = if self.double_speed { 2 } else { 1 };
        self.spine
            .step_subphase_with_ppu_divisor(&self.table, ppu_divisor);
        self.cpu_now.0 = self.spine.now.subphases();
        self.timer.observe_spine(&self.spine);
        self.if_ |= self.timer.take_interrupt_request();
        if self.spine.cpu_t != old_cpu_t {
            self.apu
                .tick_spine(self.timer.div_counter(), self.double_speed);
        }
        if self.spine.ppu_dot != old_ppu_dot {
            self.on_ppu_dot();
        }
    }

    fn on_ppu_dot(&mut self) {
        if self.spine.ppu_dot.is_multiple_of(DMG_DOTS_PER_FRAME) {
            self.frame_counter += 1;
        }
        let ly = self.ly();
        let x = (self.spine.ppu_dot % DMG_DOTS_PER_LINE) as usize;
        if ly == 144 && x == 0 {
            self.if_ |= VBLANK_IRQ;
        }
        if x == 0 && (self.io[0x41] & 0x20) != 0 {
            self.if_ |= STAT_IRQ;
        }
        if ly < 144 && x < 160 {
            let tilemap_addr = 0x1800 + ((u16::from(ly) / 8) * 32 + (x as u16 / 8)) as usize;
            let raw = self.vram[0][tilemap_addr % 0x2000] & 0x03;
            let latched = self
                .output_latch
                .latch_pixel(OutputRawPixel {
                    time: self.spine.now,
                    ly: u16::from(ly),
                    x,
                    source: LcdPaletteSource::Bg,
                    raw_color: raw,
                })
                .expect("output latch accepts machine pixel");
            self.framebuffer[usize::from(ly) * 160 + x] = latched.final_color;
            self.last_ppu_dot = self.spine.ppu_dot;
            let _ = self.ppu_internal.fetch_next_tile_for_test(u16::from(ly));
        }
        if ly < 144 && x == 252 && self.hdma.active {
            self.copy_hdma_block();
        }
    }

    fn drain_scheduled_writes(&mut self) {
        let now = self.cpu_now;
        let mut i = 0;
        while i < self.scheduled_writes.len() {
            if self.scheduled_writes[i].at <= now {
                let write = self.scheduled_writes.remove(i);
                self.corrupt_oam_for_bug(write.addr, OamBugAccess::Write);
                self.write_visible(write.addr, write.value);
            } else {
                i += 1;
            }
        }
    }
}

impl CpuBus for MachineBus {
    fn read_m(&mut self, addr: u16) -> u8 {
        self.advance_to(CpuTime(self.cpu_now.0 + 16));
        self.read_latched_for_oam_bug(addr, OamBugAccess::Read)
    }
    fn read_m_oam_bug_idu(&mut self, addr: u16) -> u8 {
        self.advance_to(CpuTime(self.cpu_now.0 + 16));
        self.read_latched_for_oam_bug(addr, OamBugAccess::ReadIncDec)
    }
    fn write_m(&mut self, addr: u16, value: u8) {
        self.schedule_cpu_write(
            CpuTime(self.cpu_now.0 + u64::from(self.write_drive_ticks(addr))),
            addr,
            value,
        );
        self.advance_to(CpuTime(self.cpu_now.0 + 16));
        self.end_cpu_cycle();
    }
    fn idle_m(&mut self) {
        self.advance_to(CpuTime(self.cpu_now.0 + 16));
    }
    fn irq_pending_mask(&self) -> u8 {
        self.ie & self.if_ & 0x1F
    }
    fn ie(&self) -> u8 {
        self.ie
    }
    fn clear_if_bit(&mut self, bit: u8) {
        self.if_ &= !(1 << bit);
    }
    fn speed_switch_armed(&self) -> bool {
        self.model.is_cgb() && self.key1_prepare
    }
    fn finish_speed_switch(&mut self) {
        self.double_speed = !self.double_speed;
        self.key1_prepare = false;
        self.io[0x4D] = if self.double_speed { 0x80 } else { 0x00 };
    }
    fn boundary(&mut self) {
        self.if_ |= 0xE0;
    }
    fn oam_bug_idu_m(&mut self, addr: u16) {
        self.idle_m();
        self.corrupt_oam_for_bug(addr, OamBugAccess::Write);
    }
    fn oam_bug_idu_glitch(&mut self, addr: u16) {
        self.corrupt_oam_for_bug(addr, OamBugAccess::Write);
    }
    fn begin_cpu_cycle(&mut self) {}
    fn tick_cpu_t(&mut self) {
        self.advance_to(CpuTime(self.cpu_now.0 + 4));
    }
    fn now(&self) -> CpuTime {
        self.cpu_now
    }
    fn schedule_cpu_write(&mut self, at: CpuTime, addr: u16, value: u8) {
        self.scheduled_writes
            .push(ScheduledWrite { at, addr, value });
    }
    fn drain_cpu_writes_through(&mut self, now: CpuTime) {
        self.advance_to(now);
    }
    fn advance_to(&mut self, target: CpuTime) {
        while self.cpu_now < target {
            self.tick_one_subphase();
            self.drain_scheduled_writes();
        }
    }
    fn read_latched(&mut self, addr: u16) -> u8 {
        self.read_latched_for_oam_bug(addr, OamBugAccess::Read)
    }
    fn write_latched(&mut self, addr: u16, value: u8) {
        self.write_visible(addr, value);
    }
    fn end_cpu_cycle(&mut self) {
        self.drain_scheduled_writes();
    }
    fn write_drive_ticks(&self, _addr: u16) -> u8 {
        CPU_ACCESS_END_OFFSET
    }
    fn sync_ppu_to_cpu(&mut self) {}
}

impl MachineBus {
    fn read_latched_for_oam_bug(&mut self, addr: u16, access: OamBugAccess) -> u8 {
        self.drain_scheduled_writes();
        if matches!(addr, 0xFE00..=0xFEFF) {
            self.corrupt_oam_for_bug(addr, access);
        }
        self.read_visible(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cgb_machine() -> MachineNg {
        MachineNg::from_rom(GbModel::CgbE, &[0; 0x8000]).expect("valid CGB machine")
    }

    #[test]
    fn gdma_copies_one_or_more_16_byte_blocks_into_cgb_vram() {
        let mut machine = cgb_machine();

        for i in 0..0x20u16 {
            machine.bus.write_visible(0xC120 + i, 0x80 | i as u8);
        }

        machine.write_io(0xFF51, 0xC1);
        machine.write_io(0xFF52, 0x23);
        machine.write_io(0xFF53, 0x82);
        machine.write_io(0xFF54, 0x05);
        machine.write_io(0xFF55, 0x01);

        for i in 0..0x20u16 {
            assert_eq!(
                machine.bus.vram[0][0x0200 + i as usize],
                0x80 | i as u8,
                "GDMA must copy byte {i:#04x} from masked source to masked VRAM destination"
            );
        }
        assert_eq!(
            machine.read_io(0xFF55),
            Some(0xFF),
            "GDMA completes immediately"
        );
    }

    #[test]
    fn hblank_hdma_copies_exactly_one_16_byte_block_per_hblank() {
        let mut machine = cgb_machine();

        for i in 0..0x20u16 {
            machine.bus.write_visible(0xC200 + i, 0x40 | i as u8);
        }

        machine.write_io(0xFF51, 0xC2);
        machine.write_io(0xFF52, 0x00);
        machine.write_io(0xFF53, 0x80);
        machine.write_io(0xFF54, 0x00);
        machine.write_io(0xFF55, 0x81);

        assert_eq!(
            machine.read_io(0xFF55),
            Some(0x81),
            "HDMA remains active with two blocks queued"
        );

        while machine.bus.ppu_mode() != 0 {
            machine.step();
        }

        for i in 0..0x10u16 {
            assert_eq!(machine.bus.vram[0][i as usize], 0x40 | i as u8);
        }
        assert_eq!(
            machine.bus.vram[0][0x10], 0,
            "second HDMA block must wait for a later HBlank"
        );
        assert_eq!(
            machine.read_io(0xFF55),
            Some(0x80),
            "one block remains active"
        );

        while machine.bus.ppu_mode() == 0 {
            machine.step();
        }
        while machine.bus.ppu_mode() != 0 {
            machine.step();
        }

        for i in 0x10..0x20u16 {
            assert_eq!(machine.bus.vram[0][i as usize], 0x40 | i as u8);
        }
        assert_eq!(
            machine.read_io(0xFF55),
            Some(0xFF),
            "HDMA completes after second HBlank block"
        );
    }

    fn dmg_machine_scanning_oam_row(row: usize) -> MachineNg {
        let mut machine =
            MachineNg::from_rom(GbModel::DmgB, &[0; 0x8000]).expect("valid DMG machine");
        machine.bus.io[0x40] = 0x91;
        let dot = (row as u64) * 4;
        machine.bus.spine.ppu_dot = dot;
        machine.bus.spine.line_dot = dot as u16;
        for (i, byte) in machine.bus.oam.iter_mut().enumerate() {
            *byte = i as u8;
        }
        machine
    }

    #[test]
    fn oam_bug_read_write_patterns_match_documented_row_formula() {
        let mut read = dmg_machine_scanning_oam_row(3);
        read.bus.corrupt_oam_for_bug(0xFE20, OamBugAccess::Read);
        assert_eq!(
            &read.bus.oam[24..32],
            &[0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17],
            "OAM read during row 3 copies previous row with word0=b|(a&c)"
        );

        let mut write = dmg_machine_scanning_oam_row(3);
        write.bus.corrupt_oam_for_bug(0xFE20, OamBugAccess::Write);
        assert_eq!(
            &write.bus.oam[24..32],
            &[0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17],
            "OAM write during row 3 uses ((a^c)&(b^c))^c for word0, then copies previous row"
        );
    }

    #[test]
    fn oam_bug_inc_dec_pattern_matches_documented_three_row_formula() {
        let mut machine = dmg_machine_scanning_oam_row(4);
        machine
            .bus
            .corrupt_oam_for_bug(0xFE20, OamBugAccess::ReadIncDec);

        assert_eq!(
            &machine.bus.oam[16..24],
            &[0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F],
            "INC/DEC first corrupts previous row word0 from three-row hardware formula"
        );
        assert_eq!(
            &machine.bus.oam[24..32],
            &[0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F],
            "INC/DEC copies the corrupted previous row into current row"
        );
        assert_eq!(
            &machine.bus.oam[32..40],
            &[0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F],
            "INC/DEC then applies the normal read-copy pattern to the current scan row"
        );
    }
}
