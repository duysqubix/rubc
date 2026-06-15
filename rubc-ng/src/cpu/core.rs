//! The M-cycle CPU state machine.
//!
//! `Cpu::step_m(bus)` performs exactly ONE bus M-cycle. Multi-M-cycle
//! instructions advance one phase per call; zero-cycle internal transitions
//! (e.g. boundary -> fetch) loop until exactly one bus operation happens.

use super::scheduler::{CpuAccessPlan, Time, CPU_ACCESS_END_OFFSET, SUBPHASES_PER_T_U8};
use super::CpuBus;

use super::regs::Regs;

const PPU_IRQ_BITS: u8 = 0x03;

fn time_after(start: Time, offset: u8) -> Time {
    Time(start.0 + u64::from(offset))
}

/// High-level CPU mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Exec {
    /// Between instructions: poll interrupts / promote EI, then fetch.
    Boundary,
    /// Decoding an opcode at `phase` (0 = just fetched).
    Execute { op: u8, phase: u8 },
    /// CB-prefixed opcode at `phase`.
    CbExecute { op: u8, phase: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CpuReg8Target {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

impl ActiveCpuCycle {
    /// The explicit sub-dot timing of this M-cycle (ADR 0001 stage 2). Derived
    /// from the cycle kind + address so it reproduces today's `write_drive_ticks`
    /// / read-at-end-of-M placement exactly -- behavior-preserving.
    fn access_plan<B: CpuBus>(&self, bus: &B) -> CpuAccessPlan {
        match *self {
            ActiveCpuCycle::Write { addr, .. } => CpuAccessPlan::write(bus.write_drive_ticks(addr)),
            ActiveCpuCycle::Fetch { .. }
            | ActiveCpuCycle::Read { .. }
            | ActiveCpuCycle::OamBugReadIncDec { .. } => CpuAccessPlan::read_like(),
            ActiveCpuCycle::Idle { .. } | ActiveCpuCycle::OamBugIdu { .. } => CpuAccessPlan::idle(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ActiveCpuCycleState {
    cycle: ActiveCpuCycle,
    elapsed_t: u8,
    /// The access plan (ADR 0001 stage 2), computed once at T0 after
    /// `begin_cpu_cycle`. `None` until the first tick of this M-cycle.
    plan: Option<CpuAccessPlan>,
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
        let start = self.inner.now();
        self.inner
            .advance_to(time_after(start, CPU_ACCESS_END_OFFSET));
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
        let start = self.inner.now();
        let plan = CpuAccessPlan::write(self.inner.write_drive_ticks(addr));
        if let Some(offset) = plan.write_visible_at {
            self.inner
                .schedule_cpu_write(time_after(start, offset), addr, value);
        }
        self.inner.advance_to(time_after(start, plan.end));
        self.inner.end_cpu_cycle();
    }

    fn idle_m(&mut self) {
        self.run_cycle();
        self.inner.observe_idle_m();
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

    fn now(&self) -> Time {
        self.inner.now()
    }

    fn schedule_cpu_write(&mut self, at: Time, addr: u16, value: u8) {
        self.inner.schedule_cpu_write(at, addr, value);
    }

    fn drain_cpu_writes_through(&mut self, now: Time) {
        self.inner.drain_cpu_writes_through(now);
    }

    fn advance_to(&mut self, target: Time) {
        self.inner.advance_to(target);
    }

    fn sync_ppu_to_cpu(&mut self) {
        self.inner.sync_ppu_to_cpu();
    }

    fn end_cpu_cycle(&mut self) {
        self.inner.end_cpu_cycle();
    }
}

/// The SM83 CPU.
#[cfg_attr(test, derive(Clone))]
#[derive(serde::Serialize, serde::Deserialize)]
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

#[allow(dead_code)]
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
            plan: None,
        });
    }

    fn sync_ppu_if_irq_can_affect_boundary<B: CpuBus>(&self, bus: &mut B) {
        if bus.ie() & PPU_IRQ_BITS != 0 {
            bus.sync_ppu_to_cpu();
        }
    }

    fn sync_ppu_if_halt_can_wake<B: CpuBus>(&self, bus: &mut B) {
        if bus.ie() & PPU_IRQ_BITS != 0 {
            bus.sync_ppu_to_cpu();
        }
    }

    pub fn step_t<B: CpuBus>(&mut self, bus: &mut B) -> bool {
        let Some(mut state) = self.active_cycle else {
            return false;
        };

        if state.elapsed_t == 0 {
            bus.begin_cpu_cycle();
            let start = bus.now();
            // ADR 0001 stage 2: snapshot the explicit access plan AFTER the
            // OAM-DMA beat in begin_cpu_cycle, so the T0 (BGP) write still lands
            // in the same order as before.
            let plan = state.cycle.access_plan(bus);
            state.plan = Some(plan);
            if let ActiveCpuCycle::Write { addr, value, .. } = state.cycle {
                if let Some(offset) = plan.write_visible_at {
                    bus.schedule_cpu_write(time_after(start, offset), addr, value);
                }
            }
            if matches!(
                state.cycle,
                ActiveCpuCycle::Idle { .. } | ActiveCpuCycle::OamBugIdu { .. }
            ) {
                bus.observe_idle_m();
            }
        }

        debug_assert!(
            state.plan.is_some(),
            "CPU access plan must be set at T0 before any tick"
        );

        let target = time_after(bus.now(), SUBPHASES_PER_T_U8);
        bus.advance_to(target);
        state.elapsed_t += 1;

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
                    self.sync_ppu_if_irq_can_affect_boundary(bus);
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
                    self.sync_ppu_if_irq_can_affect_boundary(bus);
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
                    self.sync_ppu_if_halt_can_wake(bus);
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
                        self.sync_ppu_if_irq_can_affect_boundary(bus);
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
                    self.sync_ppu_if_halt_can_wake(bus);
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
                        self.sync_ppu_if_irq_can_affect_boundary(bus);
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

    pub fn set_ime_for_vector(&mut self, ime: bool) {
        self.ime = ime;
        self.ime_pending = false;
        self.ime_delay_boundary = 0;
    }

    pub fn set_ei_pending_for_vector(&mut self, pending: bool) {
        self.ime_pending = pending;
        self.ime_delay_boundary = u8::from(pending);
    }

    pub fn ei_pending_for_vector(&self) -> bool {
        self.ime_pending
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

    pub(super) fn enter_halt<B: CpuBus>(&mut self, bus: &mut B) {
        self.sync_ppu_if_halt_can_wake(bus);
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
