#![feature(lazy_cell)]

mod bits;
mod gameboy;
mod globals;
mod logger;
mod opcodes;
mod tests;
mod utils;

use globals::*;

fn set_initial_state(cp: &mut gameboy::Cpu, state: &tests::CpuState) {
    cp.a = state.a;
    cp.b = state.b;
    cp.c = state.c;
    cp.d = state.d;
    cp.e = state.e;
    cp.f = state.f;
    cp.h = state.h;
    cp.l = state.l;
    cp.sp = state.sp;
    cp.pc = state.pc;
}

fn compare_final_state(cp: &gameboy::Cpu, state: &tests::CpuState) -> bool {
    if cp.a != state.a
        || cp.b != state.b
        || cp.c != state.c
        || cp.d != state.d
        || cp.e != state.e
        || cp.f != state.f
        || cp.h != state.h
        || cp.l != state.l
        || cp.sp != state.sp
        || cp.pc != state.pc
    {
        return false;
    }
    true
}

fn str_to_u8(s: &str) -> u8 {
    u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}

fn main() -> anyhow::Result<()> {
    logger::setup_logger()?;

    let mut mb = gameboy::Motherboard::new();
    mb.tick();
    Ok(())
}
