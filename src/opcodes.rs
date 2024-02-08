use crate::motherboard::Motherboard;
use std::collections::HashMap;

pub type OpCycles = u64;
type OpCodeFunc = fn(mb: &mut Motherboard, value: u16) -> OpCycles;
pub type OpCodeMap = HashMap<u16, OpCodeFunc>;

const CYCLE_RETURN: OpCycles = 4;

pub fn init_opcodes() -> phf::Map<u16, OpCodeFunc> {
    phf::phf_map! {
        0x00u16 => |mb: &mut Motherboard, _value: u16| -> OpCycles {
            mb.cpu.incr_pc(1);
            CYCLE_RETURN
        },
        0x01u16 => |mb: &mut Motherboard, value: u16| -> OpCycles {
            mb.cpu.set_bc(value);
            mb.cpu.incr_pc(3);
            CYCLE_RETURN * 3
        },
    }
}
