#![allow(clippy::new_without_default)]

use anyhow::Error;

use crate::{globals::*, opcodes, opcodes_cb, utils::format_binary};
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
        // log::debug!("Executing op code: CB {:02X}", op_code);
        match self.opcode_map_cb.get(&op_code) {
            Some(op) => Ok(op(self, 0)),
            None => Err(Error::msg(format!("Unexpected opcode: {:#x}", op_code))),
        }
    }

    pub fn memory_write(&mut self, address: u16, value: u8) {
        if address <= ROM1_ADDRESS_END {
            self.cart.as_mut().unwrap().rom_write(address, value);
        } else {
            self.memory[address as usize] = value;
        }
    }

    pub fn memory_read(&self, address: u16) -> u8 {
        if address <= ROM1_ADDRESS_END {
            self.cart.as_ref().unwrap().rom_read(address)
        } else {
            self.memory[address as usize]
        }
    }

    fn instruction_look_ahead(&self, number: u16) -> String {
        let mut result = Vec::new();
        for i in 0..number {
            result.push(self.memory_read(self.cpu.pc + i));
        }
        format!("{:x?}", result)
    }

    pub fn tick(&mut self) -> anyhow::Result<OpCycles> {
        let mut cycles: OpCycles = 0;
        if !self.cpu.halted {
            log::debug!(
                "PC: {:04X} - {}",
                self.cpu.pc,
                self.instruction_look_ahead(3)
            );

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

pub struct Cartridge {
    pub filename: Option<String>,
    pub rom: Vec<u8>,
    pub sram: Vec<u8>,
    pub rom_banks: usize,
    pub ram_banks: Option<usize>,
}

impl Cartridge {
    pub fn rom_write(&mut self, address: u16, value: u8) {
        self.rom[address as usize] = value;
    }

    pub fn rom_read(&self, address: u16) -> u8 {
        self.rom[address as usize]
    }

    pub fn empty() -> Cartridge {
        Cartridge {
            filename: None,
            rom: vec![0u8; (ROM_BANK_SIZE * ROM_MAX_BANKS) + 1],
            sram: vec![0u8; (RAM_BANK_SIZE * RAM_MAX_BANKS) + 1],
            rom_banks: 0,
            ram_banks: None,
        }
    }

    pub fn new(filename: &str) -> anyhow::Result<Cartridge> {
        log::debug!("Reading ROM: {}", filename);

        let rom = std::fs::read(filename).expect("Unable to read file");
        let mut cached_rom = vec![0u8; (ROM_BANK_SIZE * ROM_MAX_BANKS) + 1];
        cached_rom[..rom.len()].copy_from_slice(&rom);

        let mut cart = Cartridge::empty();
        cart.rom = cached_rom;

        // Cache ROM contents into the global ROM heap
        log::debug!("ROM length: {} bytes", rom.len());
        let calc_rom_banks = rom.len() / ROM_BANK_SIZE;
        log::debug!("Calculated ROM banks: {}", calc_rom_banks);

        log::debug!("Cache ROM into global ROM heap");
        // ROM.lock().unwrap()[..rom.len()].copy_from_slice(&rom);

        let ram_banks = match cart.rom_read(CART_SRAM_SIZE) {
            0x00 => None,
            0x01 => panic!("Invalid RAM bank size"),
            0x02 => Some(1),
            0x03 => Some(4),
            0x04 => Some(16),
            0x05 => Some(8),
            _ => panic!("Invalid RAM bank size"),
        };

        match ram_banks {
            Some(ram_banks) => log::debug!("Detected {} RAM banks", ram_banks),
            None => log::debug!("No RAM banks detected"),
        }

        let rom_banks = match cart.rom_read(CART_ROM_SIZE) {
            0x00 => 2,
            0x01 => 4,
            0x02 => 8,
            0x03 => 16,
            0x04 => 32,
            0x05 => 64,
            0x06 => 128,
            0x07 => 256,
            0x08 => 512,
            _ => panic!("Invalid ROM bank size"),
        };
        log::debug!("Detected {} ROM banks", rom_banks);

        assert_eq!(rom_banks, calc_rom_banks);

        // validate checksum
        let checksum = cart.rom[CART_TITLE_START as usize..CART_MASK_ROM_VERSION_NUMBER as usize]
            .iter()
            .fold(0, |acc: u8, x: &u8| {
                // asdf
                let y = x + 1;
                acc.wrapping_sub(y)
            })
            - 1;

        let header_checksum = cart.rom_read(CART_HEADER_CHECKSUM);

        match header_checksum == checksum {
            true => log::debug!("Checksums match"),
            false => {
                log::error!("Checksums do not match");
                return Err(Error::msg("Checksums do not match"));
            }
        }

        let sram = vec![0u8; (RAM_BANK_SIZE * RAM_MAX_BANKS) + 1];

        cart.rom_banks = rom_banks;
        cart.ram_banks = ram_banks;
        cart.sram = sram;
        Ok(cart)
        // Ok(Cartridge {
        //     filename: Some(filename.to_string()),
        //     rom_banks,
        //     ram_banks,
        //     rom: cached_rom,
        //     sram: sram,
        // })
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
