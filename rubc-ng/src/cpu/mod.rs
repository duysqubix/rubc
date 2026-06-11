pub mod alu;
pub mod core;
pub mod opcodes;
pub mod opcodes_cb;
pub mod regs;
pub mod scheduler;

use crate::CpuBusIntent;
use scheduler::Time;

pub use alu::Flags;
pub use core::{ActiveCpuCycle, Cpu, CpuCycleCompletion, CpuMode, CpuReg8Target, Exec};
pub use regs::Regs;

pub trait CpuBus {
    fn read_m(&mut self, addr: u16) -> u8;
    fn read_m_oam_bug_idu(&mut self, addr: u16) -> u8 {
        self.read_m(addr)
    }
    fn write_m(&mut self, addr: u16, value: u8);
    fn idle_m(&mut self);
    fn oam_bug_idu_m(&mut self, addr: u16) {
        self.idle_m();
        self.oam_bug_idu_glitch(addr);
    }
    fn irq_pending_mask(&self) -> u8;
    fn ie(&self) -> u8;
    fn clear_if_bit(&mut self, bit: u8);
    fn speed_switch_armed(&self) -> bool;
    fn finish_speed_switch(&mut self);
    fn boundary(&mut self) {}
    fn begin_cpu_cycle(&mut self) {}
    fn tick_cpu_t(&mut self) {}
    fn now(&self) -> Time {
        Time(0)
    }
    fn schedule_cpu_write(&mut self, _at: Time, addr: u16, value: u8) {
        self.write_latched(addr, value);
    }
    fn drain_cpu_writes_through(&mut self, _now: Time) {}
    fn advance_to(&mut self, _target: Time) {
        self.tick_cpu_t();
    }
    fn read_latched(&mut self, addr: u16) -> u8 {
        self.read_m(addr)
    }
    fn write_latched(&mut self, addr: u16, value: u8) {
        self.write_m(addr, value);
    }
    fn end_cpu_cycle(&mut self) {}
    fn write_drive_ticks(&self, _addr: u16) -> u8 {
        scheduler::CPU_ACCESS_END_OFFSET
    }
    fn oam_bug_idu_glitch(&mut self, _addr: u16) {}
    fn sync_ppu_to_cpu(&mut self) {}
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VectorState {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: Option<u8>,
    pub ie: Option<u8>,
    pub ei: Option<u8>,
    pub ram: Vec<(u16, u8)>,
}

impl VectorState {
    pub fn ram_addrs(&self) -> Vec<u16> {
        self.ram.iter().map(|&(addr, _)| addr).collect()
    }
}

pub trait VectorCpu {
    fn load_state(&mut self, s: &VectorState, bus: &mut FlatIntentBus);
    fn store_state(&self, bus: &FlatIntentBus, ram_addrs: &[u16]) -> VectorState;
}

#[derive(Clone)]
pub struct FlatIntentBus {
    mem: Box<[u8; 0x1_0000]>,
    ie: u8,
    if_: u8,
    now: Time,
    m_cycles: u64,
    intents: Vec<CpuBusIntent>,
}

impl Default for FlatIntentBus {
    fn default() -> Self {
        Self {
            mem: Box::new([0; 0x1_0000]),
            ie: 0,
            if_: 0,
            now: Time(0),
            m_cycles: 0,
            intents: Vec::new(),
        }
    }
}

impl FlatIntentBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn peek(&self, addr: u16) -> u8 {
        self.mem[addr as usize]
    }

    pub fn poke(&mut self, addr: u16, value: u8) {
        self.mem[addr as usize] = value;
    }

    pub fn set_ie(&mut self, ie: u8) {
        self.ie = ie;
    }

    pub fn intents(&self) -> &[CpuBusIntent] {
        &self.intents
    }
}

impl CpuBus for FlatIntentBus {
    fn read_m(&mut self, addr: u16) -> u8 {
        self.intents.push(CpuBusIntent::ReadSample { addr });
        self.m_cycles += 1;
        self.now.0 += 16;
        self.mem[addr as usize]
    }

    fn write_m(&mut self, addr: u16, value: u8) {
        self.intents.push(CpuBusIntent::WriteDrive { addr, value });
        self.m_cycles += 1;
        self.now.0 += 16;
        self.mem[addr as usize] = value;
    }

    fn idle_m(&mut self) {
        self.intents.push(CpuBusIntent::Idle);
        self.m_cycles += 1;
        self.now.0 += 16;
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

    fn boundary(&mut self) {}

    fn begin_cpu_cycle(&mut self) {}

    fn tick_cpu_t(&mut self) {
        self.now.0 += 4;
    }

    fn now(&self) -> Time {
        self.now
    }

    fn schedule_cpu_write(&mut self, _at: Time, addr: u16, value: u8) {
        self.intents.push(CpuBusIntent::WriteDrive { addr, value });
        self.mem[addr as usize] = value;
    }

    fn advance_to(&mut self, target: Time) {
        self.now = target;
    }

    fn read_latched(&mut self, addr: u16) -> u8 {
        self.intents.push(CpuBusIntent::ReadSample { addr });
        self.mem[addr as usize]
    }

    fn write_latched(&mut self, addr: u16, value: u8) {
        self.intents.push(CpuBusIntent::WriteDrive { addr, value });
        self.mem[addr as usize] = value;
    }

    fn end_cpu_cycle(&mut self) {
        self.m_cycles += 1;
    }
}

impl VectorCpu for Cpu {
    fn load_state(&mut self, s: &VectorState, bus: &mut FlatIntentBus) {
        self.r.a = s.a;
        self.r.b = s.b;
        self.r.c = s.c;
        self.r.d = s.d;
        self.r.e = s.e;
        self.r.set_f(s.f);
        self.r.h = s.h;
        self.r.l = s.l;
        self.r.sp = s.sp;
        self.r.pc = s.pc;
        self.set_ime_for_vector(s.ime.unwrap_or(0) != 0);
        self.set_ei_pending_for_vector(s.ei.unwrap_or(0) != 0);
        if let Some(ie) = s.ie {
            bus.set_ie(ie);
        }
        for &(addr, value) in &s.ram {
            bus.poke(addr, value);
        }
    }

    fn store_state(&self, bus: &FlatIntentBus, ram_addrs: &[u16]) -> VectorState {
        VectorState {
            a: self.r.a,
            b: self.r.b,
            c: self.r.c,
            d: self.r.d,
            e: self.r.e,
            f: self.r.f,
            h: self.r.h,
            l: self.r.l,
            sp: self.r.sp,
            pc: self.r.pc,
            ime: Some(self.ime as u8),
            ie: Some(bus.ie),
            ei: Some(self.ei_pending_for_vector() as u8),
            ram: ram_addrs
                .iter()
                .map(|&addr| (addr, bus.peek(addr)))
                .collect(),
        }
    }
}

impl Cpu {
    pub fn step_instruction<B: CpuBus>(&mut self, bus: &mut B) {
        loop {
            self.step_m(bus);
            if self.exec_is_boundary() && self.active_cpu_cycle().is_none() {
                break;
            }
        }
    }
}
