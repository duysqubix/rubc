use crate::opcodes;
use std::default::Default;

pub struct Motherboard {
    pub cpu: Cpu,
    memory: [u8; 0x10000],
    opcode_map: opcodes::OpCodeMap,
}

impl<'a> Motherboard {
    pub fn new() -> Motherboard {
        Motherboard {
            cpu: Cpu::default(),
            memory: [0; 0x10000],
            opcode_map: opcodes::init_opcodes(),
        }
    }

    pub fn execute_op_code(&mut self, op_code: u16) -> opcodes::OpCycles {
        self.opcode_map.get(&op_code).unwrap()(self, op_code)
    }
}

pub struct Cpu {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,
}

impl Cpu {
    pub fn incr_pc(&mut self, value: u16) {
        self.pc += value;
    }

    pub fn set_bc(&mut self, value: u16) {
        self.b = ((value & 0xFF00) >> 8) as u8;
        self.c = (value & 0x00FF) as u8;
    }
}

impl<'a> Default for Cpu {
    fn default() -> Cpu {
        Cpu {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            f: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0,
        }
    }
}
