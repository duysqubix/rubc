//! M-cycle-stepped SM83 CPU core.
//!
//! The CPU advances one bus M-cycle per [`step_m`](Cpu::step_m) call, driving a
//! [`CpuBus`](crate::bus::CpuBus) through the locked per-M-cycle invariant. ALU
//! flag logic lives in [`alu`] as pure, side-effect-free helpers; the state
//! machine + opcode decode live here.

pub mod alu;
pub mod core;
pub mod opcodes;
pub mod opcodes_cb;
pub mod regs;

#[cfg(test)]
pub(crate) mod equiv_harness;
#[cfg(test)]
mod interrupts_test;
#[cfg(test)]
mod mcycle;

pub use alu::Flags;
pub use core::{ActiveCpuCycle, Cpu, CpuCycleCompletion, CpuMode, CpuReg8Target, Exec};
pub use regs::Regs;
