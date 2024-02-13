#![feature(lazy_cell)]

#[macro_use]
pub mod bits;

pub mod gameboy;
pub mod globals;
pub mod logger;
pub mod opcodes;
pub mod opcodes_cb;

pub mod utils;

pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;
