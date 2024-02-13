use anyhow::Error;

use crate::{
    globals::*,
    opcodes, opcodes_cb,
    utils::{format_binary, memory_read, rom_read, rom_write, ROM},
};
use std::default::Default;
use std::fmt;

pub struct MotherboardBuilder {
    cpu: Cpu,
    cart: Option<Cartridge>,
    cgb_mode: Option<bool>,
    opcode_map: OpCodeMap,
    opcode_map_cb: OpCodeMap,
}

impl MotherboardBuilder {
    pub fn new() -> MotherboardBuilder {
        MotherboardBuilder {
            cpu: Cpu::default(),
            cart: None,
            cgb_mode: None,
            opcode_map: opcodes::init_opcodes(),
            opcode_map_cb: opcodes_cb::init_opcodes_cb(),
        }
    }

    pub fn build(self) -> Motherboard {
        Motherboard {
            cpu: self.cpu,
            cart: self.cart,
            double_speed: false,
            ime: false,
            cgb_mode: self.cgb_mode.unwrap_or(false),
            opcode_map: self.opcode_map,
            opcode_map_cb: self.opcode_map_cb,
        }
    }

    pub fn with_cart<'a>(mut self, filename: &'a str) -> MotherboardBuilder {
        self.cart = Some(Cartridge::new(filename).unwrap());
        self
    }

    pub fn enable_cgb_mode(mut self) -> MotherboardBuilder {
        self.cgb_mode = Some(true);
        self
    }
}

pub struct Motherboard {
    pub cpu: Cpu,
    pub cart: Option<Cartridge>,
    pub double_speed: bool,
    pub cgb_mode: bool,
    pub ime: bool,
    opcode_map: OpCodeMap,
    opcode_map_cb: OpCodeMap,
}

impl Motherboard {
    pub fn new() -> Motherboard {
        let mut mb = MotherboardBuilder::new()
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

    fn instruction_look_ahead(&self, number: u16) -> String {
        let mut result = Vec::new();
        for i in 0..number {
            result.push(memory_read(self.cpu.pc + i));
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

            let op_code = memory_read(self.cpu.pc);

            let value = match OPCODE_LENGTHS[op_code as usize] {
                1 => 0,
                2 => {
                    self.cpu.pc += 1;
                    memory_read(self.cpu.pc) as u16
                }
                3 => {
                    self.cpu.pc += 1;
                    let low = memory_read(self.cpu.pc) as u16;
                    self.cpu.pc += 1;
                    let high = memory_read(self.cpu.pc) as u16;
                    (high << 8) | low
                }
                _ => panic!("Invalid op code length"),
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
    pub filename: String,
    pub rom_banks: usize,
    pub ram_banks: Option<usize>,
}

impl Cartridge {
    pub fn new(filename: &str) -> anyhow::Result<Cartridge> {
        let rom = std::fs::read(filename).expect("Unable to read file");
        log::debug!("Reading ROM: {}", filename);

        // Cache ROM contents into the global ROM heap
        log::debug!("ROM length: {} bytes", rom.len());
        let calc_rom_banks = rom.len() / ROM_BANK_SIZE;
        log::debug!("Calculated ROM banks: {}", calc_rom_banks);

        log::debug!("Cache ROM into global ROM heap");
        ROM.lock().unwrap()[..rom.len()].copy_from_slice(&rom);

        let ram_banks = match rom_read(CART_SRAM_SIZE) {
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

        let rom_banks = match rom_read(CART_ROM_SIZE) {
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
        let checksum = ROM.lock().unwrap()
            [CART_TITLE_START as usize..CART_MASK_ROM_VERSION_NUMBER as usize]
            .iter()
            .fold(0, |acc: u8, x: &u8| {
                // asdf
                let y = x + 1;
                acc.wrapping_sub(y)
            })
            - 1;

        let header_checksum = rom_read(CART_HEADER_CHECKSUM);

        match header_checksum == checksum {
            true => log::debug!("Checksums match"),
            false => {
                log::error!("Checksums do not match");
                return Err(Error::msg("Checksums do not match"));
            }
        }

        Ok(Cartridge {
            filename: filename.to_string(),
            rom_banks,
            ram_banks,
        })
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
