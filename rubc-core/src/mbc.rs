use crate::globals::{
    RAM_BANK_SIZE, RAM_MAX_BANKS_MBC1, ROM_BANK_SIZE, ROM_MAX_BANKS, ROM_MAX_BANKS_MBC1,
};

pub trait IntoMBC {
    // address gaurenteed to be in range 0x0000..=0x7FFF
    fn read(&self, address: usize) -> u8;

    // address gaurenteed to be in range 0x0000..=0x7FFF
    fn write(&mut self, address: usize, value: u8);

    // address gaurenteed to be in range 0xA000..=0xBFFF
    fn read_sram(&self, address: usize) -> u8;

    // address gaurenteed to be in range 0xA000..=0xBFFF
    fn write_sram(&mut self, address: usize, value: u8);
}

pub struct MBC0 {
    rom: Vec<u8>,
}

impl std::default::Default for MBC0 {
    fn default() -> MBC0 {
        MBC0 {
            rom: vec![0; 0x8000],
        }
    }
}

impl IntoMBC for MBC0 {
    fn read(&self, address: usize) -> u8 {
        self.rom[address]
    }

    fn write(&mut self, _address: usize, _value: u8) {
        log::warn!("Attempted to write to ROM on MBC0, ignoring");
    }

    fn read_sram(&self, _address: usize) -> u8 {
        0xFF
    }

    fn write_sram(&mut self, _address: usize, _value: u8) {
        log::warn!("Attempted to write to SRAM on MBC0, ignoring");
    }
}

pub struct MBC1 {
    rom: Vec<u8>,
    sram: Vec<u8>,
    rom_banks: usize,
    ram_banks: Option<usize>,
    ram_enabled: bool,
    rom_bank_select: usize,
    ram_bank_select: usize,
    mode: u8,
}

impl std::default::Default for MBC1 {
    fn default() -> MBC1 {
        MBC1 {
            rom: vec![0; ROM_MAX_BANKS_MBC1 * ROM_BANK_SIZE],
            sram: vec![0; RAM_BANK_SIZE * RAM_MAX_BANKS_MBC1],
            rom_banks: 2,
            ram_banks: None,
            ram_enabled: false,
            rom_bank_select: 1,
            ram_bank_select: 0,
            mode: 0,
        }
    }
}
