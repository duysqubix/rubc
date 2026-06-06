//! Cartridge / MBC banking for the M-cycle [`Bus`](super::Bus).
//!
//! This replaces the old flat 32 KiB `rom: Vec<u8>` placeholder so the full
//! 64 KiB `cpu_instrs.gb` (and other MBC1 carts) can bank-switch correctly.
//!
//! Scope: **NoMbc (MBC0)**, **MBC1**, **MBC2**, **MBC3 (+RTC)** and **MBC5**
//! ROM/RAM banking. The MBC1 1 MiB+ multicart wiring quirk remains out of scope.
//! The MBC3 RTC is a functional latch-able counter (not yet wall-clock-driven).
//!
//! Address contract (caller guarantees the region; see `Bus::peek`/`poke`):
//!   - `read`      : `0x0000..=0x7FFF`
//!   - `read_ram`  : `0xA000..=0xBFFF`
//!   - `write_ram` : `0xA000..=0xBFFF`
//!   - `write_reg` : `0x0000..=0x7FFF` (MBC control register writes)

const ROM_BANK_SIZE: usize = 0x4000; // 16 KiB
const RAM_BANK_SIZE: usize = 0x2000; // 8 KiB

/// A loaded cartridge. Owns the full ROM image and any banking state.
pub enum Cartridge {
    /// No MBC (MBC0): up to 32 KiB ROM mapped flat, with external RAM ONLY when
    /// the header declares it (cart type 0x08/0x09).
    NoMbc(NoMbc),
    /// MBC1: ROM banking (5-bit primary + 2-bit secondary), mode select.
    Mbc1(Mbc1),
    /// MBC2: 4-bit ROM bank + built-in 512x4-bit RAM.
    Mbc2(Mbc2),
    /// MBC3: 7-bit ROM bank, 4 RAM banks or RTC registers, latch.
    Mbc3(Mbc3),
    /// MBC5: 9-bit ROM bank, 16 RAM banks, optional rumble.
    Mbc5(Mbc5),
}

impl Default for Cartridge {
    fn default() -> Self {
        // A blank 32 KiB MBC0 with NO external RAM (matches the old
        // `vec![0; 0x8000]` placeholder so an unloaded `Bus` behaves as before).
        Cartridge::NoMbc(NoMbc {
            rom: vec![0; 0x8000],
            ram: Vec::new(),
        })
    }
}

impl Cartridge {
    /// Build a cartridge from a raw ROM image, selecting the controller from the
    /// header byte at `0x0147`. Only the controllers the CPU/timing/MBC test ROMs
    /// need are supported here (MBC0, MBC1). Unsupported types are loaded as a
    /// flat MBC0 view of the first 32 KiB AND logged via `log::warn!`, rather than
    /// silently mis-emulated as MBC1 (which would corrupt MBC2/3/5 banking).
    /// MBC2/3/5 land in `rubc-cgi`.
    pub fn from_rom(rom: &[u8]) -> Self {
        let cart_type = rom.get(0x0147).copied().unwrap_or(0x00);
        match cart_type {
            // 0x00 ROM ONLY (no RAM); 0x08/0x09 ROM+RAM(+battery).
            0x00 | 0x08 | 0x09 => Cartridge::NoMbc(NoMbc::from_rom(rom, cart_type)),
            // 0x01..=0x03 MBC1 (+RAM/+battery).
            0x01..=0x03 => Cartridge::Mbc1(Mbc1::from_rom(rom)),
            // 0x05/0x06 MBC2 (+battery).
            0x05 | 0x06 => Cartridge::Mbc2(Mbc2::from_rom(rom)),
            // 0x0F..=0x13 MBC3 (+timer/+RAM/+battery).
            0x0F..=0x13 => Cartridge::Mbc3(Mbc3::from_rom(rom)),
            // 0x19..=0x1E MBC5 (+RAM/+battery/+rumble).
            0x19..=0x1E => Cartridge::Mbc5(Mbc5::from_rom(rom)),
            // Unsupported controller: do NOT pretend it is MBC1. Load a flat
            // 32 KiB view (enough to boot + run bank-0 code) and warn loudly.
            other => {
                log::warn!(
                    "unsupported cartridge type {other:#04X}; loading as flat MBC0 \
                     (banking unimplemented -- see rubc-cgi)"
                );
                Cartridge::NoMbc(NoMbc::from_rom(rom, 0x00))
            }
        }
    }

    /// Read a ROM byte. `addr` is in `0x0000..=0x7FFF`.
    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        match self {
            Cartridge::NoMbc(c) => c.read(addr),
            Cartridge::Mbc1(c) => c.read(addr),
            Cartridge::Mbc2(c) => c.read(addr),
            Cartridge::Mbc3(c) => c.read(addr),
            Cartridge::Mbc5(c) => c.read(addr),
        }
    }

    /// Read external RAM. `addr` is in `0xA000..=0xBFFF`.
    #[inline]
    pub fn read_ram(&self, addr: u16) -> u8 {
        match self {
            Cartridge::NoMbc(c) => c.read_ram(addr),
            Cartridge::Mbc1(c) => c.read_ram(addr),
            Cartridge::Mbc2(c) => c.read_ram(addr),
            Cartridge::Mbc3(c) => c.read_ram(addr),
            Cartridge::Mbc5(c) => c.read_ram(addr),
        }
    }

    /// Write external RAM. `addr` is in `0xA000..=0xBFFF`.
    #[inline]
    pub fn write_ram(&mut self, addr: u16, value: u8) {
        match self {
            Cartridge::NoMbc(c) => c.write_ram(addr, value),
            Cartridge::Mbc1(c) => c.write_ram(addr, value),
            Cartridge::Mbc2(c) => c.write_ram(addr, value),
            Cartridge::Mbc3(c) => c.write_ram(addr, value),
            Cartridge::Mbc5(c) => c.write_ram(addr, value),
        }
    }

    /// A write into the ROM region (`0x0000..=0x7FFF`): an MBC control register
    /// for banked carts; ignored for MBC0.
    #[inline]
    pub fn write_reg(&mut self, addr: u16, value: u8) {
        match self {
            Cartridge::NoMbc(_) => {} // ROM is read-only
            Cartridge::Mbc1(c) => c.write_reg(addr, value),
            Cartridge::Mbc2(c) => c.write_reg(addr, value),
            Cartridge::Mbc3(c) => c.write_reg(addr, value),
            Cartridge::Mbc5(c) => c.write_reg(addr, value),
        }
    }
}

/// MBC0 / ROM-only. External RAM exists ONLY when the header declares it
/// (cart type 0x08/0x09); a ROM-only `0x00` cart has no RAM, so reads of the
/// `0xA000..=0xBFFF` region return `0xFF` and writes are ignored.
pub struct NoMbc {
    rom: Vec<u8>,
    ram: Vec<u8>,
}

impl NoMbc {
    fn from_rom(rom: &[u8], cart_type: u8) -> Self {
        let mut buf = vec![0u8; 0x8000];
        let n = rom.len().min(buf.len());
        buf[..n].copy_from_slice(&rom[..n]);
        // RAM only for ROM+RAM types (0x08/0x09). ROM-only (0x00) has none.
        let ram = if matches!(cart_type, 0x08 | 0x09) {
            vec![0u8; RAM_BANK_SIZE]
        } else {
            Vec::new()
        };
        Self { rom: buf, ram }
    }

    #[inline]
    fn read(&self, addr: u16) -> u8 {
        *self.rom.get(addr as usize).unwrap_or(&0xFF)
    }

    #[inline]
    fn read_ram(&self, addr: u16) -> u8 {
        // No RAM (ROM-only) -> open-bus 0xFF.
        if self.ram.is_empty() {
            return 0xFF;
        }
        *self.ram.get((addr - 0xA000) as usize).unwrap_or(&0xFF)
    }

    #[inline]
    fn write_ram(&mut self, addr: u16, value: u8) {
        if let Some(b) = self.ram.get_mut((addr - 0xA000) as usize) {
            *b = value;
        }
    }
}

/// MBC1 controller. Implements ROM banking (the part the CPU test ROMs need):
///   - `0x0000..=0x1FFF`  RAM enable (0x0A in low nibble enables)
///   - `0x2000..=0x3FFF`  ROM bank number, low 5 bits (0 -> 1 quirk)
///   - `0x4000..=0x5FFF`  RAM bank / upper ROM bank bits (2 bits)
///   - `0x6000..=0x7FFF`  banking mode select (0 = ROM, 1 = RAM/advanced)
pub struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    num_rom_banks: usize,
    num_ram_banks: usize,
    ram_enabled: bool,
    /// Low 5 bits of the ROM bank (BANK1); 0 is remapped to 1.
    bank_lo: u8,
    /// 2-bit secondary register (BANK2): RAM bank or upper ROM bits.
    bank_hi: u8,
    /// 0 = simple ROM banking; 1 = advanced (RAM banking / upper bits to 0x0000).
    mode: u8,
}

impl Mbc1 {
    fn from_rom(rom: &[u8]) -> Self {
        // ROM size: round the image up to a whole number of 16 KiB banks
        // (>= 2). The header byte at 0x0148 is authoritative on real carts, but
        // sizing from the image length is robust for arbitrary test ROMs.
        let num_rom_banks = rom.len().div_ceil(ROM_BANK_SIZE).max(2);
        let mut rom_buf = vec![0u8; num_rom_banks * ROM_BANK_SIZE];
        rom_buf[..rom.len()].copy_from_slice(rom);

        // RAM size from header 0x0149: 0=>none, 2=>8K, 3=>32K(4 banks),
        // 4=>128K, 5=>64K. Default to 1 bank so writes don't panic.
        let num_ram_banks = match rom.get(0x0149).copied().unwrap_or(0) {
            0x00 => 0,
            0x02 => 1,
            0x03 => 4,
            0x04 => 16,
            0x05 => 8,
            _ => 1,
        };
        let ram = vec![0u8; num_ram_banks.max(1) * RAM_BANK_SIZE];

        Self {
            rom: rom_buf,
            ram,
            num_rom_banks,
            num_ram_banks,
            ram_enabled: false,
            bank_lo: 1,
            bank_hi: 0,
            mode: 0,
        }
    }

    /// The bank mapped at `0x0000..=0x3FFF`. In mode 0 this is always bank 0; in
    /// mode 1 the secondary register can shift it (large carts).
    #[inline]
    fn low_bank(&self) -> usize {
        let bank = if self.mode == 1 {
            (self.bank_hi as usize) << 5
        } else {
            0
        };
        bank % self.num_rom_banks
    }

    /// The bank mapped at `0x4000..=0x7FFF`: the combined number BANK2<<5 | BANK1.
    /// The `0 -> 1` remap applies to the BANK1 *register write* only, NOT to this
    /// combined+masked result -- so on a small ROM the modulo CAN land on bank 0
    /// (e.g. a 16-bank cart with BANK1=0x10 maps 16 % 16 == bank 0).
    #[inline]
    fn high_bank(&self) -> usize {
        let bank = ((self.bank_hi as usize) << 5) | (self.bank_lo as usize);
        bank % self.num_rom_banks
    }

    #[inline]
    fn read(&self, addr: u16) -> u8 {
        let (bank, offset) = match addr {
            0x0000..=0x3FFF => (self.low_bank(), addr as usize),
            _ => (self.high_bank(), (addr - 0x4000) as usize),
        };
        let phys = bank * ROM_BANK_SIZE + offset;
        *self.rom.get(phys).unwrap_or(&0xFF)
    }

    #[inline]
    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled || self.num_ram_banks == 0 {
            return 0xFF;
        }
        let bank = if self.mode == 1 {
            (self.bank_hi as usize) % self.num_ram_banks
        } else {
            0
        };
        let phys = bank * RAM_BANK_SIZE + (addr - 0xA000) as usize;
        *self.ram.get(phys).unwrap_or(&0xFF)
    }

    #[inline]
    fn write_ram(&mut self, addr: u16, value: u8) {
        if !self.ram_enabled || self.num_ram_banks == 0 {
            return;
        }
        let bank = if self.mode == 1 {
            (self.bank_hi as usize) % self.num_ram_banks
        } else {
            0
        };
        let phys = bank * RAM_BANK_SIZE + (addr - 0xA000) as usize;
        if let Some(b) = self.ram.get_mut(phys) {
            *b = value;
        }
    }

    #[inline]
    fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                // Low 5 bits; the 0 -> 1 remap is the MBC1 quirk.
                let v = value & 0x1F;
                self.bank_lo = if v == 0 { 1 } else { v };
            }
            0x4000..=0x5FFF => self.bank_hi = value & 0x03,
            0x6000..=0x7FFF => self.mode = value & 0x01,
            _ => {}
        }
    }
}

/// MBC2 controller. Up to 16 ROM banks (256 KiB) and a built-in 512 x 4-bit
/// RAM (no external RAM chip). The single control register region is split by
/// address bit 8: bit 8 = 0 -> RAM enable, bit 8 = 1 -> ROM bank (low 4 bits).
pub struct Mbc2 {
    rom: Vec<u8>,
    /// 512 nibbles, one per addressable cell; only the low 4 bits are stored.
    ram: Box<[u8; 512]>,
    num_rom_banks: usize,
    ram_enabled: bool,
    /// ROM bank for $4000-$7FFF (4 bits; 0 remapped to 1).
    rom_bank: u8,
}

impl Mbc2 {
    fn from_rom(rom: &[u8]) -> Self {
        let num_rom_banks = rom.len().div_ceil(ROM_BANK_SIZE).max(2);
        let mut rom_buf = vec![0u8; num_rom_banks * ROM_BANK_SIZE];
        rom_buf[..rom.len()].copy_from_slice(rom);
        Self {
            rom: rom_buf,
            ram: Box::new([0; 512]),
            num_rom_banks,
            ram_enabled: false,
            rom_bank: 1,
        }
    }

    #[inline]
    fn read(&self, addr: u16) -> u8 {
        let (bank, offset) = match addr {
            0x0000..=0x3FFF => (0, addr as usize),
            _ => (
                self.rom_bank as usize % self.num_rom_banks,
                (addr - 0x4000) as usize,
            ),
        };
        *self.rom.get(bank * ROM_BANK_SIZE + offset).unwrap_or(&0xFF)
    }

    #[inline]
    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        // Only 9 address bits are wired; upper nibble reads as 1.
        0xF0 | (self.ram[(addr & 0x01FF) as usize] & 0x0F)
    }

    #[inline]
    fn write_ram(&mut self, addr: u16, value: u8) {
        if self.ram_enabled {
            self.ram[(addr & 0x01FF) as usize] = value & 0x0F;
        }
    }

    #[inline]
    fn write_reg(&mut self, addr: u16, value: u8) {
        // Only $0000-$3FFF is decoded; address bit 8 selects the function.
        if addr & 0x4000 != 0 {
            return;
        }
        if addr & 0x0100 == 0 {
            // Bit 8 = 0: RAM enable (low nibble == 0xA enables).
            self.ram_enabled = (value & 0x0F) == 0x0A;
        } else {
            // Bit 8 = 1: ROM bank (low 4 bits; 0 -> 1).
            let v = value & 0x0F;
            self.rom_bank = if v == 0 { 1 } else { v };
        }
    }
}

/// MBC3 controller. Up to 128 ROM banks (2 MiB), 4 RAM banks (32 KiB), and a
/// built-in Real-Time Clock. The RTC here is a latch-able register file (the
/// halt bit + write are honoured); wall-clock ticking is not yet wired.
pub struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    num_rom_banks: usize,
    num_ram_banks: usize,
    ram_enabled: bool,
    /// 7-bit ROM bank for $4000-$7FFF (0 remapped to 1).
    rom_bank: u8,
    /// $4000-$5FFF select: 0x00-0x03 = RAM bank, 0x08-0x0C = RTC register.
    ram_select: u8,
    /// Live RTC registers [S, M, H, DL, DH].
    rtc: [u8; 5],
    /// Latched copy exposed at $A000-$BFFF when an RTC register is selected.
    rtc_latched: [u8; 5],
    /// Previous value written to the latch register ($6000-$7FFF).
    latch_prev: u8,
}

impl Mbc3 {
    fn from_rom(rom: &[u8]) -> Self {
        let num_rom_banks = rom.len().div_ceil(ROM_BANK_SIZE).max(2);
        let mut rom_buf = vec![0u8; num_rom_banks * ROM_BANK_SIZE];
        rom_buf[..rom.len()].copy_from_slice(rom);
        let num_ram_banks = match rom.get(0x0149).copied().unwrap_or(0) {
            0x00 => 0,
            0x02 => 1,
            0x03 => 4,
            _ => 4,
        };
        Self {
            rom: rom_buf,
            ram: vec![0u8; num_ram_banks.max(1) * RAM_BANK_SIZE],
            num_rom_banks,
            num_ram_banks,
            ram_enabled: false,
            rom_bank: 1,
            ram_select: 0,
            rtc: [0; 5],
            rtc_latched: [0; 5],
            latch_prev: 0xFF,
        }
    }

    /// True when $4000-$5FFF selected an RTC register (0x08-0x0C).
    #[inline]
    fn rtc_selected(&self) -> bool {
        matches!(self.ram_select, 0x08..=0x0C)
    }

    #[inline]
    fn read(&self, addr: u16) -> u8 {
        let (bank, offset) = match addr {
            0x0000..=0x3FFF => (0, addr as usize),
            _ => (
                self.rom_bank as usize % self.num_rom_banks,
                (addr - 0x4000) as usize,
            ),
        };
        *self.rom.get(bank * ROM_BANK_SIZE + offset).unwrap_or(&0xFF)
    }

    #[inline]
    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        if self.rtc_selected() {
            return self.rtc_latched[(self.ram_select - 0x08) as usize];
        }
        if self.num_ram_banks == 0 {
            return 0xFF;
        }
        let bank = self.ram_select as usize % self.num_ram_banks;
        *self
            .ram
            .get(bank * RAM_BANK_SIZE + (addr - 0xA000) as usize)
            .unwrap_or(&0xFF)
    }

    #[inline]
    fn write_ram(&mut self, addr: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }
        if self.rtc_selected() {
            self.rtc[(self.ram_select - 0x08) as usize] = value;
            return;
        }
        if self.num_ram_banks == 0 {
            return;
        }
        let bank = self.ram_select as usize % self.num_ram_banks;
        if let Some(b) = self
            .ram
            .get_mut(bank * RAM_BANK_SIZE + (addr - 0xA000) as usize)
        {
            *b = value;
        }
    }

    #[inline]
    fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                // 7-bit ROM bank; 0 remaps to 1.
                let v = value & 0x7F;
                self.rom_bank = if v == 0 { 1 } else { v };
            }
            0x4000..=0x5FFF => self.ram_select = value & 0x0F,
            0x6000..=0x7FFF => {
                // 0x00 then 0x01 latches the live RTC into the readable copy.
                if self.latch_prev == 0x00 && value == 0x01 {
                    self.rtc_latched = self.rtc;
                }
                self.latch_prev = value;
            }
            _ => {}
        }
    }
}

/// MBC5 controller. Up to 512 ROM banks (8 MiB) via a 9-bit bank register, up
/// to 16 RAM banks (128 KiB), and optional rumble (rumble maps to RAM-bank bit
/// 3 and is ignored for storage). Unlike MBC1/3, bank 0 is NOT remapped.
pub struct Mbc5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    num_rom_banks: usize,
    num_ram_banks: usize,
    ram_enabled: bool,
    /// 9-bit ROM bank (low 8 + bit 8).
    rom_bank: u16,
    /// 4-bit RAM bank.
    ram_bank: u8,
    /// True if this cart has a rumble motor (RAM-bank bit 3 drives it).
    has_rumble: bool,
}

impl Mbc5 {
    fn from_rom(rom: &[u8]) -> Self {
        let num_rom_banks = rom.len().div_ceil(ROM_BANK_SIZE).max(2);
        let mut rom_buf = vec![0u8; num_rom_banks * ROM_BANK_SIZE];
        rom_buf[..rom.len()].copy_from_slice(rom);
        let num_ram_banks = match rom.get(0x0149).copied().unwrap_or(0) {
            0x00 => 0,
            0x02 => 1,
            0x03 => 4,
            0x04 => 16,
            0x05 => 8,
            _ => 0,
        };
        let has_rumble = matches!(rom.get(0x0147).copied().unwrap_or(0), 0x1C..=0x1E);
        Self {
            rom: rom_buf,
            ram: vec![0u8; num_ram_banks.max(1) * RAM_BANK_SIZE],
            num_rom_banks,
            num_ram_banks,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            has_rumble,
        }
    }

    #[inline]
    fn read(&self, addr: u16) -> u8 {
        let (bank, offset) = match addr {
            0x0000..=0x3FFF => (0, addr as usize),
            _ => (
                self.rom_bank as usize % self.num_rom_banks,
                (addr - 0x4000) as usize,
            ),
        };
        *self.rom.get(bank * ROM_BANK_SIZE + offset).unwrap_or(&0xFF)
    }

    #[inline]
    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled || self.num_ram_banks == 0 {
            return 0xFF;
        }
        let bank = self.ram_bank as usize % self.num_ram_banks;
        *self
            .ram
            .get(bank * RAM_BANK_SIZE + (addr - 0xA000) as usize)
            .unwrap_or(&0xFF)
    }

    #[inline]
    fn write_ram(&mut self, addr: u16, value: u8) {
        if !self.ram_enabled || self.num_ram_banks == 0 {
            return;
        }
        let bank = self.ram_bank as usize % self.num_ram_banks;
        if let Some(b) = self
            .ram
            .get_mut(bank * RAM_BANK_SIZE + (addr - 0xA000) as usize)
        {
            *b = value;
        }
    }

    #[inline]
    fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
            0x2000..=0x2FFF => {
                // Low 8 bits of the 9-bit ROM bank (NO 0 -> 1 remap on MBC5).
                self.rom_bank = (self.rom_bank & 0x0100) | value as u16;
            }
            0x3000..=0x3FFF => {
                // Bit 8 of the ROM bank.
                self.rom_bank = (self.rom_bank & 0x00FF) | (((value & 0x01) as u16) << 8);
            }
            0x4000..=0x5FFF => {
                // 4-bit RAM bank; on rumble carts bit 3 drives the motor.
                if self.has_rumble {
                    self.ram_bank = value & 0x07;
                } else {
                    self.ram_bank = value & 0x0F;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic ROM of `banks` 16 KiB banks where every byte in bank
    /// `b` equals `b as u8` (so a read reveals which bank is mapped). Sets the
    /// MBC1 header byte so `from_rom` picks the banked controller.
    fn synth_mbc1(banks: usize) -> Vec<u8> {
        let mut rom = vec![0u8; banks * ROM_BANK_SIZE];
        for b in 0..banks {
            for byte in &mut rom[b * ROM_BANK_SIZE..(b + 1) * ROM_BANK_SIZE] {
                *byte = b as u8;
            }
        }
        rom[0x0147] = 0x01; // MBC1
        rom
    }

    #[test]
    fn nombc_reads_flat_and_ignores_rom_writes() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0000] = 0xAA;
        rom[0x4000] = 0xBB;
        rom[0x7FFF] = 0xCC;
        let mut cart = Cartridge::from_rom(&rom);
        assert!(matches!(cart, Cartridge::NoMbc(_)));
        assert_eq!(cart.read(0x0000), 0xAA);
        assert_eq!(cart.read(0x4000), 0xBB);
        assert_eq!(cart.read(0x7FFF), 0xCC);
        // ROM write is a no-op (no banking on MBC0).
        cart.write_reg(0x2000, 0x05);
        assert_eq!(cart.read(0x4000), 0xBB, "MBC0 ROM stays read-only");
    }

    #[test]
    fn mbc1_bank0_fixed_high_bank_switchable() {
        let rom = synth_mbc1(4); // banks 0..=3
        let mut cart = Cartridge::from_rom(&rom);
        assert!(matches!(cart, Cartridge::Mbc1(_)));
        // 0x0000 region always bank 0.
        assert_eq!(cart.read(0x0000), 0x00);
        // Default high bank is 1.
        assert_eq!(cart.read(0x4000), 0x01);
        // Select bank 2 -> 0x4000 maps bank 2; 0x0000 still bank 0.
        cart.write_reg(0x2000, 0x02);
        assert_eq!(cart.read(0x4000), 0x02);
        assert_eq!(cart.read(0x0000), 0x00);
        // Select bank 3.
        cart.write_reg(0x2000, 0x03);
        assert_eq!(cart.read(0x4000), 0x03);
    }

    #[test]
    fn mbc1_bank_zero_remaps_to_one() {
        let rom = synth_mbc1(4);
        let mut cart = Cartridge::from_rom(&rom);
        cart.write_reg(0x2000, 0x00); // BANK1=0 -> remap to 1
        assert_eq!(cart.read(0x4000), 0x01, "bank 0 remaps to 1");
    }

    #[test]
    fn mbc1_bank_select_wraps_on_small_rom() {
        let rom = synth_mbc1(4); // only 4 banks
        let mut cart = Cartridge::from_rom(&rom);
        // Requesting bank 5 wraps modulo 4 -> bank 1.
        cart.write_reg(0x2000, 0x05);
        assert_eq!(cart.read(0x4000), 0x01);
    }

    #[test]
    fn mbc1_ram_disabled_reads_ff_and_swallows_writes() {
        let mut rom = synth_mbc1(4);
        rom[0x0149] = 0x02; // 8 KiB RAM
        let mut cart = Cartridge::from_rom(&rom);
        // RAM disabled by default.
        assert_eq!(cart.read_ram(0xA000), 0xFF);
        cart.write_ram(0xA000, 0x42);
        assert_eq!(
            cart.read_ram(0xA000),
            0xFF,
            "write swallowed while disabled"
        );
        // Enable RAM, then it round-trips.
        cart.write_reg(0x0000, 0x0A);
        cart.write_ram(0xA000, 0x42);
        assert_eq!(cart.read_ram(0xA000), 0x42);
        // Disable again -> reads FF.
        cart.write_reg(0x0000, 0x00);
        assert_eq!(cart.read_ram(0xA000), 0xFF);
    }

    #[test]
    fn nombc_rom_only_has_no_external_ram() {
        // Cart type 0x00 (ROM ONLY): the 0xA000..=0xBFFF region is open-bus.
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x00;
        let mut cart = Cartridge::from_rom(&rom);
        assert!(matches!(cart, Cartridge::NoMbc(_)));
        assert_eq!(cart.read_ram(0xA000), 0xFF, "ROM-only has no RAM");
        cart.write_ram(0xA000, 0x42);
        assert_eq!(cart.read_ram(0xA000), 0xFF, "ROM-only RAM write ignored");
    }

    #[test]
    fn nombc_rom_plus_ram_round_trips() {
        // Cart type 0x08 (ROM+RAM): external RAM exists and round-trips.
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x08;
        let mut cart = Cartridge::from_rom(&rom);
        assert!(matches!(cart, Cartridge::NoMbc(_)));
        cart.write_ram(0xA000, 0x42);
        assert_eq!(cart.read_ram(0xA000), 0x42, "ROM+RAM round-trips");
        assert_eq!(cart.read_ram(0xBFFF), 0x00, "untouched RAM reads 0");
    }

    #[test]
    fn mbc1_high_bank_can_map_bank_zero_on_small_rom() {
        // Reviewer regression: \"never bank 0\" is FALSE for a small ROM. A
        // 16-bank cart with BANK1=0x10 -> combined bank 16, masked 16 % 16 == 0,
        // so the 0x4000 region maps physical bank 0. (The 0->1 remap applies to
        // the BANK1 register write only; 0x10 is nonzero so it is NOT remapped.)
        let rom = synth_mbc1(16); // banks 0..=15, byte == bank index
        let mut cart = Cartridge::from_rom(&rom);
        cart.write_reg(0x2000, 0x10); // BANK1 = 0x10 (nonzero -> no remap)
        assert_eq!(
            cart.read(0x4000),
            0x00,
            "16-bank cart: BANK1=0x10 wraps to physical bank 0"
        );
    }

    /// Synthetic ROM of `banks` 16 KiB banks (byte b == bank index for banks
    /// 0..=255; higher banks wrap visually but reads still reveal the low byte),
    /// with the given cartridge-type header at $0147.
    fn synth(banks: usize, cart_type: u8) -> Vec<u8> {
        let mut rom = vec![0u8; banks * ROM_BANK_SIZE];
        for b in 0..banks {
            for byte in &mut rom[b * ROM_BANK_SIZE..(b + 1) * ROM_BANK_SIZE] {
                *byte = b as u8;
            }
        }
        rom[0x0147] = cart_type;
        rom
    }

    // ---- MBC2 -------------------------------------------------------------

    #[test]
    fn mbc2_rom_bank_select_via_addr_bit8() {
        let rom = synth(8, 0x05); // MBC2
        let mut cart = Cartridge::from_rom(&rom);
        assert!(matches!(cart, Cartridge::Mbc2(_)));
        assert_eq!(cart.read(0x0000), 0x00, "bank 0 fixed");
        assert_eq!(cart.read(0x4000), 0x01, "default bank 1");
        // Bit 8 SET ($2100) selects ROM bank; bit 8 clear would be RAM enable.
        cart.write_reg(0x2100, 0x03);
        assert_eq!(cart.read(0x4000), 0x03, "bank 3 via addr bit 8 set");
        // 0 remaps to 1.
        cart.write_reg(0x2100, 0x00);
        assert_eq!(cart.read(0x4000), 0x01, "bank 0 remaps to 1");
    }

    #[test]
    fn mbc2_builtin_ram_is_4bit_and_mirrored() {
        let rom = synth(4, 0x06); // MBC2+battery
        let mut cart = Cartridge::from_rom(&rom);
        // RAM disabled by default.
        assert_eq!(cart.read_ram(0xA000), 0xFF);
        // Enable: bit 8 of addr CLEAR ($0000) + low nibble 0xA.
        cart.write_reg(0x0000, 0x0A);
        cart.write_ram(0xA000, 0xF5);
        // Only the low nibble is stored; the upper nibble reads as 1.
        assert_eq!(cart.read_ram(0xA000), 0xF5);
        // Address mirrors every 0x200 ($A000 == $A200).
        assert_eq!(cart.read_ram(0xA200), 0xF5, "512-byte RAM mirrors");
        // Disable -> open-bus.
        cart.write_reg(0x0000, 0x00);
        assert_eq!(cart.read_ram(0xA000), 0xFF);
    }

    // ---- MBC3 -------------------------------------------------------------

    #[test]
    fn mbc3_rom_bank_7bit_and_ram_banking() {
        let mut rom = synth(8, 0x13); // MBC3+RAM+battery
        rom[0x0149] = 0x03; // 4 RAM banks
        let mut cart = Cartridge::from_rom(&rom);
        assert!(matches!(cart, Cartridge::Mbc3(_)));
        cart.write_reg(0x2000, 0x05);
        assert_eq!(cart.read(0x4000), 0x05, "7-bit ROM bank select");
        cart.write_reg(0x2000, 0x00);
        assert_eq!(cart.read(0x4000), 0x01, "bank 0 remaps to 1");
        // RAM banking: enable, select bank 2, round-trip.
        cart.write_reg(0x0000, 0x0A);
        cart.write_reg(0x4000, 0x02);
        cart.write_ram(0xA000, 0x77);
        assert_eq!(cart.read_ram(0xA000), 0x77, "RAM bank 2 round-trips");
        // Bank 0 is separate, untouched storage (reads 0).
        cart.write_reg(0x4000, 0x00);
        assert_eq!(cart.read_ram(0xA000), 0x00, "bank 0 untouched");
    }

    #[test]
    fn mbc3_rtc_latch_and_register_access() {
        let rom = synth(4, 0x0F); // MBC3+TIMER+BATTERY
        let mut cart = Cartridge::from_rom(&rom);
        cart.write_reg(0x0000, 0x0A); // enable RAM/RTC access
                                      // Select RTC seconds register (0x08) and write it.
        cart.write_reg(0x4000, 0x08);
        cart.write_ram(0xA000, 0x2A);
        // Latch (0x00 -> 0x01) copies live RTC into the readable copy.
        cart.write_reg(0x6000, 0x00);
        cart.write_reg(0x6000, 0x01);
        assert_eq!(cart.read_ram(0xA000), 0x2A, "RTC seconds latched + read");
    }

    // ---- MBC5 -------------------------------------------------------------

    #[test]
    fn mbc5_9bit_rom_bank_no_zero_remap() {
        let rom = synth(260, 0x19); // MBC5, >256 banks to exercise bit 8
        let mut cart = Cartridge::from_rom(&rom);
        assert!(matches!(cart, Cartridge::Mbc5(_)));
        // Bank 0 is selectable on MBC5 (no remap).
        cart.write_reg(0x2000, 0x00);
        assert_eq!(cart.read(0x4000), 0x00, "MBC5 bank 0 NOT remapped");
        // Low byte + high bit form a 9-bit bank: 0x104 = 260 wraps to 0.
        cart.write_reg(0x2000, 0x04);
        cart.write_reg(0x3000, 0x01);
        // bank 0x104 = 260; 260 % 260 == 0 -> byte 0.
        assert_eq!(cart.read(0x4000), 0x00, "9-bit bank 0x104 wraps mod 260");
        // bank 0x05 reads bank 5.
        cart.write_reg(0x3000, 0x00);
        cart.write_reg(0x2000, 0x05);
        assert_eq!(cart.read(0x4000), 0x05);
    }

    #[test]
    fn mbc5_ram_banking_round_trips() {
        let mut rom = synth(4, 0x1B); // MBC5+RAM+battery
        rom[0x0149] = 0x03; // 4 RAM banks
        let mut cart = Cartridge::from_rom(&rom);
        cart.write_reg(0x0000, 0x0A);
        cart.write_reg(0x4000, 0x02);
        cart.write_ram(0xA000, 0x9C);
        assert_eq!(cart.read_ram(0xA000), 0x9C, "RAM bank 2 round-trips");
        cart.write_reg(0x4000, 0x00);
        assert_eq!(cart.read_ram(0xA000), 0x00, "bank 0 is separate");
    }
}
