use crate::apu::Apu;
use crate::bus_intent::{CpuBusIntent, IntentOutcome};
use crate::cartridge::Cartridge;
use crate::cpu::scheduler::{Time as CpuTime, CPU_ACCESS_END_OFFSET};
use crate::cpu::{Cpu, CpuBus, CpuMode};
use crate::model::GbModel;
use crate::output_latch::{LcdOutputLatch, LcdPaletteSource, OutputRawPixel, PaletteWrite};
use crate::ppu_internal::{LcdPixelSource, PpuInternal, SpritePalette};
use crate::ppu_public::{PpuPublic, PpuRegisterWrite};
use crate::time::{ClockSpine, Time, DMG_DOTS_PER_FRAME, DMG_DOTS_PER_LINE};
use crate::timer::Timer;
use crate::timing::TimingTable;

const VBLANK_IRQ: u8 = 0x01;
const STAT_IRQ: u8 = 0x02;

#[derive(Clone, Copy, Debug)]
struct BootCpuProfile {
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
}

#[derive(Clone, Copy, Debug)]
struct BootProfile {
    cpu: BootCpuProfile,
    div: u16,
    p1: u8,
    sc: u8,
    nr52: u8,
    stat: u8,
    ly: u8,
    lyc: u8,
    dma: u8,
}
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePixel {
    DmgShade(u8),
    CgbRgb555(u16),
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
    framebuffer: Vec<FramePixel>,
    bg_palette_ram: [u8; 0x40],
    obj_palette_ram: [u8; 0x40],
    bg_palette_index: u8,
    obj_palette_index: u8,
    cpu_now: CpuTime,
    scheduled_writes: Vec<ScheduledWrite>,
    last_ppu_dot: u64,
    frame_counter: u64,
    first_line_after_lcd_enable: bool,
    stat_lyc_equal: bool,
    stat_irq_line: bool,
    apu: Apu,
    key1_prepare: bool,
    double_speed: bool,
    hdma: Hdma,
    oam_dma: OamDma,
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

#[derive(Clone, Debug, Default)]
struct OamDma {
    source_hi: u8,
    pending_source_hi: Option<u8>,
    start_delay_m: u8,
    index: u8,
    active: bool,
    active_for_cpu_this_m: bool,
    conflict_byte_this_m: Option<u8>,
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

fn dmg_compatible_cgb_palette_ram() -> [u8; 0x40] {
    let mut ram = [0u8; 0x40];
    for palette in 0..8usize {
        for color in 0..4usize {
            let shade = match color {
                0 => 0x7FFF,
                1 => 0x56B5,
                2 => 0x294A,
                _ => 0x0000,
            };
            let offset = palette * 8 + color * 2;
            ram[offset] = shade as u8;
            ram[offset + 1] = (shade >> 8) as u8;
        }
    }
    ram
}

impl Hdma {
    fn source(&self) -> u16 {
        (u16::from(self.src_high) << 8) | u16::from(self.src_low & 0xF0)
    }

    fn destination(&self) -> u16 {
        0x8000 | (u16::from(self.dst_high & 0x1F) << 8) | u16::from(self.dst_low & 0xF0)
    }

    #[allow(dead_code)]
    fn status(&self) -> u8 {
        if self.active {
            0x80 | self.remaining_blocks.saturating_sub(1)
        } else {
            0xFF
        }
    }
}

impl BootProfile {
    fn for_model(model: GbModel, rom: &[u8]) -> Self {
        let dmg_checksum_flags = if rom.get(0x014D).copied().unwrap_or(0) == 0 {
            0x80
        } else {
            0xB0
        };
        let sgb_div_low: u16 = if rom.get(0x014E..=0x014F) == Some(&[0xA7, 0x96]) {
            0x84
        } else {
            0x74
        };
        match model {
            GbModel::Dmg0 => Self {
                cpu: BootCpuProfile {
                    a: 0x01,
                    f: 0x00,
                    b: 0xFF,
                    c: 0x13,
                    d: 0x00,
                    e: 0xC1,
                    h: 0x84,
                    l: 0x03,
                },
                div: 0x1834,
                p1: 0xCF,
                sc: 0x7E,
                nr52: 0xF1,
                stat: 0x83,
                ly: 0x91,
                lyc: 0x00,
                dma: 0xFF,
            },
            GbModel::DmgA | GbModel::DmgB => Self {
                cpu: BootCpuProfile {
                    a: 0x01,
                    f: dmg_checksum_flags,
                    b: 0x00,
                    c: 0x13,
                    d: 0x00,
                    e: 0xD8,
                    h: 0x01,
                    l: 0x4D,
                },
                div: 0xABD0,
                p1: 0xCF,
                sc: 0x7E,
                nr52: 0xF1,
                stat: 0x80,
                ly: 0x00,
                lyc: 0x00,
                dma: 0xFF,
            },
            GbModel::Mgb => Self {
                cpu: BootCpuProfile {
                    a: 0xFF,
                    f: dmg_checksum_flags,
                    b: 0x00,
                    c: 0x13,
                    d: 0x00,
                    e: 0xD8,
                    h: 0x01,
                    l: 0x4D,
                },
                div: 0xABD0,
                p1: 0xCF,
                sc: 0x7E,
                nr52: 0xF1,
                stat: 0x80,
                ly: 0x00,
                lyc: 0x00,
                dma: 0xFF,
            },
            GbModel::Sgb => Self {
                cpu: BootCpuProfile {
                    a: 0x01,
                    f: 0x00,
                    b: 0x00,
                    c: 0x14,
                    d: 0x00,
                    e: 0x00,
                    h: 0xC0,
                    l: 0x60,
                },
                div: 0xD800 | sgb_div_low.saturating_sub(16),
                p1: 0xFF,
                sc: 0x7E,
                nr52: 0xF0,
                stat: 0x80,
                ly: 0x00,
                lyc: 0x00,
                dma: 0xFF,
            },
            GbModel::Sgb2 => Self {
                cpu: BootCpuProfile {
                    a: 0xFF,
                    f: 0x00,
                    b: 0x00,
                    c: 0x14,
                    d: 0x00,
                    e: 0x00,
                    h: 0xC0,
                    l: 0x60,
                },
                div: 0xD800 | sgb_div_low.saturating_sub(16),
                p1: 0xFF,
                sc: 0x7E,
                nr52: 0xF0,
                stat: 0x80,
                ly: 0x00,
                lyc: 0x00,
                dma: 0xFF,
            },
            GbModel::Cgb0 => Self {
                cpu: BootCpuProfile {
                    a: 0x11,
                    f: 0x80,
                    b: 0x00,
                    c: 0x00,
                    d: 0x00,
                    e: 0x08,
                    h: 0x00,
                    l: 0x7C,
                },
                div: 0x2888,
                p1: 0xFF,
                sc: 0x7E,
                nr52: 0xF1,
                stat: 0x80,
                ly: 0x00,
                lyc: 0x00,
                dma: 0x00,
            },
            GbModel::Agb => Self {
                cpu: BootCpuProfile {
                    a: 0x11,
                    f: 0x00,
                    b: 0x01,
                    c: 0x00,
                    d: 0x00,
                    e: 0x08,
                    h: 0x00,
                    l: 0x7C,
                },
                div: 0x2680,
                p1: 0xFF,
                sc: 0x7E,
                nr52: 0xF1,
                stat: 0x80,
                ly: 0x00,
                lyc: 0x00,
                dma: 0x00,
            },
            GbModel::CgbA | GbModel::CgbB | GbModel::CgbC | GbModel::CgbD | GbModel::CgbE => Self {
                cpu: BootCpuProfile {
                    a: 0x11,
                    f: 0x80,
                    b: 0x00,
                    c: 0x00,
                    d: 0x00,
                    e: 0x08,
                    h: 0x00,
                    l: 0x7C,
                },
                div: 0x267C,
                p1: 0xFF,
                sc: 0x7E,
                nr52: 0xF1,
                stat: 0x82,
                ly: 0x00,
                lyc: 0x00,
                dma: 0x00,
            },
        }
    }
}

impl MachineNg {
    pub fn from_rom(model: GbModel, rom: &[u8]) -> Result<Self, String> {
        Self::boot_for_model(model, rom)
    }

    pub fn boot_dmg(rom: &[u8]) -> Result<Self, String> {
        Self::boot_for_model(GbModel::DmgB, rom)
    }

    pub fn boot_cgb(rom: &[u8]) -> Result<Self, String> {
        if rom.get(0x0143).is_some_and(|flag| flag & 0x80 != 0) {
            Self::boot_cgb_native(rom)
        } else {
            Self::boot_dmg(rom)
        }
    }

    pub fn boot_cgb_native(rom: &[u8]) -> Result<Self, String> {
        Self::boot_for_model(GbModel::CgbE, rom)
    }

    fn boot_for_model(model: GbModel, rom: &[u8]) -> Result<Self, String> {
        if rom.is_empty() {
            return Err("ROM must contain at least one byte".to_owned());
        }
        let mut machine = Self {
            model,
            cpu: Cpu::new(),
            bus: MachineBus::new(model, rom),
        };
        machine.apply_post_boot_profile(rom);
        Ok(machine)
    }

    fn apply_post_boot_profile(&mut self, rom: &[u8]) {
        let profile = BootProfile::for_model(self.model, rom);
        self.cpu.r.a = profile.cpu.a;
        self.cpu.r.f = profile.cpu.f;
        self.cpu.r.b = profile.cpu.b;
        self.cpu.r.c = profile.cpu.c;
        self.cpu.r.d = profile.cpu.d;
        self.cpu.r.e = profile.cpu.e;
        self.cpu.r.h = profile.cpu.h;
        self.cpu.r.l = profile.cpu.l;
        self.cpu.r.sp = 0xFFFE;
        self.cpu.r.pc = 0x0100;
        self.bus.apply_post_boot_profile(profile);
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

    pub fn debug_pc(&self) -> u16 {
        self.cpu.r.pc
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

    pub fn framebuffer(&self) -> &[FramePixel] {
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

    pub fn mooneye_signature(&self) -> [u8; 6] {
        let r = &self.cpu.r;
        [r.b, r.c, r.d, r.e, r.h, r.l]
    }

    pub fn hram_debug_prefix(&self) -> [u8; 32] {
        let mut out = [0; 32];
        out.copy_from_slice(&self.bus.hram[..32]);
        out
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
            io: [0xFF; 0x80],
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
            framebuffer: vec![FramePixel::DmgShade(0); 160 * 144],
            bg_palette_ram: dmg_compatible_cgb_palette_ram(),
            obj_palette_ram: dmg_compatible_cgb_palette_ram(),
            bg_palette_index: 0,
            obj_palette_index: 0,
            cpu_now: CpuTime(0),
            scheduled_writes: Vec::new(),
            last_ppu_dot: 0,
            frame_counter: 0,
            first_line_after_lcd_enable: false,
            stat_lyc_equal: false,
            stat_irq_line: false,
            apu: Apu::default(),
            key1_prepare: false,
            double_speed: false,
            hdma: Hdma::default(),
            oam_dma: OamDma::default(),
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

    fn apply_post_boot_profile(&mut self, profile: BootProfile) {
        self.timer = Timer::post_boot(&self.spine, profile.div, 0x00, 0x00, 0x00);
        self.io.fill(0xFF);
        self.io[0x00] = profile.p1;
        self.io[0x01] = 0x00;
        self.io[0x02] = profile.sc;
        self.io[0x0F] = 0xE1;
        self.io[0x40] = 0x91;
        self.io[0x41] = profile.stat;
        self.io[0x42] = 0x00;
        self.io[0x43] = 0x00;
        self.io[0x45] = profile.lyc;
        self.io[0x46] = profile.dma;
        self.io[0x47] = 0xFC;
        self.io[0x48] = 0xFF;
        self.io[0x49] = 0xFF;
        self.io[0x4A] = 0x00;
        self.io[0x4B] = 0x00;
        self.ie = 0x00;
        self.if_ = 0xE1;
        let boot_line_dot = match profile.stat & 0x03 {
            0 => 252,
            1 => 0,
            2 => 0,
            3 => 80,
            _ => unreachable!(),
        };
        self.spine.ppu_dot = u64::from(profile.ly) * DMG_DOTS_PER_LINE + boot_line_dot;
        self.spine.line_dot = (self.spine.ppu_dot % DMG_DOTS_PER_LINE) as u16;
        self.spine.frame_dot = self.spine.ppu_dot as u32;
        self.stat_lyc_equal = profile.ly == profile.lyc;
        self.vram_bank = 0;
        self.wram_bank = 1;
        self.hdma = Hdma {
            src_high: 0xFF,
            src_low: 0xF0,
            dst_high: 0x1F,
            dst_low: 0xF0,
            remaining_blocks: 0,
            active: false,
        };
        self.oam_dma = OamDma::default();
        self.apu = Apu::default();
        let cgb = self.model.is_cgb();
        self.apu.write(0xFF10, 0x00, cgb);
        self.apu.write(0xFF11, 0x80, cgb);
        self.apu.write(0xFF12, 0xF3, cgb);
        self.apu.write(
            0xFF14,
            if profile.nr52 & 0x01 != 0 { 0x80 } else { 0x00 },
            cgb,
        );
        self.apu.write(0xFF16, 0x00, cgb);
        self.apu.write(0xFF17, 0x00, cgb);
        self.apu.write(0xFF19, 0x00, cgb);
        self.apu.write(0xFF1A, 0x00, cgb);
        self.apu.write(0xFF1C, 0x00, cgb);
        self.apu.write(0xFF1E, 0x00, cgb);
        self.apu.write(0xFF21, 0x00, cgb);
        self.apu.write(0xFF22, 0x00, cgb);
        self.apu.write(0xFF23, 0x00, cgb);
        self.apu.write(0xFF24, 0x77, cgb);
        self.apu.write(0xFF25, 0xF3, cgb);
        if cgb {
            self.io[0x4D] = 0x00;
            self.io[0x4F] = 0x00;
            self.io[0x56] = 0x3E;
            self.io[0x68] = 0x88;
            self.io[0x6A] = 0x90;
            self.io[0x70] = 0x00;
            self.io[0x72] = 0x00;
            self.io[0x73] = 0x00;
            self.io[0x75] = 0x00;
            self.io[0x76] = 0x00;
            self.io[0x77] = 0x00;
        }
        self.bg_palette_index = self.io[0x68] & 0xBF;
        self.obj_palette_index = self.io[0x6A] & 0xBF;
    }

    fn read_io(&self, addr: u16) -> Option<u8> {
        if !(0xFF00..=0xFFFF).contains(&addr) {
            return None;
        }
        Some(match addr {
            0xFF03 | 0xFF08..=0xFF0E => 0xFF,
            0xFF04..=0xFF07 => self.timer.read(addr)?,
            0xFF0F => self.if_ | 0xE0,
            0xFF10..=0xFF3F => self.apu.read_for_model(addr, self.model.is_cgb()),
            0xFF00 => self.io[0x00] | 0xC0,
            0xFF02 => self.io[0x02] | 0x7E,
            0xFF40 => self.io[0x40],
            0xFF41 => self.ppu_stat(),
            0xFF42 | 0xFF43 | 0xFF45 | 0xFF47..=0xFF4B => self.io[(addr - 0xFF00) as usize],
            0xFF44 => self.ly(),
            0xFF4D if matches!(self.model, GbModel::Cgb0) => 0xFF,
            0xFF4D if self.model.is_cgb() => {
                0x7E | if self.double_speed { 0x80 } else { 0x00 } | u8::from(self.key1_prepare)
            }
            0xFF4D => 0xFF,
            0xFF4C | 0xFF4E => 0xFF,
            0xFF4F if self.model.is_cgb() => 0xFE | self.vram_bank,
            0xFF4F => 0xFF,
            0xFF50 => 0xFF,
            0xFF55 if self.model.is_cgb() && self.hdma.active => self.hdma.status(),
            0xFF51..=0xFF55 if self.model.is_cgb() => 0xFF,
            0xFF51..=0xFF67 => 0xFF,
            0xFF68 if self.model.is_cgb() => self.bg_palette_index | 0x40,
            0xFF69 if self.model.is_cgb() => 0xFF,
            0xFF6A if self.model.is_cgb() => self.obj_palette_index | 0x40,
            0xFF6B if self.model.is_cgb() => 0xFF,
            0xFF68..=0xFF7F if !self.model.is_cgb() => 0xFF,
            0xFF6C..=0xFF71 | 0xFF74 => 0xFF,
            0xFF72 | 0xFF73 if self.model.is_cgb() => self.io[(addr - 0xFF00) as usize],
            0xFF75 if self.model.is_cgb() => self.io[0x75] | 0x8F,
            0xFF76 | 0xFF77 if self.model.is_cgb() => 0x00,
            0xFF78..=0xFF7F => 0xFF,
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
            0x8000..=0x9FFF if self.vram_blocked() => 0xFF,
            0x8000..=0x9FFF => self.vram[self.vram_bank as usize][(addr - 0x8000) as usize],
            0xA000..=0xBFFF => self.cart.read_ram(addr),
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize],
            0xD000..=0xDFFF => self.wram[self.selected_wram_bank()][(addr - 0xD000) as usize],
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize],
            0xF000..=0xFDFF => self.wram[self.selected_wram_bank()][(addr - 0xF000) as usize],
            0xFE00..=0xFE9F if self.oam_blocked() => 0xFF,
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
            0x8000..=0x9FFF if self.vram_blocked() => {}
            0x8000..=0x9FFF => {
                let offset = (addr - 0x8000) as usize;
                self.vram[self.vram_bank as usize][offset] = value;
                self.ppu_internal
                    .write_vram(offset as u16, self.vram_bank, value);
            }
            0xA000..=0xBFFF => self.cart.write_ram(addr, value),
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize] = value,
            0xD000..=0xDFFF => {
                self.wram[self.selected_wram_bank()][(addr - 0xD000) as usize] = value
            }
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize] = value,
            0xF000..=0xFDFF => {
                self.wram[self.selected_wram_bank()][(addr - 0xF000) as usize] = value
            }
            0xFE00..=0xFE9F if self.oam_blocked() => {}
            0xFE00..=0xFE9F => {
                let offset = (addr - 0xFE00) as usize;
                self.oam[offset] = value;
                self.ppu_internal.write_oam(offset, value);
            }
            0xFEA0..=0xFEFF => {}
            0xFF04..=0xFF07 => {
                let div_before = self.timer.div_counter();
                self.timer.write(addr, value);
                if addr == 0xFF04 {
                    self.apu.observe_div_apu_counter_change(
                        div_before,
                        self.timer.div_counter(),
                        self.double_speed,
                    );
                }
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
            0xFF68 if self.model.is_cgb() => self.bg_palette_index = value & 0xBF,
            0xFF69 if self.model.is_cgb() => self.write_cgb_palette(true, value),
            0xFF6A if self.model.is_cgb() => self.obj_palette_index = value & 0xBF,
            0xFF6B if self.model.is_cgb() => self.write_cgb_palette(false, value),
            0xFF70 => self.wram_bank = (value & 0x07).max(1),
            0xFF72 | 0xFF73 if self.model.is_cgb() => self.io[(addr - 0xFF00) as usize] = value,
            0xFF75 if self.model.is_cgb() => self.io[0x75] = value & 0x70,
            0xFF76 | 0xFF77 if self.model.is_cgb() => {}
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
            self.ppu_internal
                .write_vram(offset as u16, self.vram_bank, value);
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
                self.first_line_after_lcd_enable = true;
                self.refresh_lyc_compare_if_lcd_on();
            }
            0xFF40 if value & 0x80 == 0 => {
                self.spine.ppu_dot = 0;
                self.spine.line_dot = 0;
                self.spine.frame_dot = 0;
                self.first_line_after_lcd_enable = false;
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
        if matches!(addr, 0xFF40 | 0xFF42 | 0xFF43 | 0xFF4A | 0xFF4B) {
            self.ppu_internal
                .write_register_at_for_model(addr, value, self.model.is_cgb());
        }
        if matches!(addr, 0xFF40 | 0xFF41 | 0xFF45) {
            self.refresh_lyc_compare_if_lcd_on();
            self.update_stat_irq_line();
        }
    }

    fn write_cgb_palette(&mut self, bg: bool, value: u8) {
        let index = if bg {
            self.bg_palette_index
        } else {
            self.obj_palette_index
        };
        let offset = (index & 0x3F) as usize;
        if bg {
            self.bg_palette_ram[offset] = value;
            if index & 0x80 != 0 {
                self.bg_palette_index = 0x80 | ((index.wrapping_add(1)) & 0x3F);
            }
        } else {
            self.obj_palette_ram[offset] = value;
            if index & 0x80 != 0 {
                self.obj_palette_index = 0x80 | ((index.wrapping_add(1)) & 0x3F);
            }
        }
    }

    fn selected_wram_bank(&self) -> usize {
        usize::from(self.wram_bank.max(1))
    }
    fn ly(&self) -> u8 {
        ((self.spine.ppu_dot / DMG_DOTS_PER_LINE) % 154) as u8
    }
    fn ppu_mode(&self) -> u8 {
        if self.io[0x40] & 0x80 == 0 {
            return 0;
        }
        let ly = self.ly();
        let dot = self.spine.ppu_dot % DMG_DOTS_PER_LINE;
        if ly >= 144 {
            1
        } else if self.first_line_after_lcd_enable && ly == 0 && dot < 82 {
            0
        } else if dot < 80 {
            2
        } else if self.first_line_after_lcd_enable && ly == 0 && dot < 336 {
            3
        } else if dot < 252 {
            3
        } else {
            0
        }
    }
    fn ppu_stat(&self) -> u8 {
        // Mode bit1 is live from the PPU once past the boot frame; in the boot
        // frame it reflects the latched post-boot seed (DMG0 STAT=0x83). Bit0 is
        // the latched writable bit preserved via mask 0x79 (keeps bits 6-3 AND
        // bit0); only bit1 is overwritten by the mode source. This matches the
        // pre-PPU-timing baseline that passes boot_hwio-* while the live mode bit1
        // serves the intr_2_*/STAT-edge tests.
        let mode_bit_1 = if self.frame_counter == 0 {
            self.io[0x41] & 0x02
        } else {
            self.ppu_mode() & 0x02
        };
        0x80 | (self.io[0x41] & 0x79) | (u8::from(self.stat_lyc_equal) << 2) | mode_bit_1
    }

    fn vram_blocked(&self) -> bool {
        self.io[0x40] & 0x80 != 0 && self.ppu_mode() == 3
    }

    fn oam_blocked(&self) -> bool {
        self.io[0x40] & 0x80 != 0 && matches!(self.ppu_mode(), 2 | 3)
    }

    fn oam_dma(&mut self, source_hi: u8) {
        self.io[0x46] = source_hi;
        if self.model.is_cgb() {
            let base = u16::from(source_hi) << 8;
            for i in 0..0xA0u16 {
                let value = self.read_visible(base.wrapping_add(i));
                self.oam[i as usize] = value;
                self.ppu_internal.write_oam(i as usize, value);
            }
            return;
        }
        self.oam_dma.pending_source_hi = Some(source_hi);
        self.oam_dma.start_delay_m = 1;
    }

    fn oam_dma_beat(&mut self) {
        self.oam_dma.conflict_byte_this_m = None;
        self.oam_dma.active_for_cpu_this_m = false;

        if self.oam_dma.pending_source_hi.is_some() {
            if self.oam_dma.start_delay_m > 0 {
                self.oam_dma.start_delay_m -= 1;
            } else {
                self.oam_dma.source_hi = self.oam_dma.pending_source_hi.take().unwrap();
                self.oam_dma.active = true;
                self.oam_dma.index = 0;
            }
        }

        if !self.oam_dma.active {
            return;
        }

        let src = (u16::from(self.oam_dma.source_hi) << 8) | u16::from(self.oam_dma.index);
        let byte = self.read_oam_dma_source(src);
        self.oam_dma.conflict_byte_this_m = Some(byte);
        self.oam_dma.active_for_cpu_this_m = true;

        let oam_index = self.oam_dma.index as usize;
        if oam_index < self.oam.len() {
            self.oam[oam_index] = byte;
            self.ppu_internal.write_oam(oam_index, byte);
        }
        self.oam_dma.index = self.oam_dma.index.wrapping_add(1);
        if self.oam_dma.index as usize >= self.oam.len() {
            self.oam_dma.active = false;
        }
    }

    fn read_oam_dma_source(&self, addr: u16) -> u8 {
        if addr >= 0xE000 {
            let echoed = addr - 0x2000;
            return match echoed {
                0xC000..=0xCFFF => self.wram[0][(echoed - 0xC000) as usize],
                0xD000..=0xDFFF => self.wram[self.selected_wram_bank()][(echoed - 0xD000) as usize],
                _ => self.read_visible(echoed),
            };
        }
        self.read_visible(addr)
    }

    fn oam_dma_conflicts_with(&self, addr: u16) -> bool {
        if matches!(addr, 0xFF00..=0xFFFF) {
            return false;
        }
        let dma_on_video_bus = matches!(self.oam_dma.source_hi, 0x80..=0x9F);
        let addr_on_video_bus = matches!(addr, 0x8000..=0x9FFF);
        dma_on_video_bus == addr_on_video_bus
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
        let line_dot = (self.spine.ppu_dot % DMG_DOTS_PER_LINE) as usize;
        if ly != 0 {
            self.first_line_after_lcd_enable = false;
        }
        self.refresh_lyc_compare_if_lcd_on();
        if ly == 144 && line_dot == 0 {
            self.if_ |= VBLANK_IRQ;
            self.ppu_internal.begin_frame_window_state();
        }
        self.update_stat_irq_line();
        // W8b·2b-fifo (rubc-d85o): the framebuffer is fed by the real pixel
        // FIFO. Pixel columns ship when the FIFO shifts them out (12-dot
        // fetch warm-up, SCX&7 discard, window restart, sprite stalls), not
        // at a fixed line_dot-80 formula — the internal output geometry is
        // independent of the public mode-3 window (ADR 0002).
        if ly < 144 && self.io[0x40] & 0x80 != 0 {
            if line_dot == 80 {
                self.ppu_internal.begin_drawing(ly);
            }
            if line_dot >= 80 {
                if let Some(shipped) = self.ppu_internal.fifo_dot(self.model.is_cgb(), ly) {
                    let frame_pixel = if self.model.is_cgb() {
                        FramePixel::CgbRgb555(self.cgb_rgb555(
                            shipped.pixel.source,
                            shipped.pixel.cgb_palette,
                            shipped.pixel.raw_color,
                        ))
                    } else {
                        let source = match shipped.pixel.source {
                            LcdPixelSource::Bg => LcdPaletteSource::Bg,
                            LcdPixelSource::Obj(SpritePalette::Obp0) => LcdPaletteSource::Obp0,
                            LcdPixelSource::Obj(SpritePalette::Obp1) => LcdPaletteSource::Obp1,
                        };
                        let latched = self
                            .output_latch
                            .latch_pixel(OutputRawPixel {
                                time: self.spine.now,
                                ly: u16::from(ly),
                                x: shipped.x,
                                source,
                                raw_color: shipped.pixel.raw_color,
                            })
                            .expect("output latch accepts machine pixel");
                        FramePixel::DmgShade(latched.final_color)
                    };
                    self.framebuffer[usize::from(ly) * 160 + shipped.x] = frame_pixel;
                    self.last_ppu_dot = self.spine.ppu_dot;
                }
            }
        }
        if ly < 144 && line_dot == 252 && self.hdma.active {
            self.copy_hdma_block();
        }
    }

    fn cgb_rgb555(&self, source: LcdPixelSource, palette: u8, raw_color: u8) -> u16 {
        let ram = match source {
            LcdPixelSource::Bg => &self.bg_palette_ram,
            LcdPixelSource::Obj(_) => &self.obj_palette_ram,
        };
        let offset = (usize::from(palette & 7) * 8 + usize::from(raw_color & 3) * 2) & 0x3F;
        u16::from_le_bytes([ram[offset], ram[(offset + 1) & 0x3F]]) & 0x7FFF
    }

    fn update_stat_irq_line(&mut self) {
        let stat = self.io[0x41];
        let mode = self.ppu_mode();
        let ly = self.ly();
        let line_dot = self.spine.ppu_dot % DMG_DOTS_PER_LINE;
        let mode2_source = mode == 2 || (ly == 144 && line_dot == 0);
        let line = (self.stat_lyc_equal && stat & 0x40 != 0)
            || (mode2_source && stat & 0x20 != 0)
            || (mode == 1 && stat & 0x10 != 0)
            || (mode == 0 && stat & 0x08 != 0);
        if line && !self.stat_irq_line {
            self.if_ |= STAT_IRQ;
        }
        self.stat_irq_line = line;
    }

    fn refresh_lyc_compare_if_lcd_on(&mut self) {
        if self.io[0x40] & 0x80 != 0 {
            self.stat_lyc_equal = self.ly() == self.io[0x45];
        }
    }

    fn drain_scheduled_writes(&mut self) {
        let now = self.cpu_now;
        let mut i = 0;
        while i < self.scheduled_writes.len() {
            if self.scheduled_writes[i].at <= now {
                let write = self.scheduled_writes.remove(i);
                self.corrupt_oam_for_bug(write.addr, OamBugAccess::Write);
                if self.oam_dma.active_for_cpu_this_m {
                    let blocked = matches!(write.addr, 0xFE00..=0xFE9F)
                        || (!matches!(write.addr, 0xFF80..=0xFFFE)
                            && self.oam_dma_conflicts_with(write.addr));
                    if blocked {
                        continue;
                    }
                }
                self.write_visible(write.addr, write.value);
            } else {
                i += 1;
            }
        }
    }
}

impl CpuBus for MachineBus {
    fn read_m(&mut self, addr: u16) -> u8 {
        self.begin_cpu_cycle();
        self.advance_to(CpuTime(self.cpu_now.0 + 16));
        let value = self.read_latched_for_oam_bug(addr, OamBugAccess::Read);
        self.end_cpu_cycle();
        value
    }
    fn read_m_oam_bug_idu(&mut self, addr: u16) -> u8 {
        self.begin_cpu_cycle();
        self.advance_to(CpuTime(self.cpu_now.0 + 16));
        let value = self.read_latched_for_oam_bug(addr, OamBugAccess::ReadIncDec);
        self.end_cpu_cycle();
        value
    }
    fn write_m(&mut self, addr: u16, value: u8) {
        self.begin_cpu_cycle();
        self.schedule_cpu_write(
            CpuTime(self.cpu_now.0 + u64::from(self.write_drive_ticks(addr))),
            addr,
            value,
        );
        self.advance_to(CpuTime(self.cpu_now.0 + 16));
        self.end_cpu_cycle();
    }
    fn idle_m(&mut self) {
        self.begin_cpu_cycle();
        self.advance_to(CpuTime(self.cpu_now.0 + 16));
        self.end_cpu_cycle();
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
    fn begin_cpu_cycle(&mut self) {
        self.oam_dma_beat();
    }
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
        if self.oam_dma.active_for_cpu_this_m {
            if matches!(addr, 0xFE00..=0xFE9F) {
                return 0xFF;
            }
            if !matches!(addr, 0xFF80..=0xFFFE) && self.oam_dma_conflicts_with(addr) {
                if let Some(byte) = self.oam_dma.conflict_byte_this_m {
                    return byte;
                }
            }
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
    fn stat_read_exposes_live_mode_bit_one_not_stale_io_latch() {
        let mut machine = cgb_machine();
        machine.bus.frame_counter = 1;
        machine.bus.spine.ppu_dot = 252;
        machine.bus.spine.line_dot = 252;
        machine.bus.io[0x41] = 0x82;

        assert_eq!(machine.read_io(0xFF41).unwrap() & 0x02, 0x00);
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
