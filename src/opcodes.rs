use std::collections::HashMap;
use std::sync::OnceLock;

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
    fn new() -> Cpu {
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

pub struct Motherboard {
    cpu: Cpu,
    memory: [u8; 0x10000],
}

impl Motherboard {
    pub fn new() -> Motherboard {
        Motherboard {
            cpu: Cpu::new(),
            memory: [0; 0x10000],
        }
    }
}

type OpCycles = u64;
type OpCodeFunc = fn(mb: &mut Motherboard, value: u16) -> OpCycles;
type OpCodeMap = HashMap<u16, OpCodeFunc>;

const CYCLE_RETURN: OpCycles = 4;

pub fn get_op_code_map() -> &'static OpCodeMap {
    static OPCODES: OnceLock<OpCodeMap> = OnceLock::new();
    OPCODES.get_or_init(|| {
        let mut opcodes = HashMap::new();
        opcodes.insert(0x00 as u16, op_nop as OpCodeFunc);
        opcodes
    })
}

#[inline(always)]
fn op_nop(mb: &mut Motherboard, _value: u16) -> OpCycles {
    mb.cpu.pc += 1;
    CYCLE_RETURN
}
