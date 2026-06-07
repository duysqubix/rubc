//! Candidate-B CPU equivalence harness.
//!
//! This is test-only infrastructure for the per-T-cycle CPU migration.  B0 has
//! no second engine yet, so the harness records the current `Cpu::step_m` trace
//! and proves it is deterministic.  B1-B5 can feed a new-engine trace into
//! [`compare_traces`] and require exact snapshot equality at every legacy step.

#![cfg(test)]

use crate::bus::{CpuBus, FlatBus};

use super::{Cpu, CpuMode, Exec};

const DEFAULT_MAX_STEPS: usize = 64;
const TRACE_HASH_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const TRACE_HASH_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Full observable CPU + flat-bus state captured after a legacy `step_m` call.
///
/// `step_m` mostly advances one bus M-cycle, but a few current opcode phases
/// complete without touching the bus (for example `NOP`'s execute phase).  The
/// harness intentionally records after every legacy call, with `m_cycles` in
/// the snapshot, so B1-B5 preserve both bus-visible M-cycles and these existing
/// zero-bus completion boundaries until the migration deliberately changes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuSnapshot {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: bool,
    pub ime_pending: bool,
    pub ime_delay_boundary: u8,
    pub mode: CpuMode,
    pub exec: Exec,
    pub halt_bug: bool,
    pub tmp8: u8,
    pub tmp16: u16,
    pub bus_m_cycles: u64,
    pub flat_ie: u8,
    pub flat_if: u8,
    pub flat_mem_digest: u64,
}

impl CpuSnapshot {
    pub fn capture(cpu: &Cpu, bus: &FlatBus) -> Self {
        Self {
            a: cpu.r.a,
            f: cpu.r.f,
            b: cpu.r.b,
            c: cpu.r.c,
            d: cpu.r.d,
            e: cpu.r.e,
            h: cpu.r.h,
            l: cpu.r.l,
            sp: cpu.r.sp,
            pc: cpu.r.pc,
            ime: cpu.ime,
            ime_pending: cpu.equiv_ime_pending(),
            ime_delay_boundary: cpu.equiv_ime_delay_boundary(),
            mode: cpu.mode,
            exec: cpu.equiv_exec(),
            halt_bug: cpu.equiv_halt_bug(),
            tmp8: cpu.equiv_tmp8(),
            tmp16: cpu.equiv_tmp16(),
            bus_m_cycles: bus.m_cycles,
            flat_ie: bus.ie(),
            flat_if: bus.if_(),
            flat_mem_digest: flat_mem_digest(bus),
        }
    }
}

/// Exact legacy trace: snapshots after each `step_m` call until instruction
/// boundary (including HALT/STOP boundary states) or `max_steps` guard.
pub fn trace_step_m_until_boundary(
    mut cpu: Cpu,
    mut bus: FlatBus,
    max_steps: usize,
) -> Result<Vec<CpuSnapshot>, TraceError> {
    let mut trace = Vec::new();

    for step in 0..max_steps {
        cpu.step_m(&mut bus);
        trace.push(CpuSnapshot::capture(&cpu, &bus));
        if cpu.exec_is_boundary() && !matches!(cpu.mode, CpuMode::InterruptDispatch { .. }) {
            return Ok(trace);
        }
        if step + 1 == max_steps {
            break;
        }
    }

    Err(TraceError::MaxStepsExceeded { max_steps })
}

pub fn trace_instruction(cpu: Cpu, bus: FlatBus) -> Result<Vec<CpuSnapshot>, TraceError> {
    trace_step_m_until_boundary(cpu, bus, DEFAULT_MAX_STEPS)
}

pub fn trace_b1_step_t_supported_instruction(
    mut cpu: Cpu,
    mut bus: FlatBus,
) -> Result<Vec<CpuSnapshot>, TraceError> {
    trace_b2_step_t_supported_instruction_inner(&mut cpu, &mut bus)
}

pub fn trace_b2_step_t_supported_instruction(
    mut cpu: Cpu,
    mut bus: FlatBus,
) -> Result<Vec<CpuSnapshot>, TraceError> {
    trace_b2_step_t_supported_instruction_inner(&mut cpu, &mut bus)
}

fn trace_b2_step_t_supported_instruction_inner(
    cpu: &mut Cpu,
    bus: &mut FlatBus,
) -> Result<Vec<CpuSnapshot>, TraceError> {
    let mut trace = Vec::new();

    for step in 0..DEFAULT_MAX_STEPS {
        if !cpu.step_b2_supported_m_via_t(bus) {
            return Err(TraceError::UnsupportedB1Opcode {
                exec: cpu.equiv_exec(),
            });
        }
        trace.push(CpuSnapshot::capture(cpu, bus));
        if cpu.exec_is_boundary() && !matches!(cpu.mode, CpuMode::InterruptDispatch { .. }) {
            return Ok(trace);
        }
        if step + 1 == DEFAULT_MAX_STEPS {
            break;
        }
    }

    Err(TraceError::MaxStepsExceeded {
        max_steps: DEFAULT_MAX_STEPS,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceError {
    MaxStepsExceeded { max_steps: usize },
    UnsupportedB1Opcode { exec: Exec },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceMismatch {
    Length {
        legacy_len: usize,
        new_len: usize,
    },
    Snapshot {
        index: usize,
        legacy: CpuSnapshot,
        new: CpuSnapshot,
    },
}

pub fn compare_traces(legacy: &[CpuSnapshot], new: &[CpuSnapshot]) -> Result<(), TraceMismatch> {
    if legacy.len() != new.len() {
        return Err(TraceMismatch::Length {
            legacy_len: legacy.len(),
            new_len: new.len(),
        });
    }

    for (index, (legacy, new)) in legacy.iter().zip(new).enumerate() {
        if legacy != new {
            return Err(TraceMismatch::Snapshot {
                index,
                legacy: legacy.clone(),
                new: new.clone(),
            });
        }
    }

    Ok(())
}

/// Deterministic, controlled flat-bus CPU fixture for any main opcode.
pub fn opcode_fixture(op: u8) -> (Cpu, FlatBus) {
    let mut cpu = seeded_cpu();
    let mut bus = seeded_bus();
    cpu.r.pc = 0x0100;
    bus.poke(0x0100, op);
    bus.poke(0x0101, 0x34);
    bus.poke(0x0102, 0x12);
    (cpu, bus)
}

/// Deterministic, controlled flat-bus CPU fixture for any CB opcode.
pub fn cb_opcode_fixture(op: u8) -> (Cpu, FlatBus) {
    let (cpu, mut bus) = opcode_fixture(0xCB);
    bus.poke(0x0101, op);
    (cpu, bus)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldenKind {
    Nop,
    LdBD8,
    LdA16Sp,
    CallA16,
    Ret,
    PushBc,
    PopBc,
    CbRlcB,
    InterruptDispatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldenTraceFixture {
    pub name: &'static str,
    pub kind: GoldenKind,
    pub expected_len: usize,
    pub expected_digest: u64,
}

impl GoldenTraceFixture {
    pub fn initial_state(self) -> (Cpu, FlatBus) {
        match self.kind {
            GoldenKind::Nop => program_fixture(&[0x00]),
            GoldenKind::LdBD8 => program_fixture(&[0x06, 0xA5]),
            GoldenKind::LdA16Sp => program_fixture(&[0x08, 0x00, 0xC0]),
            GoldenKind::CallA16 => program_fixture(&[0xCD, 0x00, 0x40]),
            GoldenKind::Ret => {
                let (mut cpu, mut bus) = program_fixture(&[0xC9]);
                cpu.r.sp = 0xD000;
                bus.poke(0xD000, 0x78);
                bus.poke(0xD001, 0x56);
                (cpu, bus)
            }
            GoldenKind::PushBc => program_fixture(&[0xC5]),
            GoldenKind::PopBc => {
                let (mut cpu, mut bus) = program_fixture(&[0xC1]);
                cpu.r.sp = 0xD000;
                bus.poke(0xD000, 0xCD);
                bus.poke(0xD001, 0xAB);
                (cpu, bus)
            }
            GoldenKind::CbRlcB => program_fixture(&[0xCB, 0x00]),
            GoldenKind::InterruptDispatch => interrupt_fixture(),
        }
    }

    pub fn capture(self) -> Vec<CpuSnapshot> {
        let (cpu, bus) = self.initial_state();
        trace_instruction(cpu, bus).expect(self.name)
    }
}

pub const GOLDEN_TRACE_FIXTURES: &[GoldenTraceFixture] = &[
    GoldenTraceFixture {
        name: "NOP",
        kind: GoldenKind::Nop,
        expected_len: 2,
        expected_digest: 16_958_643_655_462_282_994,
    },
    GoldenTraceFixture {
        name: "LD B,d8",
        kind: GoldenKind::LdBD8,
        expected_len: 3,
        expected_digest: 9_665_235_618_041_387_537,
    },
    GoldenTraceFixture {
        name: "LD (a16),SP",
        kind: GoldenKind::LdA16Sp,
        expected_len: 6,
        expected_digest: 2_053_460_029_804_388_125,
    },
    GoldenTraceFixture {
        name: "CALL a16",
        kind: GoldenKind::CallA16,
        expected_len: 7,
        expected_digest: 3_359_008_037_549_533_805,
    },
    GoldenTraceFixture {
        name: "RET",
        kind: GoldenKind::Ret,
        expected_len: 5,
        expected_digest: 5_451_895_424_911_316_636,
    },
    GoldenTraceFixture {
        name: "PUSH BC",
        kind: GoldenKind::PushBc,
        expected_len: 5,
        expected_digest: 6_952_726_061_952_546_092,
    },
    GoldenTraceFixture {
        name: "POP BC",
        kind: GoldenKind::PopBc,
        expected_len: 4,
        expected_digest: 12_469_340_811_625_377_642,
    },
    GoldenTraceFixture {
        name: "CB RLC B",
        kind: GoldenKind::CbRlcB,
        expected_len: 3,
        expected_digest: 18_238_006_097_277_624_296,
    },
    GoldenTraceFixture {
        name: "interrupt dispatch",
        kind: GoldenKind::InterruptDispatch,
        expected_len: 5,
        expected_digest: 10_818_004_750_418_623_384,
    },
];

pub fn trace_digest(trace: &[CpuSnapshot]) -> u64 {
    let mut hash = TRACE_HASH_OFFSET;
    hash = fnv_mix_usize(hash, trace.len());
    for snapshot in trace {
        hash = hash_snapshot(hash, snapshot);
    }
    hash
}

fn hash_snapshot(mut hash: u64, s: &CpuSnapshot) -> u64 {
    for b in [s.a, s.f, s.b, s.c, s.d, s.e, s.h, s.l] {
        hash = fnv_mix_u8(hash, b);
    }
    hash = fnv_mix_u16(hash, s.sp);
    hash = fnv_mix_u16(hash, s.pc);
    hash = fnv_mix_bool(hash, s.ime);
    hash = fnv_mix_bool(hash, s.ime_pending);
    hash = fnv_mix_u8(hash, s.ime_delay_boundary);
    hash = hash_mode(hash, s.mode);
    hash = hash_exec(hash, s.exec);
    hash = fnv_mix_bool(hash, s.halt_bug);
    hash = fnv_mix_u8(hash, s.tmp8);
    hash = fnv_mix_u16(hash, s.tmp16);
    hash = fnv_mix_u64(hash, s.bus_m_cycles);
    hash = fnv_mix_u8(hash, s.flat_ie);
    hash = fnv_mix_u8(hash, s.flat_if);
    fnv_mix_u64(hash, s.flat_mem_digest)
}

fn program_fixture(program: &[u8]) -> (Cpu, FlatBus) {
    let mut cpu = seeded_cpu();
    let mut bus = seeded_bus();
    cpu.r.pc = 0x0100;
    for (offset, &byte) in program.iter().enumerate() {
        bus.poke(0x0100 + offset as u16, byte);
    }
    (cpu, bus)
}

fn interrupt_fixture() -> (Cpu, FlatBus) {
    let mut cpu = seeded_cpu();
    let mut bus = seeded_bus();
    cpu.r.pc = 0x1234;
    cpu.r.sp = 0xD000;
    cpu.ime = true;
    bus.set_ie(0x01);
    bus.set_if(0x01);
    (cpu, bus)
}

fn halt_bug_fetch_fixture() -> (Cpu, FlatBus) {
    let mut cpu = seeded_cpu();
    let mut bus = seeded_bus();
    cpu.r.pc = 0x0100;
    bus.poke(0x0100, 0x00);
    bus.set_ie(0x01);
    bus.set_if(0x01);
    cpu.enter_halt(&bus);
    (cpu, bus)
}

fn seeded_cpu() -> Cpu {
    let mut cpu = Cpu::new();
    cpu.r.a = 0x12;
    cpu.r.f = 0xB0;
    cpu.r.b = 0x34;
    cpu.r.c = 0x56;
    cpu.r.d = 0x78;
    cpu.r.e = 0x9A;
    cpu.r.h = 0xC0;
    cpu.r.l = 0x20;
    cpu.r.sp = 0xDFFE;
    cpu
}

fn seeded_bus() -> FlatBus {
    let mut bus = FlatBus::new();
    for addr in 0u32..=0xFFFF {
        let byte = (addr as u8).wrapping_mul(37).wrapping_add(0x5A);
        bus.poke(addr as u16, byte);
    }
    bus
}

fn flat_mem_digest(bus: &FlatBus) -> u64 {
    let mut hash = TRACE_HASH_OFFSET;
    for (addr, byte) in bus.mem.iter().copied().enumerate() {
        hash = fnv_mix_u8(hash, byte);
        if addr & 0xFF == 0xFF {
            hash = fnv_mix_u16(hash, addr as u16);
        }
    }
    hash
}

fn hash_mode(mut hash: u64, mode: CpuMode) -> u64 {
    match mode {
        CpuMode::Running => fnv_mix_u8(hash, 0),
        CpuMode::Halt => fnv_mix_u8(hash, 1),
        CpuMode::Stopped => fnv_mix_u8(hash, 2),
        CpuMode::InterruptDispatch {
            phase,
            bit,
            vector,
            cancelled,
        } => {
            hash = fnv_mix_u8(hash, 3);
            hash = fnv_mix_u8(hash, phase);
            hash = fnv_mix_u8(hash, bit);
            hash = fnv_mix_u16(hash, vector);
            fnv_mix_bool(hash, cancelled)
        }
    }
}

fn hash_exec(mut hash: u64, exec: Exec) -> u64 {
    match exec {
        Exec::Boundary => fnv_mix_u8(hash, 0),
        Exec::Execute { op, phase } => {
            hash = fnv_mix_u8(hash, 1);
            hash = fnv_mix_u8(hash, op);
            fnv_mix_u8(hash, phase)
        }
        Exec::CbExecute { op, phase } => {
            hash = fnv_mix_u8(hash, 2);
            hash = fnv_mix_u8(hash, op);
            fnv_mix_u8(hash, phase)
        }
    }
}

fn fnv_mix_bool(hash: u64, v: bool) -> u64 {
    fnv_mix_u8(hash, u8::from(v))
}

fn fnv_mix_usize(hash: u64, v: usize) -> u64 {
    fnv_mix_u64(hash, v as u64)
}

fn fnv_mix_u16(mut hash: u64, v: u16) -> u64 {
    for byte in v.to_le_bytes() {
        hash = fnv_mix_u8(hash, byte);
    }
    hash
}

fn fnv_mix_u64(mut hash: u64, v: u64) -> u64 {
    for byte in v.to_le_bytes() {
        hash = fnv_mix_u8(hash, byte);
    }
    hash
}

fn fnv_mix_u8(mut hash: u64, byte: u8) -> u64 {
    hash ^= u64::from(byte);
    hash.wrapping_mul(TRACE_HASH_PRIME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_trace_is_deterministic() {
        let (cpu, bus) = program_fixture(&[0x08, 0x00, 0xC0]);
        let first = trace_instruction(cpu.clone(), bus.clone()).unwrap();
        let second = trace_instruction(cpu, bus).unwrap();
        compare_traces(&first, &second).unwrap();
    }

    #[test]
    fn compare_traces_reports_length_and_snapshot_mismatches() {
        let (cpu, bus) = program_fixture(&[0x00]);
        let trace = trace_instruction(cpu, bus).unwrap();
        assert_eq!(
            compare_traces(&trace, &trace[..trace.len() - 1]),
            Err(TraceMismatch::Length {
                legacy_len: trace.len(),
                new_len: trace.len() - 1,
            })
        );

        let mut changed = trace.clone();
        changed[0].pc = changed[0].pc.wrapping_add(1);
        assert!(matches!(
            compare_traces(&trace, &changed),
            Err(TraceMismatch::Snapshot { index: 0, .. })
        ));
    }

    #[test]
    fn all_main_and_cb_opcodes_trace_without_panicking() {
        for op in 0u8..=0xFF {
            let (cpu, bus) = opcode_fixture(op);
            let trace = trace_instruction(cpu, bus)
                .unwrap_or_else(|err| panic!("main opcode {op:02X} failed: {err:?}"));
            assert!(!trace.is_empty(), "main opcode {op:02X} trace is empty");
        }

        for op in 0u8..=0xFF {
            let (cpu, bus) = cb_opcode_fixture(op);
            let trace = trace_instruction(cpu, bus)
                .unwrap_or_else(|err| panic!("CB opcode {op:02X} failed: {err:?}"));
            assert!(!trace.is_empty(), "CB opcode {op:02X} trace is empty");
        }
    }

    #[test]
    fn golden_trace_fixtures_match_candidate_c_baseline() {
        for fixture in GOLDEN_TRACE_FIXTURES {
            let trace = fixture.capture();
            assert_eq!(trace.len(), fixture.expected_len, "{} length", fixture.name);
            assert_eq!(
                trace_digest(&trace),
                fixture.expected_digest,
                "{} digest",
                fixture.name
            );
        }
    }

    #[test]
    fn b1_step_t_substrate_matches_legacy_nop() {
        let (cpu, bus) = GoldenTraceFixture {
            name: "NOP",
            kind: GoldenKind::Nop,
            expected_len: 2,
            expected_digest: 0,
        }
        .initial_state();

        let legacy = trace_instruction(cpu.clone(), bus.clone()).unwrap();
        let b1 = trace_b1_step_t_supported_instruction(cpu, bus).unwrap();
        compare_traces(&legacy, &b1).unwrap();
    }

    #[test]
    fn b1_step_t_substrate_matches_legacy_ld_b_d8() {
        let (cpu, bus) = GoldenTraceFixture {
            name: "LD B,d8",
            kind: GoldenKind::LdBD8,
            expected_len: 3,
            expected_digest: 0,
        }
        .initial_state();

        let legacy = trace_instruction(cpu.clone(), bus.clone()).unwrap();
        let b1 = trace_b1_step_t_supported_instruction(cpu, bus).unwrap();
        compare_traces(&legacy, &b1).unwrap();
    }

    #[test]
    fn b2_step_t_boundary_fetch_matches_plain_legacy_fetch() {
        let (cpu, bus) = program_fixture(&[0x00]);
        let legacy = trace_instruction(cpu.clone(), bus.clone()).unwrap();
        let b2 = trace_b2_step_t_supported_instruction(cpu, bus).unwrap();
        compare_traces(&legacy, &b2).unwrap();
    }

    #[test]
    fn b2_step_t_boundary_dispatch_preempts_fetch_like_legacy() {
        let (cpu, bus) = interrupt_fixture();
        let legacy = trace_instruction(cpu.clone(), bus.clone()).unwrap();
        let b2 = trace_b2_step_t_supported_instruction(cpu, bus).unwrap();
        compare_traces(&legacy, &b2).unwrap();
    }

    #[test]
    fn b2_step_t_boundary_fetch_matches_halt_bug_legacy_fetch() {
        let (cpu, bus) = halt_bug_fetch_fixture();
        let legacy = trace_instruction(cpu.clone(), bus.clone()).unwrap();
        let b2 = trace_b2_step_t_supported_instruction(cpu, bus).unwrap();
        compare_traces(&legacy, &b2).unwrap();
    }
}
