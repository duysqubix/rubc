#![feature(lazy_cell)]
#[macro_export]
mod bits;
mod gameboy;
mod globals;
mod logger;
mod opcodes;
mod tests;
mod utils;

use globals::*;
use rayon::iter::IntoParallelRefMutIterator;
fn set_initial_state(cp: &mut gameboy::Cpu, state: &tests::CpuState) {
    cp.a = state.a;
    cp.b = state.b;
    cp.c = state.c;
    cp.d = state.d;
    cp.e = state.e;
    cp.f = state.f;
    cp.h = state.h;
    cp.l = state.l;
    cp.sp = state.sp;
    cp.pc = state.pc;
}

fn compare_state(cp: &gameboy::Cpu, state: &tests::CpuState) -> anyhow::Result<()> {
    if cp.a != state.a {
        return Err(anyhow::anyhow!(
            "A: Expected: {:#x} Got: {:#x}",
            state.a,
            cp.a
        ));
    }
    if cp.b != state.b {
        return Err(anyhow::anyhow!(
            "B: Expected: {:#x} Got: {:#x}",
            state.b,
            cp.b
        ));
    }
    if cp.c != state.c {
        return Err(anyhow::anyhow!(
            "C: Expected: {:#x} Got: {:#x}",
            state.c,
            cp.c
        ));
    }
    if cp.d != state.d {
        return Err(anyhow::anyhow!(
            "D: Expected: {:#x} Got: {:#x}",
            state.d,
            cp.d
        ));
    }
    if cp.e != state.e {
        return Err(anyhow::anyhow!(
            "E: Expected: {:#x} Got: {:#x}",
            state.e,
            cp.e
        ));
    }
    if cp.f != state.f {
        return Err(anyhow::anyhow!(
            "F: Expected: {:#x} Got: {:#x}",
            state.f,
            cp.f
        ));
    }
    if cp.h != state.h {
        return Err(anyhow::anyhow!(
            "H: Expected: {:#x} Got: {:#x}",
            state.h,
            cp.h
        ));
    }
    if cp.l != state.l {
        return Err(anyhow::anyhow!(
            "L: Expected: {:#x} Got: {:#x}",
            state.l,
            cp.l
        ));
    }
    if cp.sp != state.sp {
        return Err(anyhow::anyhow!(
            "SP: Expected: {:#x} Got: {:#x}",
            state.sp,
            cp.sp
        ));
    }
    if cp.pc != state.pc {
        return Err(anyhow::anyhow!(
            "PC: Expected: {:#x} Got: {:#x}",
            state.pc,
            cp.pc
        ));
    }
    Ok(())
}

fn str_to_u8(s: &str) -> u8 {
    u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}

fn main() -> anyhow::Result<()> {
    logger::setup_logger()?;

    let test_dir = "/home/duys/.repos/jsmoo/misc/tests/GeneratedTests/sm83/v1";
    let mut files = std::fs::read_dir(test_dir)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    files.sort();
    use rayon::prelude::*;
    use std::sync::{Arc, Mutex};
    let errors: Arc<Mutex<Vec<anyhow::Error>>> = Arc::new(Mutex::new(Vec::new()));

    files.par_iter().for_each(|file| {
        println!("{:?}", file);
        let error_copy = errors.clone();
        let mut mb = gameboy::MotherboardBuilder::new().build();

        let tests = tests::read_test_file(file.as_path());

        for test in tests {
            set_initial_state(&mut mb.cpu, &test.initial);

            //sanity check
            assert!(compare_state(&mb.cpu, &test.initial).is_ok());

            let opcode_info = test.name.split(' ').collect::<Vec<&str>>();
            let opcode_length = OPCODE_LENGTHS[str_to_u8(opcode_info[0]) as usize];

            match opcode_length {
                3 => {
                    let opcode = test.initial.ram[0][1];
                    let byte1 = test.initial.ram[1][1];
                    let byte2 = test.initial.ram[2][1];
                    let value = (byte2 << 8) | byte1;
                    mb.execute_op_code(opcode as u8, value);
                }
                2 => {
                    let opcode = test.initial.ram[0][1];
                    let byte1 = test.initial.ram[1][1];
                    let value = byte1;
                    mb.execute_op_code(opcode as u8, value as u16);
                }
                1 => {
                    let opcode = test.initial.ram[0][1];
                    mb.execute_op_code(opcode as u8, 0);
                }
                _ => panic!("Invalid op code length"),
            }
            compare_state(&mb.cpu, &test.final_state)
                .inspect_err(|_| {
                    log::debug!(
                        "Opcode info: {:?}, Opcode length: {}",
                        opcode_info,
                        opcode_length
                    );
                    log::debug!("I: {:?}", test.initial);
                    log::debug!("G: {:?}", mb.cpu);
                    log::debug!("E: {:?}", test.final_state);
                })
                .map_err(|e| {
                    error_copy.lock().unwrap().push(e);
                })
                .unwrap_or_else(|_| ());
            // break;
        }
    });
    println!("ERRORS FOUND: {:?}", errors.lock().unwrap());
    Ok(())
}
