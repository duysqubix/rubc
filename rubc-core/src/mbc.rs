use crate::globals::*;

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

pub struct DummyMBC {
    pub rom: Vec<u8>,
    pub sram: Vec<u8>,
}

impl DummyMBC {
    pub fn new() -> Self {
        Self {
            rom: vec![0u8; ROM_MAX_BANKS_MBC0 * ROM_BANK_SIZE],
            sram: vec![0u8; RAM_BANK_SIZE * 4],
        }
    }
}

impl IntoMBC for DummyMBC {
    fn read(&self, address: usize) -> u8 {
        self.rom[address]
    }

    fn write(&mut self, address: usize, value: u8) {
        self.rom[address] = value;
    }

    fn read_sram(&self, address: usize) -> u8 {
        self.sram[address]
    }

    fn write_sram(&mut self, address: usize, value: u8) {
        self.sram[address] = value;
    }
}

pub struct MBC0 {
    pub rom: Vec<u8>,
}

impl MBC0 {
    pub fn new() -> Self {
        Self {
            rom: vec![0u8; ROM_MAX_BANKS_MBC0 * ROM_BANK_SIZE],
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
    pub rom: Vec<u8>,
    pub sram: Vec<u8>,
    rom_banks: usize,
    ram_banks: usize,
    ram_enabled: bool,
    rom_bank_select: usize,
    ram_bank_select: usize,
    mode: u8,
}

impl MBC1 {
    pub fn new(rom_banks: usize, ram_banks: usize) -> MBC1 {
        MBC1 {
            rom: vec![0; ROM_MAX_BANKS_MBC1 * ROM_BANK_SIZE],
            sram: vec![0; RAM_BANK_SIZE * RAM_MAX_BANKS_MBC1],
            rom_banks: rom_banks,
            ram_banks: ram_banks,
            ram_enabled: false,
            rom_bank_select: 1,
            ram_bank_select: 0,
            mode: 0,
        }
    }
}

impl IntoMBC for MBC1 {
    fn read(&self, address: usize) -> u8 {
        match address {
            0x0000..=0x3FFF => self.rom[address],
            0x4000..=0x7FFF => self.rom[self.rom_bank_select * ROM_BANK_SIZE + (address - 0x4000)],
            _ => panic!("Invalid address for MBC1 read: {:04X}", address),
        }
    }

    fn write(&mut self, _address: usize, _value: u8) {
        log::warn!("Attempted to write to ROM on MBC0, ignoring");
    }

    fn read_sram(&self, mut address: usize) -> u8 {
        address -= 0xA000;
        if !self.ram_enabled {
            return 0xFF;
        }
        if self.mode == 0 {
            self.sram[self.ram_bank_select * RAM_BANK_SIZE + address]
        } else {
            self.sram[address]
        }
    }

    fn write_sram(&mut self, _address: usize, _value: u8) {
        log::warn!("Attempted to write to SRAM on MBC0, ignoring");
    }
}
