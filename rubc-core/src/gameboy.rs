#![allow(clippy::new_without_default)]

use anyhow::Error;

use crate::{cartridge::Cartridge, format_binary, globals::*, opcodes, opcodes_cb};

use std::default::Default;
use std::fmt;

pub struct GameboyBuilder {
    cpu: Cpu,
    cart: Option<Cartridge>,
    cgb_mode: Option<bool>,
    opcode_map: OpCodeMap,
    opcode_map_cb: OpCodeMap,
}

impl GameboyBuilder {
    pub fn new() -> GameboyBuilder {
        GameboyBuilder {
            cpu: Cpu::default(),
            cart: None,
            cgb_mode: None,
            opcode_map: opcodes::init_opcodes(),
            opcode_map_cb: opcodes_cb::init_opcodes_cb(),
        }
    }

    pub fn build(self) -> Gameboy {
        Gameboy {
            cpu: self.cpu,
            cart: self.cart,
            double_speed: false,
            ime: false,
            cgb_mode: self.cgb_mode.unwrap_or(false),
            opcode_map: self.opcode_map,
            opcode_map_cb: self.opcode_map_cb,
            memory: vec![0u8; u16::MAX as usize + 1],
        }
    }

    pub fn with_cart(mut self, filename: &str) -> GameboyBuilder {
        self.cart = Some(Cartridge::new(filename).unwrap());
        self
    }

    pub fn enable_cgb_mode(mut self) -> GameboyBuilder {
        self.cgb_mode = Some(true);
        self
    }
}

pub struct Gameboy {
    pub cpu: Cpu,
    pub cart: Option<Cartridge>,
    pub double_speed: bool,
    pub cgb_mode: bool,
    pub ime: bool,
    memory: Vec<u8>,
    opcode_map: OpCodeMap,
    opcode_map_cb: OpCodeMap,
}

impl Gameboy {
    pub fn new() -> Gameboy {
        let mut mb = GameboyBuilder::new()
            .with_cart("assests/cpu_instrs.gb")
            .build();
        mb.cpu.reset();
        mb
    }

    pub fn execute_op_code(&mut self, op_code: u8, value: u16) -> anyhow::Result<OpCycles> {
        if ILLEGAL_OPCODES.contains(&op_code) {
            self.cpu.is_stuck = true;
            return Err(Error::msg(format!("Illegal opcode: {:#x}", op_code)));
        }

        match self.opcode_map.get(&op_code) {
            Some(op) => Ok(op(self, value)),
            None => Err(Error::msg(format!("Unexpected opcode: {:#x}", op_code))),
        }
    }

    pub fn execute_op_code_cb(&mut self, op_code: u8) -> anyhow::Result<OpCycles> {
        match self.opcode_map_cb.get(&op_code) {
            Some(op) => Ok(op(self, 0)),
            None => Err(Error::msg(format!("Unexpected opcode: {:#x}", op_code))),
        }
    }

    pub fn memory_write(&mut self, address: u16, value: u8) {
        if address <= ROM1_ADDRESS_END {
            self.cart.as_mut().unwrap().rom_write(address, value);
        } else {
            if value == 0x81 && address == IO_SC {
                print!("{}", self.memory[IO_SB as usize] as char);
            }

            self.memory[address as usize] = value;
        }
    }

    pub fn memory_read(&self, address: u16) -> u8 {
        if address <= ROM1_ADDRESS_END {
            self.cart.as_ref().unwrap().rom_read(address);
            9
        } else {
            self.memory[address as usize]
        }
    }

    fn instruction_look_ahead(&mut self, number: u16) -> String {
        let mut result = Vec::new();
        for i in 0..number {
            result.push(self.memory_read(self.cpu.pc + i));
        }
        format!("{:x?}", result)
    }

    pub fn tick(&mut self) -> anyhow::Result<OpCycles> {
        let mut cycles: OpCycles = 0;
        if !self.cpu.halted {
            // log::debug!(
            //     "PC: {:04X} - {}",
            //     self.cpu.pc,
            //     // self.instruction_look_ahead(3)
            // );

            let op_code = self.memory_read(self.cpu.pc);

            let value = match OPCODE_LENGTHS[op_code as usize] {
                1 => 0,
                2 => {
                    self.cpu.pc += 1;
                    self.memory_read(self.cpu.pc) as u16
                }
                3 => {
                    self.cpu.pc += 1;
                    let low = self.memory_read(self.cpu.pc) as u16;
                    self.cpu.pc += 1;
                    let high = self.memory_read(self.cpu.pc) as u16;
                    (high << 8) | low
                }
                _ => {
                    panic!("Invalid opcode length: {:#x}", op_code)
                }
            };

            cycles = self.execute_op_code(op_code, value)?;
        }
        Ok(cycles)
    }
}

#[derive(Default)]
pub struct Cpu {
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
    pub halted: bool,
    pub is_stuck: bool,
}

impl Cpu {
    pub fn reset(&mut self) {
        // DMG
        self.a = 0x11;
        self.f = 0b1000_0000;
        self.b = 0x00;
        self.c = 0x00;
        self.d = 0xFF;
        self.e = 0x56;
        self.h = 0x00;
        self.l = 0x0D;
        self.sp = 0xFFFE;
        self.pc = 0x0100;
        self.halted = false;
        self.is_stuck = false;
    }
}

impl fmt::Debug for Cpu {
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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_cart_load() {
//         super::super::logger::setup_logger().unwrap();
//         let cart = Cartridge::new("assets/cpu_instrs.gb").unwrap();
//         assert_eq!(cart.rom_banks, 4);
//     }
// }
