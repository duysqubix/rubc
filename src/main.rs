#![feature(lazy_cell)]

#[macro_use]
mod bits;

mod gameboy;
mod globals;
mod logger;
mod opcodes;
mod opcodes_cb;
mod tests;
mod utils;

fn main() -> anyhow::Result<()> {
    logger::setup_logger()?;

    Ok(())
}
