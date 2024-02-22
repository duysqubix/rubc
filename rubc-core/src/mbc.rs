use crate::globals::*;
use crate::utils;

pub trait IntoMBC {
    // address gaurenteed to be in range 0x0000..=0x7FFF
    fn read(&self, address: usize) -> u8;

    // address gaurenteed to be in range 0x0000..=0x7FFF
    fn write(&mut self, address: usize, value: u8);

    // address gaurenteed to be in range 0xA000..=0xBFFF
    fn read_sram(&self, address: usize) -> u8;

    // address gaurenteed to be in range 0xA000..=0xBFFF
    fn write_sram(&mut self, address: usize, value: u8);

    // calculate the absolute address from bank and address
}

pub struct DummyMBC {
    pub rom: Box<[u8]>,
    pub sram: Box<[u8]>,
}

impl DummyMBC {
    pub fn new() -> Self {
        Self {
            rom: vec![0u8; ROM_MAX_BANKS_MBC0 * ROM_BANK_SIZE].into_boxed_slice(),
            sram: vec![0u8; RAM_BANK_SIZE * 4].into_boxed_slice(),
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
    pub rom: Box<[u8]>,
}
impl MBC0 {
    pub fn new() -> Self {
        Self {
            rom: vec![0u8; ROM_MAX_BANKS_MBC0 * ROM_BANK_SIZE].into_boxed_slice(),
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
    pub rom: Box<[u8]>,
    pub sram: Box<[u8]>,
    rom_banks: usize,
    ram_banks: usize,
    ram_enabled: bool,
    rom_bank_select: usize,
    ram_bank_select: usize,
    rom_mask: u8,
    mode: u8,
}

impl MBC1 {
    pub fn new(rom_banks: usize, ram_banks: usize) -> MBC1 {
        let rom_mask = match rom_banks {
            128 => 0b00011111,
            64 => 0b00011111,
            32 => 0b00011111,
            16 => 0b00001111,
            8 => 0b00000111,
            4 => 0b00000011,
            2 => 0b00000001,
            _ => panic!("Invalid number of ROM banks: {}", rom_banks),
        };

        MBC1 {
            rom: vec![0; ROM_MAX_BANKS_MBC1 * ROM_BANK_SIZE].into_boxed_slice(),
            sram: vec![0; RAM_BANK_SIZE * RAM_MAX_BANKS_MBC1].into_boxed_slice(),
            rom_banks: rom_banks,
            ram_banks: ram_banks,
            ram_enabled: false,
            rom_bank_select: 1,
            ram_bank_select: 0,
            mode: 0,
            rom_mask: rom_mask,
        }
    }
}

impl IntoMBC for MBC1 {
    fn read(&self, address: usize) -> u8 {
        match address {
            0x0000..=0x3FFF => {
                let mut bank = 0;
                if self.mode == 1 {
                    bank = (self.ram_bank_select << 5) & self.rom_banks
                }
                self.rom[utils::absolute_address(bank, address)]
            }
            0x4000..=0x7FFF => {
                let bank = (self.ram_bank_select << 5) % self.rom_banks | self.rom_bank_select;
                self.rom[utils::absolute_address(bank, address)]
            }
            _ => panic!("Invalid ROM address for MBC1 read: {:04X}", address),
        }
    }

    fn read_sram(&self, address: usize) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }

        match self.mode {
            0 => self.sram[utils::absolute_address(0 % self.ram_banks, address)],
            1 => self.sram[utils::absolute_address(self.ram_bank_select % self.ram_banks, address)],
            _ => panic!("Memory bank controller mode not supported: {}", self.mode),
        }
    }

    fn write(&mut self, address: usize, value: u8) {
        match address {
            0..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                if value == 0 {
                    self.rom_bank_select = 1;
                } else {
                    self.rom_bank_select = (value & self.rom_mask) as usize;
                }
            }
            0x4000..=0x5FFF => {
                self.ram_bank_select = (value & 0x03) as usize;
            }
            0x6000..=0x7FFF => {
                self.mode = value & 0x01;
            }
            _ => panic!(
                "Invalid ROM address for MBC1 write: {:04X}={:02X}",
                address, value
            ),
        }
    }

    fn write_sram(&mut self, address: usize, value: u8) {
        match address {
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return;
                }

                let mut bank = 0;
                if self.mode == 1 {
                    bank = self.ram_bank_select;
                }
                log::trace!("Writing to SRAM: {:04X}={:02X}", address, value);
                self.sram[utils::absolute_address(bank, address - 0xA000) % self.ram_banks] = value;
            }
            _ => panic!(
                "Invalid SRAM address for MBC1 write: {:04X}={:02X}",
                address, value
            ),
        }
    }
}
