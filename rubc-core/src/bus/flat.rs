//! Flat 64 KiB test bus for SM83 single-step vector testing.
//!
//! The SingleStepTests/sm83 JSON vectors model the CPU against a flat 64 KiB
//! address space: every address is plain RAM, including the ROM region and the
//! external-RAM top byte `0xBFFF`. The production [`Bus`](super::Bus) routes
//! `0x0000..=0x7FFF` to the cartridge (read-only) and `0xA000..=0xBFFF` to the
//! MBC, which is correct for real ROMs but wrong for these vectors — that
//! routing is exactly the `0xBFFF` bug the old harness hit (`opcode_test`
//! expected `$41` at `0xBFFF` but read `$00` because the write went to the
//! cart).
//!
//! `FlatBus` implements [`CpuBus`](super::CpuBus) with no peripherals and no
//! ticking: each `*_m` is a direct array access, so a vector's expected final
//! RAM matches byte-for-byte. It is the test double N2's `step_m` CPU runs
//! against.

use super::CpuBus;

/// A flat 64 KiB address space implementing [`CpuBus`] with no side effects.
pub struct FlatBus {
    pub mem: Box<[u8; 0x1_0000]>,
    /// Number of `*_m` calls (a coarse M-cycle count for vector cycle checks).
    pub m_cycles: u64,
    ie: u8,
    if_: u8,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self {
            mem: Box::new([0; 0x1_0000]),
            m_cycles: 0,
            ie: 0,
            if_: 0,
        }
    }
}

impl FlatBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Direct, untimed read (does not count an M-cycle). For harness setup.
    pub fn peek(&self, addr: u16) -> u8 {
        self.mem[addr as usize]
    }

    /// Direct, untimed write (does not count an M-cycle). For harness setup.
    pub fn poke(&mut self, addr: u16, value: u8) {
        self.mem[addr as usize] = value;
    }

    /// Set the IE register directly (vectors carry an `ie` field).
    pub fn set_ie(&mut self, ie: u8) {
        self.ie = ie;
    }
}

impl CpuBus for FlatBus {
    fn read_m(&mut self, addr: u16) -> u8 {
        self.m_cycles += 1;
        self.mem[addr as usize]
    }

    fn write_m(&mut self, addr: u16, value: u8) {
        self.m_cycles += 1;
        self.mem[addr as usize] = value;
    }

    fn idle_m(&mut self) {
        self.m_cycles += 1;
    }

    fn irq_pending_mask(&self) -> u8 {
        self.ie & self.if_ & 0x1F
    }

    fn ie(&self) -> u8 {
        self.ie
    }

    fn clear_if_bit(&mut self, bit: u8) {
        self.if_ &= !(1 << bit);
    }

    fn speed_switch_armed(&self) -> bool {
        // The flat vector model has no CGB speed switch.
        false
    }

    fn finish_speed_switch(&mut self) {
        // No-op: the flat vector model has no clock domains.
    }

    fn boundary(&mut self) {
        // No queued-IRQ model in the flat bus; nothing to settle.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_bus_0xbfff_roundtrips() {
        // S1: the exact bug — external-RAM top byte routes to flat RAM, not cart.
        let mut bus = FlatBus::new();
        bus.write_m(0xBFFF, 0x41);
        assert_eq!(bus.read_m(0xBFFF), 0x41, "0xBFFF must be plain flat RAM");
    }

    #[test]
    fn flat_bus_full_range_roundtrips() {
        // S2: every address is plain RAM, including the ROM region.
        let mut bus = FlatBus::new();
        for addr in [
            0x0000u16, 0x4000, 0x7FFF, 0x8000, 0xA000, 0xBFFF, 0xC000, 0xFFFF,
        ] {
            let val = (addr & 0xFF) as u8 ^ 0xA5;
            bus.poke(addr, val);
            assert_eq!(bus.read_m(addr), val, "addr {addr:04X} round-trips");
        }
    }

    #[test]
    fn flat_bus_is_cpubus() {
        // S3: FlatBus satisfies the CpuBus contract used by the CPU.
        fn drive<B: CpuBus>(bus: &mut B) -> u8 {
            bus.write_m(0xC000, 0x99);
            bus.idle_m();
            bus.boundary();
            bus.finish_speed_switch();
            let _ = bus.ie();
            bus.read_m(0xC000)
        }
        let mut bus = FlatBus::new();
        assert_eq!(drive(&mut bus), 0x99);
        assert_eq!(bus.m_cycles, 3, "write + idle + read = 3 M-cycles");
    }

    #[test]
    fn ie_if_mask_works() {
        let mut bus = FlatBus::new();
        bus.set_ie(0x1F);
        bus.if_ = 0x04;
        assert_eq!(bus.irq_pending_mask(), 0x04);
        bus.clear_if_bit(2);
        assert_eq!(bus.irq_pending_mask(), 0x00);
    }
}
