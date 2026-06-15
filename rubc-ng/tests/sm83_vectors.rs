use rubc_ng::cpu::{Cpu, FlatIntentBus, VectorCpu, VectorState};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Vector {
    name: String,
    initial: VectorState,
    final_: VectorState,
    cycles: Vec<VectorCycle>,
}

#[derive(Debug)]
struct VectorCycle {
    addr: Option<u16>,
    data: Option<u8>,
    kind: String,
}

fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rubc-ng has a workspace parent")
        .join("assets/sm83/v1")
}

fn byte(v: &Value, key: &'static str) -> u8 {
    v.get(key).and_then(Value::as_u64).expect(key) as u8
}

fn word(v: &Value, key: &'static str) -> u16 {
    v.get(key).and_then(Value::as_u64).expect(key) as u16
}

fn opt_byte(v: &Value, key: &'static str) -> Option<u8> {
    v.get(key).map(|x| x.as_u64().expect(key) as u8)
}

fn state(v: &Value) -> VectorState {
    let ram = v
        .get("ram")
        .and_then(Value::as_array)
        .expect("ram")
        .iter()
        .map(|pair| {
            let pair = pair.as_array().expect("ram pair");
            (
                pair[0].as_u64().expect("ram addr") as u16,
                pair[1].as_u64().expect("ram byte") as u8,
            )
        })
        .collect();

    VectorState {
        a: byte(v, "a"),
        b: byte(v, "b"),
        c: byte(v, "c"),
        d: byte(v, "d"),
        e: byte(v, "e"),
        f: byte(v, "f"),
        h: byte(v, "h"),
        l: byte(v, "l"),
        sp: word(v, "sp"),
        pc: word(v, "pc"),
        ime: opt_byte(v, "ime"),
        ie: opt_byte(v, "ie"),
        ei: opt_byte(v, "ei"),
        ram,
    }
}

fn cycle(v: &Value) -> VectorCycle {
    let entry = v.as_array().expect("cycle entry");
    assert_eq!(entry.len(), 3, "cycle entry shape");
    VectorCycle {
        addr: entry[0].as_u64().map(|n| n as u16),
        data: entry[1].as_u64().map(|n| n as u8),
        kind: entry[2].as_str().expect("cycle kind").to_owned(),
    }
}

fn load_vectors(path: &Path) -> Vec<Vector> {
    let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    let json: Value = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
    json.as_array()
        .expect("top-level vector array")
        .iter()
        .map(|v| Vector {
            name: v
                .get("name")
                .and_then(Value::as_str)
                .expect("name")
                .to_owned(),
            initial: state(v.get("initial").expect("initial")),
            final_: state(v.get("final").expect("final")),
            cycles: v
                .get("cycles")
                .and_then(Value::as_array)
                .expect("cycles")
                .iter()
                .map(cycle)
                .collect(),
        })
        .collect()
}

fn assert_cycles(bus: &FlatIntentBus, expected: &[VectorCycle], label: &str) {
    let actual = bus.cycles();
    assert_eq!(actual.len(), expected.len(), "{label} cycle count");
    for (i, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual.kind, expected.kind, "{label} cycle {i} kind");
        if expected.kind != "---" {
            assert_eq!(actual.addr, expected.addr, "{label} cycle {i} addr");
            assert_eq!(actual.data, expected.data, "{label} cycle {i} data");
        }
    }
}

fn run_vector_exact_cycles(cpu: &mut Cpu, bus: &mut FlatIntentBus, cycles: usize) {
    let target = bus.m_cycles() + cycles as u64;
    let mut guard = 0u32;
    while bus.m_cycles() < target || !cpu.exec_is_boundary() {
        cpu.step_m(bus);
        guard += 1;
        assert!(guard <= 64, "vector did not consume expected bus cycles");
    }
    assert_eq!(bus.m_cycles(), target, "vector consumed extra bus cycles");
}

fn assert_final_state(actual: &VectorState, expected: &VectorState, label: &str) {
    assert_eq!(actual.a, expected.a, "{label} A");
    assert_eq!(actual.b, expected.b, "{label} B");
    assert_eq!(actual.c, expected.c, "{label} C");
    assert_eq!(actual.d, expected.d, "{label} D");
    assert_eq!(actual.e, expected.e, "{label} E");
    assert_eq!(actual.f, expected.f, "{label} F");
    assert_eq!(actual.h, expected.h, "{label} H");
    assert_eq!(actual.l, expected.l, "{label} L");
    assert_eq!(actual.sp, expected.sp, "{label} SP");
    assert_eq!(actual.pc, expected.pc, "{label} PC");
    if let Some(ime) = expected.ime {
        assert_eq!(actual.ime, Some(ime), "{label} IME");
    }
    if let Some(ie) = expected.ie {
        assert_eq!(actual.ie, Some(ie), "{label} IE");
    }
    if let Some(ei) = expected.ei {
        assert_eq!(actual.ei, Some(ei), "{label} EI");
    }
    assert_eq!(actual.ram, expected.ram, "{label} RAM snapshot");
}

#[test]
fn sm83_single_step_vectors_pass_on_flat_intent_bus() {
    let mut files = fs::read_dir(assets_dir())
        .expect("SM83 vector assets")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    files.sort();

    let mut passed_files = 0usize;
    for file in files {
        for vector in load_vectors(&file) {
            let mut cpu = Cpu::new();
            let mut bus = FlatIntentBus::new();
            cpu.load_state(&vector.initial, &mut bus);

            run_vector_exact_cycles(&mut cpu, &mut bus, vector.cycles.len());

            let actual = cpu.store_state(&bus, &vector.final_.ram_addrs());
            assert_final_state(
                &actual,
                &vector.final_,
                &format!("{} in {file:?}", vector.name),
            );
            for &(addr, expected) in &vector.final_.ram {
                assert_eq!(bus.peek(addr), expected, "{} RAM[{addr:04X}]", vector.name);
            }
            assert_cycles(&bus, &vector.cycles, &vector.name);
        }
        passed_files += 1;
    }
    assert_eq!(
        passed_files, 499,
        "all checked-in SingleStepTests/sm83 JSON files passed"
    );
}

fn load_program(bus: &mut FlatIntentBus, program: &[u8]) {
    for (addr, byte) in program.iter().copied().enumerate() {
        bus.poke(addr as u16, byte);
    }
}

fn run_until_running_boundary(cpu: &mut Cpu, bus: &mut FlatIntentBus) -> u64 {
    let start = bus.m_cycles();
    for _ in 0..16 {
        cpu.step_m(bus);
        if matches!(cpu.mode, rubc_ng::cpu::CpuMode::Running) && cpu.exec_is_boundary() {
            return bus.m_cycles() - start;
        }
    }
    panic!("CPU did not return to running boundary");
}

#[test]
fn interrupt_dispatch_consumes_five_mcycles_pushes_pc_and_clears_if() {
    let mut cpu = Cpu::new();
    let mut bus = FlatIntentBus::new();
    cpu.set_ime_for_vector(true);
    cpu.r.pc = 0x0150;
    cpu.r.sp = 0xFFFE;
    bus.set_ie(0x05);
    bus.set_if(0xE5);

    let m = run_until_running_boundary(&mut cpu, &mut bus);

    assert_eq!(m, 5, "interrupt dispatch is exactly 5 M-cycles");
    assert_eq!(cpu.r.pc, 0x0040, "lowest pending IRQ vector is serviced");
    assert_eq!(cpu.r.sp, 0xFFFC, "dispatch pushes PC onto stack");
    assert_eq!(bus.peek(0xFFFC), 0x50, "pushed PC low byte");
    assert_eq!(bus.peek(0xFFFD), 0x01, "pushed PC high byte");
    assert_eq!(bus.if_() & 0x05, 0x04, "servicing clears only IRQ bit 0");
    assert!(!cpu.ime, "dispatch clears IME");
    assert!(
        bus.intents()
            .iter()
            .any(|intent| matches!(intent, rubc_ng::CpuBusIntent::IntrPoll)),
        "boundary poll emits an IntrPoll intent before dispatch"
    );
}

#[test]
fn ei_delay_and_halt_bug_have_observable_timing() {
    let mut cpu = Cpu::new();
    let mut bus = FlatIntentBus::new();
    load_program(&mut bus, &[0xFB, 0x00, 0x00]);
    cpu.r.sp = 0xFFFE;
    bus.set_ie(0x01);
    bus.set_if(0xE1);

    assert_eq!(
        cpu.run_one_instruction(&mut bus, FlatIntentBus::m_cycles),
        1,
        "EI cycles"
    );
    assert!(!cpu.ime, "EI does not enable IME immediately");
    assert_eq!(
        cpu.run_one_instruction(&mut bus, FlatIntentBus::m_cycles),
        1,
        "post-EI NOP cycles"
    );
    assert!(
        !cpu.ime,
        "IME remains disabled through the instruction after EI"
    );
    assert_eq!(
        run_until_running_boundary(&mut cpu, &mut bus),
        5,
        "dispatch after EI delay"
    );
    assert_eq!(cpu.r.pc, 0x0040);
    assert_eq!(
        u16::from_le_bytes([bus.peek(cpu.r.sp), bus.peek(cpu.r.sp + 1)]),
        0x0002
    );

    let mut cpu = Cpu::new();
    let mut bus = FlatIntentBus::new();
    load_program(&mut bus, &[0x76, 0x06, 0x42, 0x00]);
    bus.set_ie(0x01);
    bus.set_if(0xE1);

    assert_eq!(
        cpu.run_one_instruction(&mut bus, FlatIntentBus::m_cycles),
        1,
        "HALT bug fetch cycles"
    );
    assert_eq!(cpu.r.pc, 0x0001, "HALT advances PC once");
    assert_eq!(
        cpu.run_one_instruction(&mut bus, FlatIntentBus::m_cycles),
        2,
        "LD B,d8 timing after HALT bug"
    );
    assert_eq!(cpu.r.b, 0x06, "HALT bug reuses opcode byte as operand");
    assert_eq!(cpu.r.pc, 0x0002, "HALT bug leaves PC one byte short");
}
