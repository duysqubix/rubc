use crate::{globals::*, mbc::*, utils, Error};
pub enum Cartridge {
    Empty,
    DummyMBC(DummyMBC), // used for testing only
    MBC0(MBC0),
    MBC1(MBC1),
}

impl Cartridge {
    pub fn rom(&self) -> &[u8] {
        match self {
            Self::MBC0(mbc) => &mbc.rom,
            Self::MBC1(mbc) => &mbc.rom,
            _ => panic!("Invalid cart type"),
        }
    }

    pub fn load_rom(&mut self, rom: &[u8]) {
        log::trace!("Loading ROM into cartridge");
        log::trace!("ROM length: {} bytes", rom.len());

        match self {
            Self::MBC0(mbc) => mbc.rom[..rom.len()].copy_from_slice(&rom),
            Self::MBC1(mbc) => mbc.rom[..rom.len()].copy_from_slice(&rom),
            _ => log::error!("Invalid cart type"),
        }
    }

    pub fn rom_banks(&self) -> usize {
        match self {
            Self::MBC0(mbc) => mbc.rom_banks(),
            Self::MBC1(mbc) => mbc.rom_banks(),
            _ => panic!("Invalid cart type"),
        }
    }

    pub fn ram_banks(&self) -> usize {
        match self {
            Self::MBC0(mbc) => mbc.ram_banks(),
            Self::MBC1(mbc) => mbc.ram_banks(),
            _ => panic!("Invalid cart type"),
        }
    }

    #[inline]
    pub fn read(&self, address: u16) -> u8 {
        // check if reading from ROM vs SRAM
        match address {
            0x0000..=0x7FFF => {
                // ROM bank select
                match self {
                    Self::DummyMBC(mbc) => mbc.read(address as usize),
                    Self::MBC0(mbc) => mbc.read(address as usize),
                    Self::MBC1(mbc) => mbc.read(address as usize),
                    _ => {
                        panic!("Cart type not supported for reading ROM bank")
                    }
                }
            }
            0xA000..=0xBFFF => {
                // SRAM bank select
                match self {
                    Self::DummyMBC(mbc) => mbc.read_sram(address as usize - 0xA000),
                    Self::MBC0(mbc) => mbc.read_sram(address as usize - 0xA000),
                    Self::MBC1(mbc) => mbc.read_sram(address as usize - 0xA000),
                    _ => {
                        panic!("Cart type not supported for reading SRAM bank")
                    }
                }
            }
            _ => 0,
        }
    }

    #[inline]
    pub fn write(&mut self, address: u16, value: u8) {
        // check if writing to ROM vs SRAM
        match address {
            0x0000..=0x7FFF => {
                // ROM bank select
                match self {
                    Self::DummyMBC(mbc) => mbc.write(address as usize, value),
                    Self::MBC0(mbc) => mbc.write(address as usize, value),
                    Self::MBC1(mbc) => mbc.write(address as usize, value),
                    _ => panic!("Cart type not supported for writing ROM bank"),
                }
            }
            0xA000..=0xBFFF => {
                // SRAM bank select
                match self {
                    Self::DummyMBC(mbc) => mbc.write_sram(address as usize - 0xA000, value),
                    Self::MBC0(mbc) => mbc.write_sram(address as usize - 0xA000, value),
                    Self::MBC1(mbc) => mbc.write_sram(address as usize - 0xA000, value),
                    _ => panic!("Cart type not supported for writing SRAM bank"),
                }
            }
            _ => log::error!("Invalid cart type"),
        }
    }
    pub fn empty() -> Self {
        Self::Empty
    }

    pub fn new(filename: &str) -> anyhow::Result<Cartridge> {
        log::debug!("Reading ROM: {}", filename);

        let rom = std::fs::read(filename).expect("Unable to read file");

        // Cache ROM contents into the global ROM heap
        log::debug!("ROM length: {} bytes", rom.len());
        let calc_rom_banks = rom.len() / ROM_BANK_SIZE;
        log::debug!("Calculated ROM banks: {}", calc_rom_banks);

        log::debug!("Cache ROM into global ROM heap");
        // ROM.lock().unwrap()[..rom.len()].copy_from_slice(&rom);

        let ram_banks = match rom[CART_SRAM_SIZE as usize] {
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

        let rom_banks = match rom[CART_ROM_SIZE as usize] {
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
        // let checksum = rom[CART_TITLE_START as usize..CART_MASK_ROM_VERSION_NUMBER as usize]
        //     .iter()
        //     .fold(0, |acc: u8, x: &u8| {
        //         // asdf
        //         let y = x + 1;
        //         acc.wrapping_sub(y)
        //     })
        //     - 1;
        let checksum = utils::calculate_checksum(
            &rom[CART_TITLE_START as usize..CART_MASK_ROM_VERSION_NUMBER as usize],
        );

        let header_checksum = rom[CART_HEADER_CHECKSUM as usize];

        match header_checksum == checksum {
            true => log::debug!("Checksums match"),
            false => {
                log::error!("Checksums do not match");
                return Err(Error::msg("Checksums do not match"));
            }
        }

        // let sram = vec![0u8; (RAM_BANK_SIZE * RAM_MAX_BANKS) + 1];
        let mut cart = Cartridge::Empty;

        let cart_type_id = rom[CART_TYPE as usize];
        log::debug!("Cart type: {}", cart_type_map(cart_type_id));
        match cart_type_id {
            0x00 => {
                log::debug!("Initializing MBC0 cartridge type");
                cart = Cartridge::MBC0(MBC0::new());
            }
            0x01 | 0x02 | 0x03 => {
                log::debug!("Initializing MBC1 cartridge type");
                cart = Cartridge::MBC1(MBC1::new(rom_banks, ram_banks.unwrap_or(0)))
            }

            _ => {
                log::error!("Unsupported cartridge type");
                return Err(Error::msg("Unsupported cartridge type"));
            }
        }
        cart.load_rom(&rom);

        let metadata = utils::get_metadata(&cart);
        log::debug!("Cartridge metadata:\n{}", metadata);
        Ok(cart)
    }
}
