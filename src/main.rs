#![feature(lazy_cell)]

#[macro_use]
mod bits;

mod gameboy;
mod globals;
mod logger;
mod opcodes;
mod opcodes_cb;
mod tests;
mod utils;

use globals::*;
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

    for byte in state.ram.iter() {
        let addr = byte[0];
        let value = byte[1];
        log::trace!("Writing to addr: ${:04X} value: ${:02X}", addr, value);
        utils::memory_write(addr, value as u8);
    }
}

fn compare_state(cp: &gameboy::Cpu, state: &tests::CpuState) -> anyhow::Result<()> {
    log::trace!("Comparing RAM state");
    for byte in state.ram.iter() {
        let addr = byte[0];
        let value = byte[1];
        let read_value = utils::memory_read(addr);
        if read_value != value as u8 {
            return Err(anyhow::anyhow!(
                "RAM: Expected: ${:02X} Got: ${:02X}",
                value,
                read_value
            ));
        }
    }

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

fn main() -> anyhow::Result<()> {
    logger::setup_logger()?;

    let test_dir = "/home/duys/Documents/sm83/v1";
    let mut files = std::fs::read_dir(test_dir)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    files.sort();
    use rayon::prelude::*;
    use std::sync::{Arc, Mutex};
    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    files.iter().for_each(|file| {
        if !file.is_file() {
            return;
        }

        // if file.file_name().unwrap().to_str().unwrap().contains("cb ") {
        //     return;
        // }

        let mut s = format!("Testing OpCode: {:?}.......", file.file_name().unwrap());
        let error_copy = errors.clone();
        let mut mb = gameboy::MotherboardBuilder::new().build();

        let tests = tests::read_test_file(file.as_path());

        for test in &tests {
            set_initial_state(&mut mb.cpu, &test.initial);

            //sanity check
            assert!(compare_state(&mb.cpu, &test.initial).is_ok());

            let mut op_idx = 0;
            for ram_state in test.initial.ram.iter().enumerate() {
                if test.initial.pc == ram_state.1[0] {
                    op_idx = ram_state.0;
                    break;
                }
            }

            let opcode = test.initial.ram[op_idx][1];
            let opcode_length = OPCODE_LENGTHS[opcode as usize];
            log::trace!(
                "test name: {}, op_idx: {}, opcode: {:02X}, opcode_length: {}",
                &test.name,
                op_idx,
                opcode,
                opcode_length
            );

            let res = match opcode_length {
                3 => {
                    let opcode = test.initial.ram[op_idx][1];
                    let byte1 = test.initial.ram[op_idx + 1][1];
                    let byte2 = test.initial.ram[op_idx + 2][1];
                    let value = (byte2 << 8) | byte1;
                    log::trace!(
                        "Executing op code: {:02X} with value: {:04X}",
                        opcode,
                        value
                    );
                    mb.execute_op_code(opcode as u8, value)
                }
                2 => {
                    let opcode = test.initial.ram[op_idx][1];
                    let byte1 = test.initial.ram[op_idx + 1][1];
                    let value = byte1;
                    log::trace!(
                        "Executing op code: {:02X} with value: {:04X}",
                        opcode,
                        value
                    );

                    mb.execute_op_code(opcode as u8, value)
                }
                1 => {
                    let opcode = test.initial.ram[op_idx][1];
                    log::trace!("Executing op code: {:02X}", opcode,);

                    mb.execute_op_code(opcode as u8, 0)
                }
                _ => panic!("Invalid op code length"),
            };
            if let Err(e) = res {
                error_copy.lock().unwrap().push(e.to_string());
                log::error!("Error: {:?}", e);
                panic!();
                // return;
            }

            log::trace!("Comparing state");
            compare_state(&mb.cpu, &test.final_state)
                .map_err(|e| {
                    log::debug!(
                        "Test: {:?}, Opcode info: {:X?}, Opcode length: {}, E:  {:?}",
                        test.name,
                        opcode,
                        opcode_length,
                        e
                    );
                    log::debug!("I: {:?}", test.initial);
                    log::debug!("G: {:?}", mb.cpu);
                    log::debug!("E: {:?}", test.final_state);
                    error_copy.lock().unwrap().push(e.to_string());
                })
                .unwrap();
        }

        s.push_str(format!("{} tests passed", tests.len()).as_str());
        log::debug!("{}", s);
    });
    println!("ERRORS FOUND: {:?}", errors.lock().unwrap());
    Ok(())
}
