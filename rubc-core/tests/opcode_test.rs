use rubc_core::format_binary;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct CpuState {
    // #[serde(deserialize_with = "from_hex")]
    pub a: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub b: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub c: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub d: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub e: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub f: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub h: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub l: u8,

    // #[serde(deserialize_with = "from_hex_16")]
    pub sp: u16,

    // #[serde(deserialize_with = "from_hex_16")]
    pub pc: u16,

    pub ram: Vec<Vec<u16>>,
}

impl fmt::Debug for CpuState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A: `{:#x}` F: `{}` B: `{:#x}` C: `{:#x}` D: `{:#x}` E: `{:#x}` H: `{:#x}` L: `{:#x}` SP: `{:0X}` PC: `{:0X}`",
            self.a,
            format_binary(self.f),
            self.b,
            self.c,
            self.d,
            self.e,
            self.h,
            self.l,
            self.sp,
            self.pc,
        )
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Object {
    pub name: String,
    pub initial: CpuState,

    #[serde(rename = "final")]
    pub final_state: CpuState,
    // cycles: Vec<Vec<String>>,
}

pub fn read_test_file(file: &std::path::Path) -> Vec<Object> {
    let data = fs::read_to_string(file).unwrap();
    let tests: Vec<Object> = serde_json::from_str(&data).unwrap();
    tests
}

#[cfg(test)]
mod tests {

    use super::*;
    use rubc_core::globals::*;
    use rubc_core::mbc::DummyMBC;
    use rubc_core::{cartridge, gameboy, mbc};

    fn set_initial_state(gb: &mut gameboy::Gameboy, state: &CpuState) {
        gb.cpu.a = state.a;
        gb.cpu.b = state.b;
        gb.cpu.c = state.c;
        gb.cpu.d = state.d;
        gb.cpu.e = state.e;
        gb.cpu.f = state.f;
        gb.cpu.h = state.h;
        gb.cpu.l = state.l;
        gb.cpu.sp = state.sp;
        gb.cpu.pc = state.pc;

        for byte in state.ram.iter() {
            let addr = byte[0];
            let value = byte[1];
            // println!("Writing to addr: ${:04X} value: ${:02X}", addr, value);
            gb.memory_write(addr as u16, value as u8);
        }
    }

    fn compare_state(gb: &gameboy::Gameboy, state: &CpuState) -> anyhow::Result<()> {
        // log::trace!("Comparing RAM state");
        for byte in state.ram.iter() {
            let addr = byte[0];
            let value = byte[1];
            let read_value = gb.memory_read(addr);
            if read_value != value as u8 {
                return Err(anyhow::anyhow!(
                    "RAM: Addr: {:04X} Expected: ${:02X} Got: ${:02X}",
                    addr,
                    value,
                    read_value
                ));
            }
        }

        let cp = &gb.cpu;
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

    #[test]
    fn test_opcodes() -> anyhow::Result<()> {
        let test_dir = "../assets/sm83/v1";
        let mut files = std::fs::read_dir(test_dir)?
            .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        files.sort();

        files.iter().for_each(|file| {
            if !file.is_file() {
                return;
            }

            let mut s = format!("Testing OpCode: {:?}.......", file.file_name().unwrap());
            let mut mb = gameboy::GameboyBuilder::new().build();
            mb.cart = cartridge::Cartridge::DummyMBC(DummyMBC::new());
            let tests = read_test_file(file.as_path());

            for test in &tests {
                set_initial_state(&mut mb, &test.initial);

                //sanity check
                assert!(compare_state(&mb, &test.initial)
                    .map_err(|e| {
                        println!("Test: {:?}, E:  {:?}", test.name, e);
                        println!("I: {:?}", test.initial);
                        println!("G: {:?}", mb.cpu);
                    })
                    .is_ok());

                let mut op_idx = 0;
                for ram_state in test.initial.ram.iter().enumerate() {
                    if test.initial.pc == ram_state.1[0] {
                        op_idx = ram_state.0;
                        break;
                    }
                }

                let opcode = test.initial.ram[op_idx][1];
                let opcode_length = OPCODE_LENGTHS[opcode as usize];
                // log::trace!(
                //     "test name: {}, op_idx: {}, opcode: {:02X}, opcode_length: {}",
                //     &test.name,
                //     op_idx,
                //     opcode,
                //     opcode_length
                // );

                let res = match opcode_length {
                    3 => {
                        let opcode = test.initial.ram[op_idx][1];
                        let byte1 = test.initial.ram[op_idx + 1][1];
                        let byte2 = test.initial.ram[op_idx + 2][1];
                        let value = (byte2 << 8) | byte1;
                        // log::trace!(
                        //     "Executing op code: {:02X} with value: {:04X}",
                        //     opcode,
                        //     value
                        // );
                        mb.execute_op_code(opcode as u8, value)
                    }
                    2 => {
                        let opcode = test.initial.ram[op_idx][1];
                        let byte1 = test.initial.ram[op_idx + 1][1];
                        let value = byte1;
                        // log::trace!(
                        //     "Executing op code: {:02X} with value: {:04X}",
                        //     opcode,
                        //     value
                        // );

                        mb.execute_op_code(opcode as u8, value)
                    }
                    1 => {
                        let opcode = test.initial.ram[op_idx][1];
                        // log::trace!("Executing op code: {:02X}", opcode,);

                        mb.execute_op_code(opcode as u8, 0)
                    }
                    _ => panic!("Invalid op code length"),
                };
                if let Err(e) = res {
                    println!("Error: {:?}", e);
                    panic!();
                }

                // log::trace!("Comparing state");
                assert!(compare_state(&mb, &test.final_state)
                    .map_err(|e| {
                        println!(
                            "Test: {:?}, Opcode info: {:X?}, Opcode length: {}, E:  {:?}",
                            test.name, opcode, opcode_length, e
                        );
                        println!("I: {:?}", test.initial);
                        println!("G: {:?}", mb.cpu);
                        println!("E: {:?}", test.final_state);
                    })
                    .is_ok());
            }

            s.push_str(format!("{} tests passed", tests.len()).as_str());
            println!("{}", s);
        });
        Ok(())
    }
}
