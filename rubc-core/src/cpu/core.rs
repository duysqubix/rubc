//! The M-cycle CPU state machine.
//!
//! `Cpu::step_m(bus)` performs exactly ONE bus M-cycle. Multi-M-cycle
//! instructions advance one phase per call; zero-cycle internal transitions
//! (e.g. boundary -> fetch) loop until exactly one bus operation happens.

use crate::bus::CpuBus;

use super::regs::Regs;

/// High-level CPU mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuMode {
    Running,
    Halt,
    Stopped,
    /// Servicing an interrupt: `phase` 0..=4 over the 5-M-cycle dispatch.
    InterruptDispatch {
        phase: u8,
        bit: u8,
        vector: u16,
        cancelled: bool,
    },
}

/// Where we are within the current instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exec {
    /// Between instructions: poll interrupts / promote EI, then fetch.
    Boundary,
    /// Decoding an opcode at `phase` (0 = just fetched).
    Execute { op: u8, phase: u8 },
    /// CB-prefixed opcode at `phase`.
    CbExecute { op: u8, phase: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuReg8Target {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuCycleCompletion {
    None,
    FetchOpcode,
    ReadReg8 {
        target: CpuReg8Target,
        increment_pc: bool,
        finish: bool,
    },
    DispatchIdleToSpDec,
    DispatchSpDec,
    DispatchPushPcHigh,
    DispatchPushPcLow,
    DispatchFinish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveCpuCycle {
    Idle {
        completion: CpuCycleCompletion,
    },
    Fetch {
        addr: u16,
        completion: CpuCycleCompletion,
    },
    Read {
        addr: u16,
        completion: CpuCycleCompletion,
    },
    Write {
        addr: u16,
        value: u8,
        completion: CpuCycleCompletion,
    },
    OamBugIdu {
        addr: u16,
        completion: CpuCycleCompletion,
    },
    OamBugReadIncDec {
        addr: u16,
        completion: CpuCycleCompletion,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveCpuCycleState {
    cycle: ActiveCpuCycle,
    elapsed_t: u8,
}

struct PerTOpcodeBus<'a, B> {
    inner: &'a mut B,
}

impl<'a, B: CpuBus> PerTOpcodeBus<'a, B> {
    fn new(inner: &'a mut B) -> Self {
        Self { inner }
    }

    fn run_cycle(&mut self) {
        self.inner.begin_cpu_cycle();
        for _ in 0..4 {
            self.inner.tick_cpu_t();
        }
    }
}

impl<B: CpuBus> CpuBus for PerTOpcodeBus<'_, B> {
    fn read_m(&mut self, addr: u16) -> u8 {
        self.run_cycle();
        let value = self.inner.read_latched(addr);
        self.inner.end_cpu_cycle();
        value
    }

    fn read_m_oam_bug_idu(&mut self, addr: u16) -> u8 {
        self.inner.read_m_oam_bug_idu(addr)
    }

    fn write_m(&mut self, addr: u16, value: u8) {
        self.inner.begin_cpu_cycle();
        let drive_ticks = self.inner.write_drive_ticks(addr);
        for elapsed in 0..4 {
            if elapsed == drive_ticks {
                self.inner.write_latched(addr, value);
            }
            self.inner.tick_cpu_t();
        }
        if drive_ticks == 4 {
            self.inner.write_latched(addr, value);
        }
        self.inner.end_cpu_cycle();
    }

    fn idle_m(&mut self) {
        self.run_cycle();
        self.inner.end_cpu_cycle();
    }

    fn oam_bug_idu_m(&mut self, addr: u16) {
        self.inner.oam_bug_idu_m(addr);
    }

    fn oam_bug_idu_glitch(&mut self, addr: u16) {
        self.inner.oam_bug_idu_glitch(addr);
    }

    fn irq_pending_mask(&self) -> u8 {
        self.inner.irq_pending_mask()
    }

    fn ie(&self) -> u8 {
        self.inner.ie()
    }

    fn clear_if_bit(&mut self, bit: u8) {
        self.inner.clear_if_bit(bit);
    }

    fn speed_switch_armed(&self) -> bool {
        self.inner.speed_switch_armed()
    }

    fn finish_speed_switch(&mut self) {
        self.inner.finish_speed_switch();
    }

    fn boundary(&mut self) {
        self.inner.boundary();
    }

    fn begin_cpu_cycle(&mut self) {
        self.inner.begin_cpu_cycle();
    }

    fn tick_cpu_t(&mut self) {
        self.inner.tick_cpu_t();
    }

    fn read_latched(&mut self, addr: u16) -> u8 {
        self.inner.read_latched(addr)
    }

    fn write_latched(&mut self, addr: u16, value: u8) {
        self.inner.write_latched(addr, value);
    }

    fn end_cpu_cycle(&mut self) {
        self.inner.end_cpu_cycle();
    }
}

/// The SM83 CPU.
#[cfg_attr(test, derive(Clone))]
pub struct Cpu {
    pub r: Regs,
    pub ime: bool,
    ime_pending: bool,
    ime_delay_boundary: u8,
    pub mode: CpuMode,
    exec: Exec,
    halt_bug: bool,
    // Scratch registers for multi-phase instructions.
    tmp8: u8,
    tmp16: u16,
    active_cycle: Option<ActiveCpuCycleState>,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            r: Regs::new(),
            ime: false,
            ime_pending: false,
            ime_delay_boundary: 0,
            mode: CpuMode::Running,
            exec: Exec::Boundary,
            halt_bug: false,
            tmp8: 0,
            tmp16: 0,
            active_cycle: None,
        }
    }

    pub(crate) fn save_state(&self) -> crate::savestate::CpuState {
        crate::savestate::CpuState {
            regs: self.r,
            ime: self.ime,
            ime_pending: self.ime_pending,
            ime_delay_boundary: self.ime_delay_boundary,
            mode: self.mode,
            exec: self.exec,
            halt_bug: self.halt_bug,
            tmp8: self.tmp8,
            tmp16: self.tmp16,
            active_cycle: self.active_cycle.map(|state| crate::savestate::ActiveCpuCycleState {
                cycle: state.cycle,
                elapsed_t: state.elapsed_t,
            }),
        }
    }

    pub(crate) fn load_state(&mut self, state: crate::savestate::CpuState) {
        self.r = state.regs;
        self.ime = state.ime;
        self.ime_pending = state.ime_pending;
        self.ime_delay_boundary = state.ime_delay_boundary;
        self.mode = state.mode;
        self.exec = state.exec;
        self.halt_bug = state.halt_bug;
        self.tmp8 = state.tmp8;
        self.tmp16 = state.tmp16;
        self.active_cycle = state.active_cycle.map(|state| ActiveCpuCycleState {
            cycle: state.cycle,
            elapsed_t: state.elapsed_t,
        });
    }

    pub fn active_cpu_cycle(&self) -> Option<ActiveCpuCycle> {
        self.active_cycle.map(|state| state.cycle)
    }

    pub fn start_cpu_cycle(&mut self, cycle: ActiveCpuCycle) {
        assert!(
            self.active_cycle.is_none(),
            "cannot start a CPU cycle while another is active"
        );
        self.active_cycle = Some(ActiveCpuCycleState {
            cycle,
            elapsed_t: 0,
        });
    }

    pub fn step_t<B: CpuBus>(&mut self, bus: &mut B) -> bool {
        let Some(mut state) = self.active_cycle else {
            return false;
        };

        if state.elapsed_t == 0 {
            bus.begin_cpu_cycle();
            if let ActiveCpuCycle::Write { addr, value, .. } = state.cycle {
                if bus.write_drive_ticks(addr) == 0 {
                    bus.write_latched(addr, value);
                }
            }
        }

        bus.tick_cpu_t();
        state.elapsed_t += 1;

        if let ActiveCpuCycle::Write { addr, value, .. } = state.cycle {
            if bus.write_drive_ticks(addr) == state.elapsed_t {
                bus.write_latched(addr, value);
            }
        }

        if state.elapsed_t < 4 {
            self.active_cycle = Some(state);
            return false;
        }

        let result = match state.cycle {
            ActiveCpuCycle::Idle { .. } => 0xFF,
            ActiveCpuCycle::Fetch { addr, .. }
            | ActiveCpuCycle::Read { addr, .. }
            | ActiveCpuCycle::OamBugReadIncDec { addr, .. } => bus.read_latched(addr),
            ActiveCpuCycle::Write { .. } => 0xFF,
            ActiveCpuCycle::OamBugIdu { addr, .. } => {
                bus.oam_bug_idu_glitch(addr);
                0xFF
            }
        };

        bus.end_cpu_cycle();
        self.active_cycle = None;
        self.apply_cycle_completion(bus, state.cycle.completion(), result);
        true
    }

    #[cfg(test)]
    pub(super) fn step_b2_supported_m_via_t<B: CpuBus>(&mut self, bus: &mut B) -> bool {
        match self.mode {
            CpuMode::InterruptDispatch { .. } => {
                self.step_dispatch(bus);
                true
            }
            CpuMode::Running => match self.exec {
                Exec::Boundary => {
                    if self.ime_pending {
                        if self.ime_delay_boundary == 0 {
                            self.ime = true;
                            self.ime_pending = false;
                        } else {
                            self.ime_delay_boundary -= 1;
                        }
                    }
                    bus.boundary();
                    if self.try_dispatch_interrupt(bus) {
                        self.step_dispatch(bus);
                        return true;
                    }
                    self.start_cpu_cycle(ActiveCpuCycle::Fetch {
                        addr: self.r.pc,
                        completion: CpuCycleCompletion::FetchOpcode,
                    });
                    self.finish_active_cycle(bus);
                    true
                }
                Exec::Execute { op: 0x00, phase: 0 } => {
                    self.finish();
                    true
                }
                Exec::Execute { op: 0x06, phase: 0 } => {
                    self.next_phase(0x06, 1);
                    true
                }
                Exec::Execute { op: 0x06, phase: 1 } => {
                    self.start_cpu_cycle(ActiveCpuCycle::Read {
                        addr: self.r.pc,
                        completion: CpuCycleCompletion::ReadReg8 {
                            target: CpuReg8Target::B,
                            increment_pc: true,
                            finish: true,
                        },
                    });
                    self.finish_active_cycle(bus);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    #[cfg(test)]
    pub(super) fn step_b3_supported_m_via_t<B: CpuBus>(&mut self, bus: &mut B) -> bool {
        match self.mode {
            CpuMode::InterruptDispatch { .. } => {
                self.step_dispatch_via_t(bus);
                true
            }
            CpuMode::Running => match self.exec {
                Exec::Boundary => {
                    if self.ime_pending {
                        if self.ime_delay_boundary == 0 {
                            self.ime = true;
                            self.ime_pending = false;
                        } else {
                            self.ime_delay_boundary -= 1;
                        }
                    }
                    bus.boundary();
                    if self.try_dispatch_interrupt(bus) {
                        self.step_dispatch_via_t(bus);
                        return true;
                    }
                    self.start_cpu_cycle(ActiveCpuCycle::Fetch {
                        addr: self.r.pc,
                        completion: CpuCycleCompletion::FetchOpcode,
                    });
                    self.finish_active_cycle(bus);
                    true
                }
                Exec::Execute { op: 0x00, phase: 0 } => {
                    self.finish();
                    true
                }
                Exec::Execute { op: 0x06, phase: 0 } => {
                    self.next_phase(0x06, 1);
                    true
                }
                Exec::Execute { op: 0x06, phase: 1 } => {
                    self.start_cpu_cycle(ActiveCpuCycle::Read {
                        addr: self.r.pc,
                        completion: CpuCycleCompletion::ReadReg8 {
                            target: CpuReg8Target::B,
                            increment_pc: true,
                            finish: true,
                        },
                    });
                    self.finish_active_cycle(bus);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn step_m_via_t<B: CpuBus>(&mut self, bus: &mut B) {
        loop {
            match self.mode {
                CpuMode::Halt => {
                    bus.boundary();
                    if bus.irq_pending_mask() != 0 {
                        self.mode = CpuMode::Running;
                        self.exec = Exec::Boundary;
                        continue;
                    }
                    self.start_cpu_cycle(ActiveCpuCycle::Idle {
                        completion: CpuCycleCompletion::None,
                    });
                    self.finish_active_cycle(bus);
                    return;
                }
                CpuMode::Stopped => {
                    self.start_cpu_cycle(ActiveCpuCycle::Idle {
                        completion: CpuCycleCompletion::None,
                    });
                    self.finish_active_cycle(bus);
                    return;
                }
                CpuMode::InterruptDispatch { .. } => {
                    self.step_dispatch_via_t(bus);
                    return;
                }
                CpuMode::Running => match self.exec {
                    Exec::Boundary => {
                        if self.ime_pending {
                            if self.ime_delay_boundary == 0 {
                                self.ime = true;
                                self.ime_pending = false;
                            } else {
                                self.ime_delay_boundary -= 1;
                            }
                        }
                        bus.boundary();
                        if self.try_dispatch_interrupt(bus) {
                            continue;
                        }
                        self.start_cpu_cycle(ActiveCpuCycle::Fetch {
                            addr: self.r.pc,
                            completion: CpuCycleCompletion::FetchOpcode,
                        });
                        self.finish_active_cycle(bus);
                        return;
                    }
                    Exec::Execute { op, phase } => {
                        let mut per_t_bus = PerTOpcodeBus::new(bus);
                        super::opcodes::step(self, &mut per_t_bus, op, phase);
                        return;
                    }
                    Exec::CbExecute { op, phase } => {
                        let mut per_t_bus = PerTOpcodeBus::new(bus);
                        super::opcodes_cb::step(self, &mut per_t_bus, op, phase);
                        return;
                    }
                },
            }
        }
    }

    fn step_dispatch_via_t<B: CpuBus>(&mut self, bus: &mut B) {
        let CpuMode::InterruptDispatch { phase, .. } = self.mode else {
            unreachable!();
        };

        match phase {
            0 => self.start_cpu_cycle(ActiveCpuCycle::Idle {
                completion: CpuCycleCompletion::DispatchIdleToSpDec,
            }),
            1 => self.start_cpu_cycle(ActiveCpuCycle::OamBugIdu {
                addr: self.r.sp,
                completion: CpuCycleCompletion::DispatchSpDec,
            }),
            2 => self.start_cpu_cycle(ActiveCpuCycle::Write {
                addr: self.r.sp,
                value: (self.r.pc >> 8) as u8,
                completion: CpuCycleCompletion::DispatchPushPcHigh,
            }),
            3 => self.start_cpu_cycle(ActiveCpuCycle::Write {
                addr: self.r.sp,
                value: self.r.pc as u8,
                completion: CpuCycleCompletion::DispatchPushPcLow,
            }),
            _ => self.start_cpu_cycle(ActiveCpuCycle::Idle {
                completion: CpuCycleCompletion::DispatchFinish,
            }),
        }

        self.finish_active_cycle(bus);
    }

    fn finish_active_cycle<B: CpuBus>(&mut self, bus: &mut B) {
        for t in 0..4 {
            let done = self.step_t(bus);
            assert_eq!(done, t == 3);
        }
    }

    /// Advance exactly one bus M-cycle.
    pub fn step_m<B: CpuBus>(&mut self, bus: &mut B) {
        self.step_m_via_t(bus);
    }

    #[cfg(test)]
    pub(super) fn step_m_legacy<B: CpuBus>(&mut self, bus: &mut B) {
        loop {
            match self.mode {
                CpuMode::Halt => {
                    bus.boundary();
                    if bus.irq_pending_mask() != 0 {
                        self.mode = CpuMode::Running;
                        self.exec = Exec::Boundary;
                        continue;
                    }
                    bus.idle_m();
                    return;
                }
                CpuMode::Stopped => {
                    bus.idle_m();
                    return;
                }
                CpuMode::InterruptDispatch { .. } => {
                    self.step_dispatch(bus);
                    return;
                }
                CpuMode::Running => match self.exec {
                    Exec::Boundary => {
                        if self.ime_pending {
                            if self.ime_delay_boundary == 0 {
                                self.ime = true;
                                self.ime_pending = false;
                            } else {
                                self.ime_delay_boundary -= 1;
                            }
                        }
                        bus.boundary();
                        if self.try_dispatch_interrupt(bus) {
                            continue;
                        }
                        // Fetch the opcode (1 M-cycle), then decode next calls.
                        let op = bus.read_m(self.r.pc);
                        if self.halt_bug {
                            self.halt_bug = false;
                        } else {
                            self.r.pc = self.r.pc.wrapping_add(1);
                        }
                        self.exec = Exec::Execute { op, phase: 0 };
                        return;
                    }
                    Exec::Execute { op, phase } => {
                        super::opcodes::step(self, bus, op, phase);
                        return;
                    }
                    Exec::CbExecute { op, phase } => {
                        super::opcodes_cb::step(self, bus, op, phase);
                        return;
                    }
                },
            }
        }
    }

    /// End the current instruction; next `step_m` polls at the boundary.
    pub(super) fn finish(&mut self) {
        self.exec = Exec::Boundary;
    }

    /// True if the CPU is at an instruction boundary (ready to fetch the next
    /// opcode). Used by the machine runner to step exactly one instruction.
    pub fn exec_is_boundary(&self) -> bool {
        self.exec == Exec::Boundary
    }

    #[cfg(test)]
    pub(super) fn equiv_exec(&self) -> Exec {
        self.exec
    }

    #[cfg(test)]
    pub(super) fn equiv_ime_pending(&self) -> bool {
        self.ime_pending
    }

    #[cfg(test)]
    pub(super) fn equiv_ime_delay_boundary(&self) -> u8 {
        self.ime_delay_boundary
    }

    #[cfg(test)]
    pub(super) fn equiv_halt_bug(&self) -> bool {
        self.halt_bug
    }

    #[cfg(test)]
    pub(super) fn equiv_tmp8(&self) -> u8 {
        self.tmp8
    }

    #[cfg(test)]
    pub(super) fn equiv_tmp16(&self) -> u16 {
        self.tmp16
    }

    /// Advance to the next phase of the current (non-CB) opcode.
    pub(super) fn next_phase(&mut self, op: u8, phase: u8) {
        self.exec = Exec::Execute { op, phase };
    }

    pub(super) fn next_cb_phase(&mut self, op: u8, phase: u8) {
        self.exec = Exec::CbExecute { op, phase };
    }

    /// Enter the CB-prefix decode after the 0xCB fetch.
    pub(super) fn begin_cb(&mut self, op: u8) {
        self.exec = Exec::CbExecute { op, phase: 0 };
    }

    pub(super) fn set_tmp8(&mut self, v: u8) {
        self.tmp8 = v;
    }
    pub(super) fn tmp8(&self) -> u8 {
        self.tmp8
    }
    pub(super) fn set_tmp16(&mut self, v: u16) {
        self.tmp16 = v;
    }
    pub(super) fn tmp16(&self) -> u16 {
        self.tmp16
    }

    /// EI: enable interrupts after the next instruction.
    pub(super) fn schedule_ei(&mut self) {
        if !self.ime_pending {
            self.ime_pending = true;
            self.ime_delay_boundary = 1;
        }
    }
    /// DI: disable interrupts immediately.
    pub(super) fn di(&mut self) {
        self.ime = false;
        self.ime_pending = false;
        self.ime_delay_boundary = 0;
    }
    /// RETI / interrupt-return enabling.
    pub(super) fn set_ime_now(&mut self, on: bool) {
        self.ime = on;
        self.ime_pending = false;
        self.ime_delay_boundary = 0;
    }

    pub(super) fn enter_halt<B: CpuBus>(&mut self, bus: &B) {
        if !self.ime && bus.irq_pending_mask() != 0 {
            // HALT bug: PC fails to increment on the next fetch.
            self.halt_bug = true;
            self.finish();
        } else {
            self.mode = CpuMode::Halt;
            self.finish();
        }
    }

    pub(super) fn enter_stop(&mut self) {
        self.mode = CpuMode::Stopped;
        self.finish();
    }

    fn try_dispatch_interrupt<B: CpuBus>(&mut self, bus: &B) -> bool {
        if !self.ime {
            return false;
        }
        let pending = bus.irq_pending_mask();
        if pending == 0 {
            return false;
        }
        let bit = pending.trailing_zeros() as u8;
        let vector = 0x40 + (bit as u16) * 8;
        self.ime = false;
        self.ime_pending = false;
        self.ime_delay_boundary = 0;
        self.mode = CpuMode::InterruptDispatch {
            phase: 0,
            bit,
            vector,
            cancelled: false,
        };
        true
    }

    #[cfg(test)]
    fn step_dispatch<B: CpuBus>(&mut self, bus: &mut B) {
        let CpuMode::InterruptDispatch {
            phase,
            bit,
            vector,
            cancelled,
        } = &mut self.mode
        else {
            unreachable!();
        };
        match *phase {
            0 => {
                bus.idle_m();
                *phase += 1;
            }
            1 => {
                let old_sp = self.r.sp;
                bus.oam_bug_idu_m(old_sp);
                self.r.sp = old_sp.wrapping_sub(1);
                *phase = 2;
            }
            2 => {
                bus.write_m(self.r.sp, (self.r.pc >> 8) as u8); // PC high
                                                                // Re-select the interrupt from the CURRENT (IE & IF) right after
                                                                // the PC-HIGH push (Oracle ses_164828bc5 + mooneye ie_push). A
                                                                // push to $FFFF here can rewrite IE; the dispatch then commits to
                                                                // whatever is pending NOW. If the PC-high write cleared all
                                                                // pending interrupts, the dispatch is cancelled (PC=$0000, no IF
                                                                // clear); otherwise the highest-priority pending bit is serviced
                                                                // -- which may differ from the bit latched at dispatch start.
                                                                // The later PC-LOW push (phase 3) is too late to re-decide: by
                                                                // then the serviced interrupt is already committed (ie_push
                                                                // round 3, where SP=$0001 makes only the LOW byte hit IE).
                let pending = bus.irq_pending_mask();
                if pending == 0 {
                    *cancelled = true;
                } else {
                    let new_bit = pending.trailing_zeros() as u8;
                    *bit = new_bit;
                    *vector = 0x0040 + (new_bit as u16) * 8;
                }
                self.r.sp = self.r.sp.wrapping_sub(1);
                *phase = 3;
            }
            3 => {
                bus.write_m(self.r.sp, self.r.pc as u8); // PC low
                *phase = 4;
            }
            _ => {
                bus.idle_m();
                if *cancelled {
                    self.r.pc = 0x0000;
                } else {
                    bus.clear_if_bit(*bit);
                    self.r.pc = *vector;
                }
                self.mode = CpuMode::Running;
                self.exec = Exec::Boundary;
            }
        }
    }

    /// Run exactly one full instruction (fetch through the next boundary),
    /// returning the number of bus M-cycles it consumed. Used by the SM83
    /// vector harness. `bus_m_at` reads the bus's current M-cycle count.
    pub fn run_one_instruction<B: CpuBus, F: Fn(&B) -> u64>(
        &mut self,
        bus: &mut B,
        bus_m_at: F,
    ) -> u64 {
        let start = bus_m_at(bus);
        let mut guard = 0u32;
        loop {
            self.step_m(bus);
            if matches!(self.mode, CpuMode::Running) && self.exec == Exec::Boundary {
                break;
            }
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        bus_m_at(bus) - start
    }

    fn apply_cycle_completion<B: CpuBus>(
        &mut self,
        bus: &mut B,
        completion: CpuCycleCompletion,
        value: u8,
    ) {
        match completion {
            CpuCycleCompletion::None => {}
            CpuCycleCompletion::FetchOpcode => {
                if self.halt_bug {
                    self.halt_bug = false;
                } else {
                    self.r.pc = self.r.pc.wrapping_add(1);
                }
                self.exec = Exec::Execute {
                    op: value,
                    phase: 0,
                };
            }
            CpuCycleCompletion::ReadReg8 {
                target,
                increment_pc,
                finish,
            } => {
                self.write_reg8_target(target, value);
                if increment_pc {
                    self.r.pc = self.r.pc.wrapping_add(1);
                }
                if finish {
                    self.finish();
                }
            }
            CpuCycleCompletion::DispatchIdleToSpDec => {
                let CpuMode::InterruptDispatch { phase, .. } = &mut self.mode else {
                    unreachable!();
                };
                *phase = 1;
            }
            CpuCycleCompletion::DispatchSpDec => {
                let CpuMode::InterruptDispatch { phase, .. } = &mut self.mode else {
                    unreachable!();
                };
                self.r.sp = self.r.sp.wrapping_sub(1);
                *phase = 2;
            }
            CpuCycleCompletion::DispatchPushPcHigh => {
                let CpuMode::InterruptDispatch {
                    phase,
                    bit,
                    vector,
                    cancelled,
                } = &mut self.mode
                else {
                    unreachable!();
                };

                let pending = bus.irq_pending_mask();
                if pending == 0 {
                    *cancelled = true;
                } else {
                    let new_bit = pending.trailing_zeros() as u8;
                    *bit = new_bit;
                    *vector = 0x0040 + (new_bit as u16) * 8;
                }
                self.r.sp = self.r.sp.wrapping_sub(1);
                *phase = 3;
            }
            CpuCycleCompletion::DispatchPushPcLow => {
                let CpuMode::InterruptDispatch { phase, .. } = &mut self.mode else {
                    unreachable!();
                };
                *phase = 4;
            }
            CpuCycleCompletion::DispatchFinish => {
                let CpuMode::InterruptDispatch {
                    bit,
                    vector,
                    cancelled,
                    ..
                } = self.mode
                else {
                    unreachable!();
                };
                if cancelled {
                    self.r.pc = 0x0000;
                } else {
                    bus.clear_if_bit(bit);
                    self.r.pc = vector;
                }
                self.mode = CpuMode::Running;
                self.exec = Exec::Boundary;
            }
        }
    }

    fn write_reg8_target(&mut self, target: CpuReg8Target, value: u8) {
        match target {
            CpuReg8Target::A => self.r.a = value,
            CpuReg8Target::B => self.r.b = value,
            CpuReg8Target::C => self.r.c = value,
            CpuReg8Target::D => self.r.d = value,
            CpuReg8Target::E => self.r.e = value,
            CpuReg8Target::H => self.r.h = value,
            CpuReg8Target::L => self.r.l = value,
        }
    }
}

impl ActiveCpuCycle {
    fn completion(self) -> CpuCycleCompletion {
        match self {
            ActiveCpuCycle::Idle { completion }
            | ActiveCpuCycle::Fetch { completion, .. }
            | ActiveCpuCycle::Read { completion, .. }
            | ActiveCpuCycle::Write { completion, .. }
            | ActiveCpuCycle::OamBugIdu { completion, .. }
            | ActiveCpuCycle::OamBugReadIncDec { completion, .. } => completion,
        }
    }
}

impl crate::bus::sm83_vectors::VectorCpu for Cpu {
    fn load_state(
        &mut self,
        s: &crate::bus::sm83_vectors::VectorState,
        bus: &mut crate::bus::FlatBus,
    ) {
        self.r.a = s.a;
        self.r.f = s.f & 0xF0;
        self.r.b = s.b;
        self.r.c = s.c;
        self.r.d = s.d;
        self.r.e = s.e;
        self.r.h = s.h;
        self.r.l = s.l;
        self.r.sp = s.sp;
        self.r.pc = s.pc;
        if let Some(ime) = s.ime {
            self.ime = ime != 0;
        }
        self.ime_pending = s.ei.map(|v| v != 0).unwrap_or(false);
        self.ime_delay_boundary = u8::from(self.ime_pending);
        self.mode = CpuMode::Running;
        self.exec = Exec::Boundary;
        crate::bus::sm83_vectors::apply_initial(bus, s);
    }

    fn store_state(
        &self,
        bus: &crate::bus::FlatBus,
        ram_addrs: &[u16],
    ) -> crate::bus::sm83_vectors::VectorState {
        crate::bus::sm83_vectors::VectorState {
            a: self.r.a,
            b: self.r.b,
            c: self.r.c,
            d: self.r.d,
            e: self.r.e,
            f: self.r.f & 0xF0,
            h: self.r.h,
            l: self.r.l,
            sp: self.r.sp,
            pc: self.r.pc,
            ime: Some(self.ime as u8),
            ie: None,
            ei: Some(self.ime_pending as u8),
            ram: ram_addrs.iter().map(|&a| (a, bus.peek(a))).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::FlatBus;

    fn run_one_instr(cpu: &mut Cpu, bus: &mut FlatBus) -> u64 {
        // Count real bus M-cycles (FlatBus.m_cycles), not step_m calls: a
        // zero-bus internal transition (e.g. NOP's finish) is not an M-cycle.
        let start = bus.m_cycles;
        let mut guard = 0;
        loop {
            cpu.step_m(bus);
            if cpu.exec == Exec::Boundary && cpu.mode == CpuMode::Running {
                break;
            }
            guard += 1;
            if guard > 48 {
                panic!("instruction did not complete");
            }
        }
        bus.m_cycles - start
    }

    #[test]
    fn nop_is_one_mcycle() {
        let mut cpu = Cpu::new();
        let mut bus = FlatBus::new();
        bus.poke(0x0000, 0x00); // NOP
        let m = run_one_instr(&mut cpu, &mut bus);
        assert_eq!(m, 1, "NOP is 1 M-cycle");
        assert_eq!(cpu.r.pc, 0x0001);
    }

    #[test]
    fn ld_b_c_is_one_mcycle() {
        let mut cpu = Cpu::new();
        let mut bus = FlatBus::new();
        cpu.r.c = 0x42;
        bus.poke(0x0000, 0x41); // LD B,C
        let m = run_one_instr(&mut cpu, &mut bus);
        assert_eq!(m, 1, "LD r,r is 1 M-cycle");
        assert_eq!(cpu.r.b, 0x42);
        assert_eq!(cpu.r.pc, 0x0001);
    }

    // ---- real SM83 vector run gate ----------------------------------------

    use crate::bus::sm83_vectors::{parse_vector, VectorCpu};

    fn asset(file: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../assets/sm83/v1")
            .join(file)
    }

    /// Run every vector in `file`, returning (passed, failed, first_failure).
    fn run_vector_file(file: &str) -> (usize, usize, Option<String>) {
        let text = std::fs::read_to_string(asset(file))
            .unwrap_or_else(|_| panic!("asset {file} must exist"));
        let arr: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut pass = 0;
        let mut fail = 0;
        let mut first_fail = None;
        for raw in arr.as_array().unwrap() {
            let v = parse_vector(raw).expect("vector parses");
            let mut cpu = Cpu::new();
            let mut bus = FlatBus::new();
            cpu.load_state(&v.initial, &mut bus);
            // Run a single instruction, capturing its bus M-cycle count.
            let mcycles = cpu.run_one_instruction(&mut bus, |b| b.m_cycles);
            let ram_addrs: Vec<u16> = v.final_.ram.iter().map(|&(a, _)| a).collect();
            let got = cpu.store_state(&bus, &ram_addrs);

            // Final register + RAM state.
            let state_ok = got.a == v.final_.a
                && got.b == v.final_.b
                && got.c == v.final_.c
                && got.d == v.final_.d
                && got.e == v.final_.e
                && got.f == v.final_.f
                && got.h == v.final_.h
                && got.l == v.final_.l
                && got.sp == v.final_.sp
                && got.pc == v.final_.pc
                && got.ram == v.final_.ram;

            // M-cycle count: each entry in the vector's `cycles` array is one
            // bus M-cycle. A wrong instruction-cycle count fails here.
            let cycles_ok = mcycles as usize == v.cycles.len();

            // Interrupt-control final state. `ime` is compared when the vector
            // carries it. `ei` in a vector's `final` means the EI-delay has
            // promoted IME to 1 by the end of the NEXT instruction; the vectors
            // model a single instruction, so a `final.ei == Some(1)` vector
            // expects IME still 0 after THIS instruction (the EI itself) with the
            // promotion pending. We assert IME matches the vector's stated value.
            let ime_ok = match v.final_.ime {
                Some(expected) => (got.ime == Some(expected & 1)) || got.ime == Some(expected),
                None => true,
            };

            // `ei` validation: a vector's `final.ei == 1` means an EI-delay is
            // pending after this instruction. This is what actually proves EI
            // scheduled the deferred enable (a no-op EI would leave ei = 0).
            let ei_ok = match v.final_.ei {
                Some(expected) => got.ei == Some(expected),
                None => true,
            };

            if state_ok && cycles_ok && ime_ok && ei_ok {
                pass += 1;
            } else {
                fail += 1;
                if first_fail.is_none() {
                    let why = if !state_ok {
                        "state"
                    } else if !cycles_ok {
                        "cycles"
                    } else if !ime_ok {
                        "ime"
                    } else {
                        "ei"
                    };
                    first_fail = Some(format!(
                        "{} [{}]: exp pc={:04X} f={:02X} m={} ime={:?} ei={:?} | got pc={:04X} f={:02X} m={} ime={:?} ei={:?}",
                        v.name, why, v.final_.pc, v.final_.f, v.cycles.len(), v.final_.ime, v.final_.ei,
                        got.pc, got.f, mcycles, got.ime, got.ei
                    ));
                }
            }
        }
        (pass, fail, first_fail)
    }

    #[test]
    fn vector_run_nop_00() {
        let (pass, fail, first) = run_vector_file("00.json");
        assert_eq!(
            fail, 0,
            "NOP vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_ld_b_c_41() {
        let (pass, fail, first) = run_vector_file("41.json");
        assert_eq!(
            fail, 0,
            "LD B,C vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_06() {
        let (pass, fail, first) = run_vector_file("06.json");
        assert_eq!(
            fail, 0,
            "06 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_0e() {
        let (pass, fail, first) = run_vector_file("0e.json");
        assert_eq!(
            fail, 0,
            "0e vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_16() {
        let (pass, fail, first) = run_vector_file("16.json");
        assert_eq!(
            fail, 0,
            "16 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_1e() {
        let (pass, fail, first) = run_vector_file("1e.json");
        assert_eq!(
            fail, 0,
            "1e vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_26() {
        let (pass, fail, first) = run_vector_file("26.json");
        assert_eq!(
            fail, 0,
            "26 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_2e() {
        let (pass, fail, first) = run_vector_file("2e.json");
        assert_eq!(
            fail, 0,
            "2e vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_36() {
        let (pass, fail, first) = run_vector_file("36.json");
        assert_eq!(
            fail, 0,
            "36 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_3e() {
        let (pass, fail, first) = run_vector_file("3e.json");
        assert_eq!(
            fail, 0,
            "3e vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_80() {
        let (pass, fail, first) = run_vector_file("80.json");
        assert_eq!(
            fail, 0,
            "80 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_81() {
        let (pass, fail, first) = run_vector_file("81.json");
        assert_eq!(
            fail, 0,
            "81 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_82() {
        let (pass, fail, first) = run_vector_file("82.json");
        assert_eq!(
            fail, 0,
            "82 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_83() {
        let (pass, fail, first) = run_vector_file("83.json");
        assert_eq!(
            fail, 0,
            "83 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_84() {
        let (pass, fail, first) = run_vector_file("84.json");
        assert_eq!(
            fail, 0,
            "84 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_85() {
        let (pass, fail, first) = run_vector_file("85.json");
        assert_eq!(
            fail, 0,
            "85 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_86() {
        let (pass, fail, first) = run_vector_file("86.json");
        assert_eq!(
            fail, 0,
            "86 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_87() {
        let (pass, fail, first) = run_vector_file("87.json");
        assert_eq!(
            fail, 0,
            "87 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_88() {
        let (pass, fail, first) = run_vector_file("88.json");
        assert_eq!(
            fail, 0,
            "88 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_89() {
        let (pass, fail, first) = run_vector_file("89.json");
        assert_eq!(
            fail, 0,
            "89 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_8a() {
        let (pass, fail, first) = run_vector_file("8a.json");
        assert_eq!(
            fail, 0,
            "8a vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_8b() {
        let (pass, fail, first) = run_vector_file("8b.json");
        assert_eq!(
            fail, 0,
            "8b vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_8c() {
        let (pass, fail, first) = run_vector_file("8c.json");
        assert_eq!(
            fail, 0,
            "8c vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_8d() {
        let (pass, fail, first) = run_vector_file("8d.json");
        assert_eq!(
            fail, 0,
            "8d vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_8e() {
        let (pass, fail, first) = run_vector_file("8e.json");
        assert_eq!(
            fail, 0,
            "8e vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_8f() {
        let (pass, fail, first) = run_vector_file("8f.json");
        assert_eq!(
            fail, 0,
            "8f vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_90() {
        let (pass, fail, first) = run_vector_file("90.json");
        assert_eq!(
            fail, 0,
            "90 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_91() {
        let (pass, fail, first) = run_vector_file("91.json");
        assert_eq!(
            fail, 0,
            "91 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_92() {
        let (pass, fail, first) = run_vector_file("92.json");
        assert_eq!(
            fail, 0,
            "92 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_93() {
        let (pass, fail, first) = run_vector_file("93.json");
        assert_eq!(
            fail, 0,
            "93 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_94() {
        let (pass, fail, first) = run_vector_file("94.json");
        assert_eq!(
            fail, 0,
            "94 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_95() {
        let (pass, fail, first) = run_vector_file("95.json");
        assert_eq!(
            fail, 0,
            "95 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_96() {
        let (pass, fail, first) = run_vector_file("96.json");
        assert_eq!(
            fail, 0,
            "96 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_97() {
        let (pass, fail, first) = run_vector_file("97.json");
        assert_eq!(
            fail, 0,
            "97 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_98() {
        let (pass, fail, first) = run_vector_file("98.json");
        assert_eq!(
            fail, 0,
            "98 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_99() {
        let (pass, fail, first) = run_vector_file("99.json");
        assert_eq!(
            fail, 0,
            "99 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_9a() {
        let (pass, fail, first) = run_vector_file("9a.json");
        assert_eq!(
            fail, 0,
            "9a vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_9b() {
        let (pass, fail, first) = run_vector_file("9b.json");
        assert_eq!(
            fail, 0,
            "9b vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_9c() {
        let (pass, fail, first) = run_vector_file("9c.json");
        assert_eq!(
            fail, 0,
            "9c vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_9d() {
        let (pass, fail, first) = run_vector_file("9d.json");
        assert_eq!(
            fail, 0,
            "9d vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_9e() {
        let (pass, fail, first) = run_vector_file("9e.json");
        assert_eq!(
            fail, 0,
            "9e vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_9f() {
        let (pass, fail, first) = run_vector_file("9f.json");
        assert_eq!(
            fail, 0,
            "9f vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a0() {
        let (pass, fail, first) = run_vector_file("a0.json");
        assert_eq!(
            fail, 0,
            "a0 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a1() {
        let (pass, fail, first) = run_vector_file("a1.json");
        assert_eq!(
            fail, 0,
            "a1 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a2() {
        let (pass, fail, first) = run_vector_file("a2.json");
        assert_eq!(
            fail, 0,
            "a2 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a3() {
        let (pass, fail, first) = run_vector_file("a3.json");
        assert_eq!(
            fail, 0,
            "a3 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a4() {
        let (pass, fail, first) = run_vector_file("a4.json");
        assert_eq!(
            fail, 0,
            "a4 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a5() {
        let (pass, fail, first) = run_vector_file("a5.json");
        assert_eq!(
            fail, 0,
            "a5 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a6() {
        let (pass, fail, first) = run_vector_file("a6.json");
        assert_eq!(
            fail, 0,
            "a6 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a7() {
        let (pass, fail, first) = run_vector_file("a7.json");
        assert_eq!(
            fail, 0,
            "a7 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a8() {
        let (pass, fail, first) = run_vector_file("a8.json");
        assert_eq!(
            fail, 0,
            "a8 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_a9() {
        let (pass, fail, first) = run_vector_file("a9.json");
        assert_eq!(
            fail, 0,
            "a9 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_aa() {
        let (pass, fail, first) = run_vector_file("aa.json");
        assert_eq!(
            fail, 0,
            "aa vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_ab() {
        let (pass, fail, first) = run_vector_file("ab.json");
        assert_eq!(
            fail, 0,
            "ab vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_ac() {
        let (pass, fail, first) = run_vector_file("ac.json");
        assert_eq!(
            fail, 0,
            "ac vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_ad() {
        let (pass, fail, first) = run_vector_file("ad.json");
        assert_eq!(
            fail, 0,
            "ad vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_ae() {
        let (pass, fail, first) = run_vector_file("ae.json");
        assert_eq!(
            fail, 0,
            "ae vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_af() {
        let (pass, fail, first) = run_vector_file("af.json");
        assert_eq!(
            fail, 0,
            "af vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b0() {
        let (pass, fail, first) = run_vector_file("b0.json");
        assert_eq!(
            fail, 0,
            "b0 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b1() {
        let (pass, fail, first) = run_vector_file("b1.json");
        assert_eq!(
            fail, 0,
            "b1 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b2() {
        let (pass, fail, first) = run_vector_file("b2.json");
        assert_eq!(
            fail, 0,
            "b2 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b3() {
        let (pass, fail, first) = run_vector_file("b3.json");
        assert_eq!(
            fail, 0,
            "b3 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b4() {
        let (pass, fail, first) = run_vector_file("b4.json");
        assert_eq!(
            fail, 0,
            "b4 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b5() {
        let (pass, fail, first) = run_vector_file("b5.json");
        assert_eq!(
            fail, 0,
            "b5 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b6() {
        let (pass, fail, first) = run_vector_file("b6.json");
        assert_eq!(
            fail, 0,
            "b6 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b7() {
        let (pass, fail, first) = run_vector_file("b7.json");
        assert_eq!(
            fail, 0,
            "b7 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b8() {
        let (pass, fail, first) = run_vector_file("b8.json");
        assert_eq!(
            fail, 0,
            "b8 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_b9() {
        let (pass, fail, first) = run_vector_file("b9.json");
        assert_eq!(
            fail, 0,
            "b9 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_ba() {
        let (pass, fail, first) = run_vector_file("ba.json");
        assert_eq!(
            fail, 0,
            "ba vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_bb() {
        let (pass, fail, first) = run_vector_file("bb.json");
        assert_eq!(
            fail, 0,
            "bb vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_bc() {
        let (pass, fail, first) = run_vector_file("bc.json");
        assert_eq!(
            fail, 0,
            "bc vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_bd() {
        let (pass, fail, first) = run_vector_file("bd.json");
        assert_eq!(
            fail, 0,
            "bd vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_be() {
        let (pass, fail, first) = run_vector_file("be.json");
        assert_eq!(
            fail, 0,
            "be vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_bf() {
        let (pass, fail, first) = run_vector_file("bf.json");
        assert_eq!(
            fail, 0,
            "bf vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_c6() {
        let (pass, fail, first) = run_vector_file("c6.json");
        assert_eq!(
            fail, 0,
            "c6 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_ce() {
        let (pass, fail, first) = run_vector_file("ce.json");
        assert_eq!(
            fail, 0,
            "ce vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_d6() {
        let (pass, fail, first) = run_vector_file("d6.json");
        assert_eq!(
            fail, 0,
            "d6 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_de() {
        let (pass, fail, first) = run_vector_file("de.json");
        assert_eq!(
            fail, 0,
            "de vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_e6() {
        let (pass, fail, first) = run_vector_file("e6.json");
        assert_eq!(
            fail, 0,
            "e6 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_ee() {
        let (pass, fail, first) = run_vector_file("ee.json");
        assert_eq!(
            fail, 0,
            "ee vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_f6() {
        let (pass, fail, first) = run_vector_file("f6.json");
        assert_eq!(
            fail, 0,
            "f6 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_fe() {
        let (pass, fail, first) = run_vector_file("fe.json");
        assert_eq!(
            fail, 0,
            "fe vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_04() {
        let (pass, fail, first) = run_vector_file("04.json");
        assert_eq!(
            fail, 0,
            "04 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_0c() {
        let (pass, fail, first) = run_vector_file("0c.json");
        assert_eq!(
            fail, 0,
            "0c vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_14() {
        let (pass, fail, first) = run_vector_file("14.json");
        assert_eq!(
            fail, 0,
            "14 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_1c() {
        let (pass, fail, first) = run_vector_file("1c.json");
        assert_eq!(
            fail, 0,
            "1c vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_24() {
        let (pass, fail, first) = run_vector_file("24.json");
        assert_eq!(
            fail, 0,
            "24 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_2c() {
        let (pass, fail, first) = run_vector_file("2c.json");
        assert_eq!(
            fail, 0,
            "2c vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_34() {
        let (pass, fail, first) = run_vector_file("34.json");
        assert_eq!(
            fail, 0,
            "34 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_3c() {
        let (pass, fail, first) = run_vector_file("3c.json");
        assert_eq!(
            fail, 0,
            "3c vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_05() {
        let (pass, fail, first) = run_vector_file("05.json");
        assert_eq!(
            fail, 0,
            "05 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_0d() {
        let (pass, fail, first) = run_vector_file("0d.json");
        assert_eq!(
            fail, 0,
            "0d vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_15() {
        let (pass, fail, first) = run_vector_file("15.json");
        assert_eq!(
            fail, 0,
            "15 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_1d() {
        let (pass, fail, first) = run_vector_file("1d.json");
        assert_eq!(
            fail, 0,
            "1d vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_25() {
        let (pass, fail, first) = run_vector_file("25.json");
        assert_eq!(
            fail, 0,
            "25 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_2d() {
        let (pass, fail, first) = run_vector_file("2d.json");
        assert_eq!(
            fail, 0,
            "2d vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_35() {
        let (pass, fail, first) = run_vector_file("35.json");
        assert_eq!(
            fail, 0,
            "35 vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    #[test]
    fn vector_run_3d() {
        let (pass, fail, first) = run_vector_file("3d.json");
        assert_eq!(
            fail, 0,
            "3d vectors: {pass} pass, {fail} fail. first: {first:?}"
        );
        assert!(pass > 0);
    }

    macro_rules! main_vector_tests {
        ($($name:ident => $file:literal),+ $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let (pass, fail, first) = run_vector_file($file);
                    assert_eq!(
                        fail, 0,
                        "{} vectors: {} pass, {} fail. first: {:?}",
                        $file, pass, fail, first
                    );
                    assert!(pass > 0);
                }
            )+
        };
    }

    main_vector_tests! {
        vector_run_01 => "01.json",
        vector_run_02 => "02.json",
        vector_run_03 => "03.json",
        vector_run_07 => "07.json",
        vector_run_08 => "08.json",
        vector_run_09 => "09.json",
        vector_run_0a => "0a.json",
        vector_run_0b => "0b.json",
        vector_run_0f => "0f.json",
        vector_run_11 => "11.json",
        vector_run_12 => "12.json",
        vector_run_13 => "13.json",
        vector_run_17 => "17.json",
        vector_run_18 => "18.json",
        vector_run_19 => "19.json",
        vector_run_1a => "1a.json",
        vector_run_1b => "1b.json",
        vector_run_1f => "1f.json",
        vector_run_20 => "20.json",
        vector_run_21 => "21.json",
        vector_run_22 => "22.json",
        vector_run_23 => "23.json",
        vector_run_27 => "27.json",
        vector_run_28 => "28.json",
        vector_run_29 => "29.json",
        vector_run_2a => "2a.json",
        vector_run_2b => "2b.json",
        vector_run_2f => "2f.json",
        vector_run_30 => "30.json",
        vector_run_31 => "31.json",
        vector_run_32 => "32.json",
        vector_run_33 => "33.json",
        vector_run_37 => "37.json",
        vector_run_38 => "38.json",
        vector_run_39 => "39.json",
        vector_run_3a => "3a.json",
        vector_run_3b => "3b.json",
        vector_run_3f => "3f.json",
        vector_run_c0 => "c0.json",
        vector_run_c1 => "c1.json",
        vector_run_c2 => "c2.json",
        vector_run_c3 => "c3.json",
        vector_run_c4 => "c4.json",
        vector_run_c5 => "c5.json",
        vector_run_c7 => "c7.json",
        vector_run_c8 => "c8.json",
        vector_run_c9 => "c9.json",
        vector_run_ca => "ca.json",
        vector_run_cc => "cc.json",
        vector_run_cd => "cd.json",
        vector_run_cf => "cf.json",
        vector_run_d0 => "d0.json",
        vector_run_d1 => "d1.json",
        vector_run_d2 => "d2.json",
        vector_run_d4 => "d4.json",
        vector_run_d5 => "d5.json",
        vector_run_d7 => "d7.json",
        vector_run_d8 => "d8.json",
        vector_run_d9 => "d9.json",
        vector_run_da => "da.json",
        vector_run_dc => "dc.json",
        vector_run_df => "df.json",
        vector_run_e0 => "e0.json",
        vector_run_e1 => "e1.json",
        vector_run_e2 => "e2.json",
        vector_run_e5 => "e5.json",
        vector_run_e7 => "e7.json",
        vector_run_e8 => "e8.json",
        vector_run_e9 => "e9.json",
        vector_run_ea => "ea.json",
        vector_run_ef => "ef.json",
        vector_run_f0 => "f0.json",
        vector_run_f1 => "f1.json",
        vector_run_f2 => "f2.json",
        vector_run_f3 => "f3.json",
        vector_run_f5 => "f5.json",
        vector_run_f7 => "f7.json",
        vector_run_f8 => "f8.json",
        vector_run_f9 => "f9.json",
        vector_run_fa => "fa.json",
        vector_run_fb => "fb.json",
        vector_run_ff => "ff.json",
    }

    macro_rules! cb_vector_tests {
        ($($name:ident => $file:literal),+ $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let (pass, fail, first) = run_vector_file($file);
                    assert_eq!(
                        fail, 0,
                        "{} vectors: {} pass, {} fail. first: {:?}",
                        $file, pass, fail, first
                    );
                    assert!(pass > 0);
                }
            )+
        };
    }

    cb_vector_tests! {
        vector_run_cb_00 => "cb 00.json",
        vector_run_cb_01 => "cb 01.json",
        vector_run_cb_02 => "cb 02.json",
        vector_run_cb_03 => "cb 03.json",
        vector_run_cb_04 => "cb 04.json",
        vector_run_cb_05 => "cb 05.json",
        vector_run_cb_06 => "cb 06.json",
        vector_run_cb_07 => "cb 07.json",
        vector_run_cb_08 => "cb 08.json",
        vector_run_cb_09 => "cb 09.json",
        vector_run_cb_0a => "cb 0a.json",
        vector_run_cb_0b => "cb 0b.json",
        vector_run_cb_0c => "cb 0c.json",
        vector_run_cb_0d => "cb 0d.json",
        vector_run_cb_0e => "cb 0e.json",
        vector_run_cb_0f => "cb 0f.json",
        vector_run_cb_10 => "cb 10.json",
        vector_run_cb_11 => "cb 11.json",
        vector_run_cb_12 => "cb 12.json",
        vector_run_cb_13 => "cb 13.json",
        vector_run_cb_14 => "cb 14.json",
        vector_run_cb_15 => "cb 15.json",
        vector_run_cb_16 => "cb 16.json",
        vector_run_cb_17 => "cb 17.json",
        vector_run_cb_18 => "cb 18.json",
        vector_run_cb_19 => "cb 19.json",
        vector_run_cb_1a => "cb 1a.json",
        vector_run_cb_1b => "cb 1b.json",
        vector_run_cb_1c => "cb 1c.json",
        vector_run_cb_1d => "cb 1d.json",
        vector_run_cb_1e => "cb 1e.json",
        vector_run_cb_1f => "cb 1f.json",
        vector_run_cb_20 => "cb 20.json",
        vector_run_cb_21 => "cb 21.json",
        vector_run_cb_22 => "cb 22.json",
        vector_run_cb_23 => "cb 23.json",
        vector_run_cb_24 => "cb 24.json",
        vector_run_cb_25 => "cb 25.json",
        vector_run_cb_26 => "cb 26.json",
        vector_run_cb_27 => "cb 27.json",
        vector_run_cb_28 => "cb 28.json",
        vector_run_cb_29 => "cb 29.json",
        vector_run_cb_2a => "cb 2a.json",
        vector_run_cb_2b => "cb 2b.json",
        vector_run_cb_2c => "cb 2c.json",
        vector_run_cb_2d => "cb 2d.json",
        vector_run_cb_2e => "cb 2e.json",
        vector_run_cb_2f => "cb 2f.json",
        vector_run_cb_30 => "cb 30.json",
        vector_run_cb_31 => "cb 31.json",
        vector_run_cb_32 => "cb 32.json",
        vector_run_cb_33 => "cb 33.json",
        vector_run_cb_34 => "cb 34.json",
        vector_run_cb_35 => "cb 35.json",
        vector_run_cb_36 => "cb 36.json",
        vector_run_cb_37 => "cb 37.json",
        vector_run_cb_38 => "cb 38.json",
        vector_run_cb_39 => "cb 39.json",
        vector_run_cb_3a => "cb 3a.json",
        vector_run_cb_3b => "cb 3b.json",
        vector_run_cb_3c => "cb 3c.json",
        vector_run_cb_3d => "cb 3d.json",
        vector_run_cb_3e => "cb 3e.json",
        vector_run_cb_3f => "cb 3f.json",
        vector_run_cb_40 => "cb 40.json",
        vector_run_cb_41 => "cb 41.json",
        vector_run_cb_42 => "cb 42.json",
        vector_run_cb_43 => "cb 43.json",
        vector_run_cb_44 => "cb 44.json",
        vector_run_cb_45 => "cb 45.json",
        vector_run_cb_46 => "cb 46.json",
        vector_run_cb_47 => "cb 47.json",
        vector_run_cb_48 => "cb 48.json",
        vector_run_cb_49 => "cb 49.json",
        vector_run_cb_4a => "cb 4a.json",
        vector_run_cb_4b => "cb 4b.json",
        vector_run_cb_4c => "cb 4c.json",
        vector_run_cb_4d => "cb 4d.json",
        vector_run_cb_4e => "cb 4e.json",
        vector_run_cb_4f => "cb 4f.json",
        vector_run_cb_50 => "cb 50.json",
        vector_run_cb_51 => "cb 51.json",
        vector_run_cb_52 => "cb 52.json",
        vector_run_cb_53 => "cb 53.json",
        vector_run_cb_54 => "cb 54.json",
        vector_run_cb_55 => "cb 55.json",
        vector_run_cb_56 => "cb 56.json",
        vector_run_cb_57 => "cb 57.json",
        vector_run_cb_58 => "cb 58.json",
        vector_run_cb_59 => "cb 59.json",
        vector_run_cb_5a => "cb 5a.json",
        vector_run_cb_5b => "cb 5b.json",
        vector_run_cb_5c => "cb 5c.json",
        vector_run_cb_5d => "cb 5d.json",
        vector_run_cb_5e => "cb 5e.json",
        vector_run_cb_5f => "cb 5f.json",
        vector_run_cb_60 => "cb 60.json",
        vector_run_cb_61 => "cb 61.json",
        vector_run_cb_62 => "cb 62.json",
        vector_run_cb_63 => "cb 63.json",
        vector_run_cb_64 => "cb 64.json",
        vector_run_cb_65 => "cb 65.json",
        vector_run_cb_66 => "cb 66.json",
        vector_run_cb_67 => "cb 67.json",
        vector_run_cb_68 => "cb 68.json",
        vector_run_cb_69 => "cb 69.json",
        vector_run_cb_6a => "cb 6a.json",
        vector_run_cb_6b => "cb 6b.json",
        vector_run_cb_6c => "cb 6c.json",
        vector_run_cb_6d => "cb 6d.json",
        vector_run_cb_6e => "cb 6e.json",
        vector_run_cb_6f => "cb 6f.json",
        vector_run_cb_70 => "cb 70.json",
        vector_run_cb_71 => "cb 71.json",
        vector_run_cb_72 => "cb 72.json",
        vector_run_cb_73 => "cb 73.json",
        vector_run_cb_74 => "cb 74.json",
        vector_run_cb_75 => "cb 75.json",
        vector_run_cb_76 => "cb 76.json",
        vector_run_cb_77 => "cb 77.json",
        vector_run_cb_78 => "cb 78.json",
        vector_run_cb_79 => "cb 79.json",
        vector_run_cb_7a => "cb 7a.json",
        vector_run_cb_7b => "cb 7b.json",
        vector_run_cb_7c => "cb 7c.json",
        vector_run_cb_7d => "cb 7d.json",
        vector_run_cb_7e => "cb 7e.json",
        vector_run_cb_7f => "cb 7f.json",
        vector_run_cb_80 => "cb 80.json",
        vector_run_cb_81 => "cb 81.json",
        vector_run_cb_82 => "cb 82.json",
        vector_run_cb_83 => "cb 83.json",
        vector_run_cb_84 => "cb 84.json",
        vector_run_cb_85 => "cb 85.json",
        vector_run_cb_86 => "cb 86.json",
        vector_run_cb_87 => "cb 87.json",
        vector_run_cb_88 => "cb 88.json",
        vector_run_cb_89 => "cb 89.json",
        vector_run_cb_8a => "cb 8a.json",
        vector_run_cb_8b => "cb 8b.json",
        vector_run_cb_8c => "cb 8c.json",
        vector_run_cb_8d => "cb 8d.json",
        vector_run_cb_8e => "cb 8e.json",
        vector_run_cb_8f => "cb 8f.json",
        vector_run_cb_90 => "cb 90.json",
        vector_run_cb_91 => "cb 91.json",
        vector_run_cb_92 => "cb 92.json",
        vector_run_cb_93 => "cb 93.json",
        vector_run_cb_94 => "cb 94.json",
        vector_run_cb_95 => "cb 95.json",
        vector_run_cb_96 => "cb 96.json",
        vector_run_cb_97 => "cb 97.json",
        vector_run_cb_98 => "cb 98.json",
        vector_run_cb_99 => "cb 99.json",
        vector_run_cb_9a => "cb 9a.json",
        vector_run_cb_9b => "cb 9b.json",
        vector_run_cb_9c => "cb 9c.json",
        vector_run_cb_9d => "cb 9d.json",
        vector_run_cb_9e => "cb 9e.json",
        vector_run_cb_9f => "cb 9f.json",
        vector_run_cb_a0 => "cb a0.json",
        vector_run_cb_a1 => "cb a1.json",
        vector_run_cb_a2 => "cb a2.json",
        vector_run_cb_a3 => "cb a3.json",
        vector_run_cb_a4 => "cb a4.json",
        vector_run_cb_a5 => "cb a5.json",
        vector_run_cb_a6 => "cb a6.json",
        vector_run_cb_a7 => "cb a7.json",
        vector_run_cb_a8 => "cb a8.json",
        vector_run_cb_a9 => "cb a9.json",
        vector_run_cb_aa => "cb aa.json",
        vector_run_cb_ab => "cb ab.json",
        vector_run_cb_ac => "cb ac.json",
        vector_run_cb_ad => "cb ad.json",
        vector_run_cb_ae => "cb ae.json",
        vector_run_cb_af => "cb af.json",
        vector_run_cb_b0 => "cb b0.json",
        vector_run_cb_b1 => "cb b1.json",
        vector_run_cb_b2 => "cb b2.json",
        vector_run_cb_b3 => "cb b3.json",
        vector_run_cb_b4 => "cb b4.json",
        vector_run_cb_b5 => "cb b5.json",
        vector_run_cb_b6 => "cb b6.json",
        vector_run_cb_b7 => "cb b7.json",
        vector_run_cb_b8 => "cb b8.json",
        vector_run_cb_b9 => "cb b9.json",
        vector_run_cb_ba => "cb ba.json",
        vector_run_cb_bb => "cb bb.json",
        vector_run_cb_bc => "cb bc.json",
        vector_run_cb_bd => "cb bd.json",
        vector_run_cb_be => "cb be.json",
        vector_run_cb_bf => "cb bf.json",
        vector_run_cb_c0 => "cb c0.json",
        vector_run_cb_c1 => "cb c1.json",
        vector_run_cb_c2 => "cb c2.json",
        vector_run_cb_c3 => "cb c3.json",
        vector_run_cb_c4 => "cb c4.json",
        vector_run_cb_c5 => "cb c5.json",
        vector_run_cb_c6 => "cb c6.json",
        vector_run_cb_c7 => "cb c7.json",
        vector_run_cb_c8 => "cb c8.json",
        vector_run_cb_c9 => "cb c9.json",
        vector_run_cb_ca => "cb ca.json",
        vector_run_cb_cb => "cb cb.json",
        vector_run_cb_cc => "cb cc.json",
        vector_run_cb_cd => "cb cd.json",
        vector_run_cb_ce => "cb ce.json",
        vector_run_cb_cf => "cb cf.json",
        vector_run_cb_d0 => "cb d0.json",
        vector_run_cb_d1 => "cb d1.json",
        vector_run_cb_d2 => "cb d2.json",
        vector_run_cb_d3 => "cb d3.json",
        vector_run_cb_d4 => "cb d4.json",
        vector_run_cb_d5 => "cb d5.json",
        vector_run_cb_d6 => "cb d6.json",
        vector_run_cb_d7 => "cb d7.json",
        vector_run_cb_d8 => "cb d8.json",
        vector_run_cb_d9 => "cb d9.json",
        vector_run_cb_da => "cb da.json",
        vector_run_cb_db => "cb db.json",
        vector_run_cb_dc => "cb dc.json",
        vector_run_cb_dd => "cb dd.json",
        vector_run_cb_de => "cb de.json",
        vector_run_cb_df => "cb df.json",
        vector_run_cb_e0 => "cb e0.json",
        vector_run_cb_e1 => "cb e1.json",
        vector_run_cb_e2 => "cb e2.json",
        vector_run_cb_e3 => "cb e3.json",
        vector_run_cb_e4 => "cb e4.json",
        vector_run_cb_e5 => "cb e5.json",
        vector_run_cb_e6 => "cb e6.json",
        vector_run_cb_e7 => "cb e7.json",
        vector_run_cb_e8 => "cb e8.json",
        vector_run_cb_e9 => "cb e9.json",
        vector_run_cb_ea => "cb ea.json",
        vector_run_cb_eb => "cb eb.json",
        vector_run_cb_ec => "cb ec.json",
        vector_run_cb_ed => "cb ed.json",
        vector_run_cb_ee => "cb ee.json",
        vector_run_cb_ef => "cb ef.json",
        vector_run_cb_f0 => "cb f0.json",
        vector_run_cb_f1 => "cb f1.json",
        vector_run_cb_f2 => "cb f2.json",
        vector_run_cb_f3 => "cb f3.json",
        vector_run_cb_f4 => "cb f4.json",
        vector_run_cb_f5 => "cb f5.json",
        vector_run_cb_f6 => "cb f6.json",
        vector_run_cb_f7 => "cb f7.json",
        vector_run_cb_f8 => "cb f8.json",
        vector_run_cb_f9 => "cb f9.json",
        vector_run_cb_fa => "cb fa.json",
        vector_run_cb_fb => "cb fb.json",
        vector_run_cb_fc => "cb fc.json",
        vector_run_cb_fd => "cb fd.json",
        vector_run_cb_fe => "cb fe.json",
        vector_run_cb_ff => "cb ff.json",
    }
}
