use crate::mbc::*;
use crate::{globals::*, Error};
use std::fmt::Debug;

pub enum CartType {
    Empty,
    MBC0(MBC0),
}

impl CartType {
    pub fn from(value: u8) -> Self {
        match value {
            0x00 => CartType::MBC0(MBC0::default()),
            _ => CartType::Empty,
        }
    }

    #[inline]
    pub fn read(&self, address: u16) -> u8 {
        // check if reading from ROM vs SRAM
        match address {
            0x0000..=0x7FFF => {
                // ROM bank select
                match self {
                    CartType::MBC0(mbc0) => mbc0.read(address as usize),
                    _ => {
                        log::error!("Invalid cart type");
                        0
                    }
                }
            }
            0xA000..=0xBFFF => {
                // SRAM bank select
                match self {
                    CartType::MBC0(mbc0) => mbc0.read_sram(address as usize - 0xA000),
                    _ => {
                        log::error!("Invalid cart type");
                        0
                    }
                }
            }
            _ => 0,
        }
    }

    #[inline]
    fn write(&mut self, address: u16, value: u8) {
        // check if writing to ROM vs SRAM
        match address {
            0x0000..=0x7FFF => {
                // ROM bank select
                match self {
                    CartType::MBC0(mbc0) => mbc0.write(address as usize, value),
                    _ => log::error!("Invalid cart type"),
                }
            }
            0xA000..=0xBFFF => {
                // SRAM bank select
                match self {
                    CartType::MBC0(mbc0) => mbc0.write_sram(address as usize - 0xA000, value),
                    _ => log::error!("Invalid cart type"),
                }
            }
            _ => log::error!("Invalid cart type"),
        }
    }
}

pub struct Cartridge {
    pub filename: Option<String>,
    pub cart: CartType,
}

impl Cartridge {
    pub fn empty() -> Self {
        Cartridge {
            filename: None,
            cart: CartType::Empty,
        }
    }

    pub fn new(filename: &str) -> anyhow::Result<Self> {
        log::debug!("Reading ROM: {}", filename);

        let rom = std::fs::read(filename).expect("Unable to read file");
        // let mut cached_rom = Vec::new();
        // cached_rom[..rom.len()].copy_from_slice(&rom);

        //     // let mut cart = Cartridge::empty();
        //     cart.rom = cached_rom;

        //     // Cache ROM contents into the global ROM heap
        //     log::debug!("ROM length: {} bytes", rom.len());
        //     let calc_rom_banks = rom.len() / ROM_BANK_SIZE;
        //     log::debug!("Calculated ROM banks: {}", calc_rom_banks);

        //     log::debug!("Cache ROM into global ROM heap");
        //     // ROM.lock().unwrap()[..rom.len()].copy_from_slice(&rom);

        //     let ram_banks = match cart.rom_read(CART_SRAM_SIZE) {
        //         0x00 => None,
        //         0x01 => panic!("Invalid RAM bank size"),
        //         0x02 => Some(1),
        //         0x03 => Some(4),
        //         0x04 => Some(16),
        //         0x05 => Some(8),
        //         _ => panic!("Invalid RAM bank size"),
        //     };

        //     match ram_banks {
        //         Some(ram_banks) => log::debug!("Detected {} RAM banks", ram_banks),
        //         None => log::debug!("No RAM banks detected"),
        //     }

        //     let rom_banks = match cart.rom_read(CART_ROM_SIZE) {
        //         0x00 => 2,
        //         0x01 => 4,
        //         0x02 => 8,
        //         0x03 => 16,
        //         0x04 => 32,
        //         0x05 => 64,
        //         0x06 => 128,
        //         0x07 => 256,
        //         0x08 => 512,
        //         _ => panic!("Invalid ROM bank size"),
        //     };
        //     log::debug!("Detected {} ROM banks", rom_banks);

        //     assert_eq!(rom_banks, calc_rom_banks);

        //     // validate checksum
        //     let checksum = cart.rom[CART_TITLE_START as usize..CART_MASK_ROM_VERSION_NUMBER as usize]
        //         .iter()
        //         .fold(0, |acc: u8, x: &u8| {
        //             // asdf
        //             let y = x + 1;
        //             acc.wrapping_sub(y)
        //         })
        //         - 1;

        //     let header_checksum = cart.rom_read(CART_HEADER_CHECKSUM);

        //     match header_checksum == checksum {
        //         true => log::debug!("Checksums match"),
        //         false => {
        //             log::error!("Checksums do not match");
        //             return Err(Error::msg("Checksums do not match"));
        //         }
        //     }

        //     let sram = vec![0u8; (RAM_BANK_SIZE * RAM_MAX_BANKS) + 1];

        //     cart.rom_banks = rom_banks;
        //     cart.ram_banks = ram_banks;
        //     cart.sram = sram;

        //     match cart.rom_read(CART_TYPE) {
        //         0x00 => {
        //             cart.cart_read_func = mbc0_read;
        //             cart.cart_write_func = mbc0_write;
        //         }
        //         0x01 => {
        //             cart.cart_read_func = mbc0_read;
        //             cart.cart_write_func = mbc0_write;
        //         }
        //         _ => panic!("Unsupported cartridge type"),
        //     }
        //     log::debug!("Cart: {:?}", cart);
        //     Ok(cart)
        Ok(Cartridge {
            filename: Some(filename.to_string()),
            cart: CartType::Empty,
        })
    }
}
