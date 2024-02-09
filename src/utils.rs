use rand::{thread_rng, Rng};
use std::sync::{LazyLock, Mutex};

use crate::globals::*;

pub fn format_binary(value: u8) -> String {
    format!("0b{:04b}_{:04b}", value >> 4, value & 0x0F)
}

pub static ROM: LazyLock<Mutex<Vec<u8>>> = LazyLock::new(|| {
    // initialize memory
    Mutex::new(vec![0u8; ROM_BANK_SIZE * ROM_MAX_BANKS])
});

pub static EXTERNAL_RAM: LazyLock<Mutex<Vec<u8>>> = LazyLock::new(|| {
    // initialize memory
    Mutex::new(vec![0u8; RAM_BANK_SIZE * RAM_MAX_BANKS])
});

pub static MEMORY: LazyLock<Mutex<Vec<u8>>> = LazyLock::new(|| {
    // initialize memory
    let mut memory = vec![0u8; 0xFFFF];
    let mut rng = thread_rng();
    for i in &mut memory {
        *i = rng.gen();
    }
    Mutex::new(memory)
});

#[inline(always)]
pub fn copy_ram_external_to_internal(rambank: usize) {
    MEMORY.lock().unwrap()[EXTERNAL_RAM_ADDRESS_START as usize..EXTERNAL_RAM_ADDRESS_END as usize]
        .copy_from_slice(
            &EXTERNAL_RAM.lock().unwrap()
                [(rambank * RAM_BANK_SIZE)..((rambank + 1) * RAM_BANK_SIZE)],
        );
}

#[inline(always)]
pub fn copy_ram_internal_to_external(rambank: usize) {
    EXTERNAL_RAM.lock().unwrap()[(rambank * RAM_BANK_SIZE)..((rambank + 1) * RAM_BANK_SIZE)]
        .copy_from_slice(
            &MEMORY.lock().unwrap()
                [EXTERNAL_RAM_ADDRESS_START as usize..EXTERNAL_RAM_ADDRESS_END as usize],
        );
}

#[inline(always)]
pub fn memory_write(address: u16, value: u8) {
    // MEMORY.lock().unwrap()[address as usize] = value;
    if address <= ROM1_ADDRESS_END {
        rom_write(address, value);
    } else {
        MEMORY.lock().unwrap()[address as usize] = value;
    }
}

#[inline(always)]
pub fn memory_read(address: u16) -> u8 {
    if address <= ROM1_ADDRESS_END {
        rom_read(address)
    } else {
        MEMORY.lock().unwrap()[address as usize]
    }
}

#[inline(always)]
pub fn rom_write(address: u16, value: u8) {
    ROM.lock().unwrap()[address as usize] = value;
}

#[inline(always)]
pub fn rom_read(address: u16) -> u8 {
    ROM.lock().unwrap()[address as usize]
}
