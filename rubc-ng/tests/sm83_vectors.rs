use rubc_ng::cpu::{Cpu, FlatIntentBus, VectorCpu, VectorState};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Vector {
    name: String,
    initial: VectorState,
    final_: VectorState,
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
        })
        .collect()
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

            cpu.step_instruction(&mut bus);

            let actual = cpu.store_state(&bus, &vector.final_.ram_addrs());
            assert_final_state(
                &actual,
                &vector.final_,
                &format!("{} in {file:?}", vector.name),
            );
            for &(addr, expected) in &vector.final_.ram {
                assert_eq!(bus.peek(addr), expected, "{} RAM[{addr:04X}]", vector.name);
            }
            assert!(
                bus.intents()
                    .iter()
                    .all(|intent| !matches!(intent, rubc_ng::CpuBusIntent::IntrPoll)),
                "slice 1 vectors must not require interrupt integration"
            );
        }
        passed_files += 1;
    }
    assert_eq!(
        passed_files, 499,
        "all checked-in SingleStepTests/sm83 JSON files passed"
    );
}
