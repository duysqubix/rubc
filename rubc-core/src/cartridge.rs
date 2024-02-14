use crate::{globals::*, Error};

type CartReadFunc = fn(&mut Cartridge, u16) -> u8;
type CartWriteFunc = fn(&mut Cartridge, u16, u8);

#[derive(Debug)]
pub struct Cartridge {
    pub filename: Option<String>,
    pub rom: Vec<u8>,
    pub sram: Vec<u8>,
    pub rom_banks: usize,
    pub ram_banks: Option<usize>,
    pub ram_enabled: bool,
    pub rom_bank_select: usize,
    pub ram_bank_select: usize,
    cart_read_func: CartReadFunc,
    cart_write_func: CartWriteFunc,
    memory_model: u8,
}

impl Cartridge {
    pub fn rom_write(&mut self, address: u16, value: u8) {
        (self.cart_write_func)(self, address, value)
    }

    pub fn rom_read(&mut self, address: u16) -> u8 {
        (self.cart_read_func)(self, address)
    }

    pub fn empty() -> Cartridge {
        Cartridge {
            filename: None,
            rom: vec![0u8; (ROM_BANK_SIZE * ROM_MAX_BANKS) + 1],
            sram: vec![0u8; (RAM_BANK_SIZE * RAM_MAX_BANKS) + 1],
            rom_bank_select: 0,
            ram_bank_select: 0,
            rom_banks: 0,
            ram_banks: None,
            ram_enabled: false,
            cart_read_func: mbc0_read,
            cart_write_func: mbc0_write,
            memory_model: 0,
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

        match cart.rom_read(CART_TYPE) {
            0x00 => {
                cart.cart_read_func = mbc0_read;
                cart.cart_write_func = mbc0_write;
            }
            0x01 => {
                cart.cart_read_func = mbc1_read;
                cart.cart_write_func = mbc1_write;
            }
            _ => panic!("Unsupported cartridge type"),
        }
        log::debug!("Cart: {:?}", cart);
        Ok(cart)
    }
}

fn mbc0_read(cart: &mut Cartridge, address: u16) -> u8 {
    log::trace!("mbc0_read: {:04X}", address);
    match address {
        0x0000..=0x7FFF => cart.rom[address as usize],
        _ => 0xFF,
    }
}

fn mbc0_write(_cart: &mut Cartridge, address: u16, value: u8) {
    log::warn!("attempted mbc0_write: {:04X} = {:02X}", address, value);
}

fn mbc1_read(cart: &mut Cartridge, address: u16) -> u8 {
    log::trace!("mbc1_read: {:04X}", address);
    match address {
        0x0000..=0x3FFF => {
            if cart.memory_model == 1 {
                cart.rom_bank_select = (cart.ram_bank_select << 5) & cart.rom_banks;
            } else {
                cart.rom_bank_select = 0;
            }

            let addr_position = (cart.rom_bank_select * ROM_BANK_SIZE) + (address as usize);
            cart.rom[addr_position]
        }
        0x4000..=0x7FFF => {
            let bank = (cart.ram_bank_select << 5) & cart.rom_banks | cart.rom_bank_select;
            let addr_position = (bank * ROM_BANK_SIZE) + (address as usize);
            cart.rom[addr_position]
        }
        0xA000..=0xBFFF => {
            if cart.ram_banks.is_none() || !cart.ram_enabled {
                return 0xFF;
            }

            if cart.memory_model == 1 {
                // cart.ram_bank_select = cart.ram_bank_select;
            } else {
                cart.ram_bank_select = 0;
            }

            let bank = cart.ram_bank_select & cart.ram_banks.unwrap();
            let addr_position = (bank * RAM_BANK_SIZE) + (address as usize);
            cart.sram[addr_position]
        }
        _ => panic!("Invalid address: {:04X}", address),
    }
}

fn mbc1_write(cart: &mut Cartridge, address: u16, value: u8) {
    log::trace!("mbc1_write: {:04X} = {:02X}", address, value);
    match address {
        0x0000..=0x1FFF => cart.ram_enabled = (value & 0x0F) == 0x0A,
        0x2000..=0x3FFF => {
            let mut bank = value & 0x1F;
            if bank == 0 {
                bank = 1;
            }
            cart.rom_bank_select = bank as usize;
        }
        0x4000..=0x5FFF => {
            cart.ram_bank_select = (value as usize) & 0x3;
        }
        0x6000..=0x7FFF => {
            cart.memory_model = value & 0x1;
        }
        0xA000..=0xBFFF => {
            if cart.ram_banks.is_none() || !cart.ram_enabled {
                log::error!("Attempted write to SRAM, but sram not enabled or not present");
                return;
            }

            if cart.memory_model == 1 {
                // cart.ram_bank_select = cart.ram_bank_select;
            } else {
                cart.ram_bank_select = 0;
            }

            let bank = cart.ram_bank_select & cart.ram_banks.unwrap();
            let addr_position = (bank * RAM_BANK_SIZE) + (address as usize);
            cart.sram[addr_position] = value;
        }

        _ => panic!("Writing to invalid address: {:04X}", address),
    }
}
