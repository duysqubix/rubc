#![feature(lazy_cell)]

#[macro_use]
pub mod bits;

pub mod cartridge;
pub mod gameboy;
pub mod globals;
pub mod logger;
pub mod opcodes;
pub mod opcodes_cb;

pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;

pub fn format_binary(value: u8) -> String {
    format!("0b{:04b}_{:04b}", value >> 4, value & 0x0F)
}
