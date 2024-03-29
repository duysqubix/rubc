#![allow(clippy::new_without_default)]

use anyhow::Error;

use crate::{bits, cartridge::Cartridge, format_binary, globals::*, opcodes, opcodes_cb, utils};

use std::default::Default;
use std::{fmt, io, io::Write};

pub struct GameboyBuilder {
    cpu: Cpu,
    cart: Option<Cartridge>,
    cgb_mode: Option<bool>,
    breakpoints: Option<Vec<usize>>,
    opcode_map: OpCodeMap,
    opcode_map_cb: OpCodeMap,
    test_mode: bool,
    panic_on_stuck: bool,
}

impl GameboyBuilder {
    pub fn new() -> GameboyBuilder {
        GameboyBuilder {
            cpu: Cpu::default(),
            cart: None,
            cgb_mode: None,
            opcode_map: opcodes::init_opcodes(),
            opcode_map_cb: opcodes_cb::init_opcodes_cb(),
            breakpoints: None,
            test_mode: false,
            panic_on_stuck: false,
        }
    }

    pub fn build(mut self) -> Gameboy {
        self.cpu.reset();
        println!("Panic on stuck: {}", self.panic_on_stuck);
        Gameboy {
            cpu: self.cpu,
            cart: self.cart.unwrap_or(Cartridge::empty()),
            double_speed: false,
            cgb_mode: self.cgb_mode.unwrap_or(false),
            opcode_map: self.opcode_map,
            opcode_map_cb: self.opcode_map_cb,
            memory: vec![0u8; u16::MAX as usize + 1],
            interrupt_enabling: false,
            interrupts_on: false,
            timer_div_counter: 0,
            timer_tima_counter: 0,
            breakpoints: self.breakpoints.unwrap_or_default(),
            test_mode: self.test_mode,
            panic_on_stuck: self.panic_on_stuck,
        }
    }

    pub fn with_cart(mut self, filename: &str) -> anyhow::Result<GameboyBuilder> {
        self.cart = Some(Cartridge::new(filename)?);
        Ok(self)
    }

    pub fn set_cart(mut self, cart: Cartridge) -> GameboyBuilder {
        self.cart = Some(cart);
        self
    }

    pub fn enable_cgb_mode(mut self) -> GameboyBuilder {
        self.cgb_mode = Some(true);
        self
    }

    pub fn with_cpu_breakpoints(mut self, breakpoints: Vec<usize>) -> GameboyBuilder {
        self.breakpoints = Some(breakpoints);
        self
    }

    pub fn enable_test_mode(mut self) -> GameboyBuilder {
        self.test_mode = true;
        self
    }

    pub fn panic_on_stuck(mut self) -> GameboyBuilder {
        self.panic_on_stuck = true;
        self
    }
}

pub struct Gameboy {
    pub cpu: Cpu,
    pub cart: Cartridge,
    pub double_speed: bool,
    pub cgb_mode: bool,
    pub interrupt_enabling: bool,
    pub interrupts_on: bool,
    pub timer_div_counter: OpCycles,
    pub timer_tima_counter: OpCycles,
    memory: Vec<u8>,
    opcode_map: OpCodeMap,
    opcode_map_cb: OpCodeMap,
    breakpoints: Vec<usize>,
    test_mode: bool,
    panic_on_stuck: bool,
}

impl Gameboy {
    #[inline]
    fn timer_enabled(&self) -> bool {
        bits::is_bit_set(self.memory[IO_TAC as usize], 2)
    }

    fn get_clock_freq_count(&self) -> OpCycles {
        match self.memory[IO_TAC as usize] & 0x3 {
            0 => 1024,
            1 => 16,
            2 => 64,
            3 => 256,
            _ => 0,
        }
    }

    fn update_timer_divider(&mut self, cycles: OpCycles) {
        let ds = if self.double_speed { 2 } else { 1 };

        let max_div_cycles = DMG_CLOCK_SPEED / GB_TIMER_FREQ * ds;
        self.timer_div_counter += cycles;

        if self.timer_div_counter >= max_div_cycles {
            self.timer_div_counter -= max_div_cycles;
            self.memory[IO_DIV as usize] = self.memory[IO_DIV as usize].wrapping_add(1);
        }
    }

    fn handle_timer(&mut self, cycles: OpCycles) {
        self.update_timer_divider(cycles);

        if self.timer_enabled() {
            self.timer_tima_counter += cycles;
            let freq = self.get_clock_freq_count();

            while self.timer_tima_counter >= freq {
                self.timer_tima_counter -= freq;
                if self.memory[IO_TIMA as usize] == 0xFF {
                    self.memory[IO_TIMA as usize] = self.memory[IO_TMA as usize];
                    // self.service_interrupt(INTR_TIMER_POS);
                    // let req = self.memory_read(IO_IF) | 0xE0;
                    bits::set_bit(&mut self.memory[IO_IF as usize], INTR_TIMER_POS);
                    break;
                } else {
                    self.memory[IO_TIMA as usize] = self.memory[IO_TIMA as usize].wrapping_add(1);
                }
            }
        }
    }

    pub fn new() -> Gameboy {
        let mut mb = GameboyBuilder::new()
            .with_cart("assests/cpu_instrs.gb")
            .expect("Failed to load cart")
            .build();
        mb.cpu.reset();
        mb
    }

    pub fn execute_op_code(&mut self, op_code: u8, value: u16) -> anyhow::Result<OpCycles> {
        if ILLEGAL_OPCODES.contains(&op_code) {
            self.cpu.is_stuck = true;
            return Err(Error::msg(format!("Illegal opcode: {:#x}", op_code)));
        }

        match self.opcode_map.get(&op_code) {
            Some(op) => Ok(op(self, value)),
            None => Err(Error::msg(format!("Unexpected opcode: {:#x}", op_code))),
        }
    }

    pub fn execute_op_code_cb(&mut self, op_code: u8) -> anyhow::Result<OpCycles> {
        match self.opcode_map_cb.get(&op_code) {
            Some(op) => Ok(op(self, 0)),
            None => Err(Error::msg(format!("Unexpected opcode: {:#x}", op_code))),
        }
    }

    pub fn memory_write(&mut self, address: u16, value: u8) {
        if self.test_mode {
            match address {
                ROM_ADDRESS_START..=ROM1_ADDRESS_END => {
                    self.cart.write(address, value);
                }
                _ => {
                    self.memory[address as usize] = value;
                }
            }
            return;
        }

        match address {
            ROM_ADDRESS_START..=ROM1_ADDRESS_END => {
                self.cart.write(address, value);
            }
            EXTERNAL_RAM_ADDRESS_END..=EXTERNAL_RAM_ADDRESS_END => {
                self.cart.write(address, value);
            }

            IO_DIV => {
                self.timer_tima_counter = 0;
                self.timer_div_counter = 0;
                self.memory[address as usize] = 0;
            }
            IO_TAC => {
                // log::debug!("Setting TAC, value: {:#x}", value);
                let current_freq = self.memory[address as usize] & 0x3;
                self.memory[address as usize] = value | 0xF8;
                let new_freq = self.memory[address as usize] & 0x3;

                if current_freq != new_freq {
                    self.timer_tima_counter = 0;
                }
            }
            _ => {
                self.memory[address as usize] = value;
            }
        }

        if value == 0x81 && address == IO_SC {
            print!("{}", self.memory[IO_SB as usize] as char);
            io::stdout().flush().unwrap();
        }
    }

    pub fn memory_read(&self, address: u16) -> u8 {
        if self.test_mode {
            return match address {
                ROM_ADDRESS_START..=ROM1_ADDRESS_END => self.cart.read(address),
                EXTERNAL_RAM_ADDRESS_END..=EXTERNAL_RAM_ADDRESS_END => self.cart.read(address),
                _ => self.memory[address as usize],
            };
        }

        match address {
            ROM_ADDRESS_START..=ROM1_ADDRESS_END => self.cart.read(address),
            EXTERNAL_RAM_ADDRESS_END..=EXTERNAL_RAM_ADDRESS_END => self.cart.read(address),
            IO_LY => 0x90,
            _ => self.memory[address as usize],
        }
    }

    fn instruction_look_behind(&self, number: u16) -> String {
        let mut result = Vec::new();
        for i in 0..number {
            let pc = self.cpu.pc.wrapping_sub(i);
            result.push(self.memory_read(pc));
        }
        format!("{:02X?}", result)
    }

    fn instruction_look_ahead(&self, number: u16) -> String {
        let mut result = Vec::new();
        for i in 0..number {
            let pc = self.cpu.pc.wrapping_add(i);
            result.push(self.memory_read(pc));
        }
        format!("{:02X?}", result)
    }

    fn cpu_state_snapshot(&self) -> String {
        format!(
            "A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X} SP: {:04X} PC: {:02X}:{:04X} {}|{}",
            self.cpu.a,
            self.cpu.f,
            self.cpu.b,
            self.cpu.c,
            self.cpu.d,
            self.cpu.e,
            self.cpu.h,
            self.cpu.l,
            self.cpu.sp,
            utils::rom_bank_from_address(self.cpu.pc.into(), self.cart.rom_banks()),
            self.cpu.pc,
            self.instruction_look_behind(4),
            self.instruction_look_ahead(4)
        )
    }

    #[inline]
    fn log_state(&self) {
        if !self.breakpoints.is_empty() && self.breakpoints.contains(&(self.cpu.pc as usize)) {
            log::debug!("{}", self.cpu_state_snapshot());
        }
        // let flatten_expr = |addr: u16, bank: usize| -> usize { (bank * ROM_BANK_SIZE) + addr as usize };

        // utils::write_to_file(&self.cpu_state_snapshot());
    }

    pub fn tick(&mut self) -> anyhow::Result<OpCycles> {
        let mut cycles: OpCycles = 4;

        if self.cpu.stopped || self.cpu.is_stuck {
            return Ok(cycles);
        }

        if !self.cpu.halted {
            // Tick CPU
            let old_pc = self.cpu.pc;
            let old_sp = self.cpu.sp;
            cycles = {
                let op_code = self.memory_read(self.cpu.pc);

                self.log_state();

                let value = match OPCODE_LENGTHS[op_code as usize] {
                    1 => 0,
                    2 => self.memory_read(self.cpu.pc + 1) as u16,
                    3 => {
                        let low = self.memory_read(self.cpu.pc + 1) as u16;
                        let high = self.memory_read(self.cpu.pc + 2) as u16;
                        (high << 8) | low
                    }
                    _ => {
                        log::error!("Current State: {}", self.cpu_state_snapshot());
                        panic!("Invalid opcode length: {:#x}", op_code);
                    }
                };

                // std::thread::sleep(std::time::Duration::from_millis(100));
                self.execute_op_code(op_code, value)?
            };

            if !self.cpu.is_stuck
                && (old_pc == self.cpu.pc)
                && (old_sp == self.cpu.sp)
                && !self.cpu.is_stuck
            {
                log::warn!("Stuck CPU: {:#x}", old_pc);
                self.cpu.is_stuck = true;
                if self.panic_on_stuck {
                    std::process::exit(0);
                }
            }
        }
        // Tick Cart (RTC)
        // Tick Timer
        self.handle_timer(cycles);
        // Tick PPU
        // Tick Interrupts
        cycles += self.handle_interrupts();
        Ok(cycles)
    }

    fn handle_interrupts(&mut self) -> OpCycles {
        if self.interrupt_enabling {
            self.interrupts_on = true;
            self.interrupt_enabling = false;
            log::trace!("Interrupts enabled");
            return 0;
        }
        if !self.interrupts_on && !self.cpu.halted {
            return 0;
        }

        let req = self.memory_read(IO_IF) | 0xE0;
        let enabled = self.memory_read(IO_IE);

        if req > 0 {
            for i in 0..5 {
                if bits::is_bit_set(req, i as u8) && bits::is_bit_set(enabled, i as u8) {
                    self.service_interrupt(i as u8);
                    return 20;
                }
            }
        }

        0
    }

    fn service_interrupt(&mut self, interrupt: u8) {
        if self.cpu.halted {
            self.cpu.halted = false;
            self.cpu.pc += 1;
            return;
        }

        if self.interrupts_on {
            log::trace!("Servicing interrupt: {:#x}", interrupt);
            self.interrupts_on = false;
            self.cpu.halted = false;
            bits::clear_bit(&mut self.memory[IO_IF as usize], interrupt);
            let sp = self.cpu.sp;
            let pc = self.cpu.pc;

            self.memory_write(sp - 1, ((pc & 0xff00) >> 8) as u8);
            self.memory_write(sp - 2, (pc & 0xff) as u8);
            self.cpu.sp -= 2;
            self.cpu.pc = utils::interrupt_address(interrupt);
        }
    }
}

#[derive(Default)]
pub struct Cpu {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub halted: bool,
    pub is_stuck: bool,
    pub stopped: bool,
}

impl Cpu {
    pub fn reset(&mut self) {
        // DMG
        self.a = 0x01;
        self.f = 0xB0;
        self.b = 0x00;
        self.c = 0x13;
        self.d = 0x00;
        self.e = 0xD8;
        self.h = 0x01;
        self.l = 0x4D;
        self.sp = 0xFFFE;
        self.pc = 0x0100;
        self.halted = false;
        self.is_stuck = false;
    }
}

impl fmt::Debug for Cpu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A: `{:#x}` F: `{}` B: `{:#x}` C: `{:#x}` D: `{:#x}` E: `{:#x}` H: `{:#x}` L: `{:#x}` SP: `{:0X}` PC: `{:0X}`",
            self.a,
            format_binary(self.f),
            self.b,
            self.c,
            self.d,
            self.e,
            self.h,
            self.l,
            self.sp,
            self.pc,
        )
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_cart_load() {
//         super::super::logger::setup_logger().unwrap();
//         let cart = Cartridge::new("assets/cpu_instrs.gb").unwrap();
//         assert_eq!(cart.rom_banks, 4);
//     }
// }
