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
    BlarggDone,
    Timeout,
    Stuck,
}

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
        for _ in 0..max_instructions {
            if matches!(self.cpu.mode, CpuMode::Stopped) {
                return RunStopNg::Stuck;
            }
            self.step_instruction();
            if self.bus.serial_output.contains("Passed")
                || self.bus.serial_output.contains("Failed")
            {
                return RunStopNg::BlarggDone;
            }
        }
        RunStopNg::Timeout
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
            0xFF4F => 0xFE | self.vram_bank,
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
            0xFF4F => self.vram_bank = value & 1,
            0xFF70 => self.wram_bank = (value & 0x07).max(1),
            0xFFFF => self.ie = value,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,
            _ => self.io[(addr - 0xFF00) as usize] = value,
        }
        if addr == 0xFF02 && value == 0x81 {
            self.serial_output.push(self.io[0x01] as char);
            self.io[0x02] = 0x01;
        } else if (0xFF00..=0xFF7F).contains(&addr)
            && !matches!(addr, 0xFF04..=0xFF07 | 0xFF10..=0xFF3F | 0xFF40..=0xFF4B | 0xFF4F | 0xFF70)
        {
            self.io[(addr - 0xFF00) as usize] = value;
        }
    }

    fn write_ppu_register(&mut self, addr: u16, value: u8) {
        self.io[(addr - 0xFF00) as usize] = value;
        self.ppu_public.write_register(PpuRegisterWrite {
            time: self.spine.now,
            addr,
            value,
        });
        match addr {
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

    fn tick_one_subphase(&mut self) {
        let old_cpu_t = self.spine.cpu_t;
        let old_ppu_dot = self.spine.ppu_dot;
        self.spine.step_subphase(&self.table);
        self.cpu_now.0 = self.spine.now.subphases();
        self.timer.observe_spine(&self.spine);
        self.if_ |= self.timer.take_interrupt_request();
        if self.spine.cpu_t != old_cpu_t {
            self.apu.tick_spine(self.timer.div_counter(), false);
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
    }

    fn drain_scheduled_writes(&mut self) {
        let now = self.cpu_now;
        let mut i = 0;
        while i < self.scheduled_writes.len() {
            if self.scheduled_writes[i].at <= now {
                let write = self.scheduled_writes.remove(i);
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
        self.read_latched(addr)
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
        false
    }
    fn finish_speed_switch(&mut self) {}
    fn boundary(&mut self) {
        self.if_ |= 0xE0;
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
        self.drain_scheduled_writes();
        self.read_visible(addr)
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
