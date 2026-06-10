//! Tickable peripheral stubs for the N4 bus skeleton.
//!
//! These are intentionally minimal: just enough state so the bus invariant
//! (OAM-DMA beat -> 4 T-cycle ticks -> latched access) can be built and proven
//! BEFORE the real PPU/APU land in later waves. Each stub counts its ticks so
//! tests can assert the ordering of the invariant.

/// Interrupt enable / flag registers.
///
/// IRQ sources latch IF immediately on the T-cycle that requests them. The CPU
/// still decides whether to dispatch only at an instruction boundary.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Interrupts {
    pub ie: u8,
    pub if_: u8,
}

impl Interrupts {
    /// Request an interrupt (by bit 0..=4). Latches IF immediately.
    pub fn request(&mut self, bit: u8) {
        self.if_ |= 1 << bit;
    }

    /// Normalize IF's unused high bits at an instruction boundary.
    pub fn settle_boundary(&mut self) {
        self.if_ |= 0xE0; // top 3 bits read as 1
    }

    /// Currently visible pending+enabled interrupts (IE & IF, low 5 bits).
    pub fn pending_mask(&self) -> u8 {
        self.ie & self.if_ & 0x1F
    }

    pub fn clear_bit(&mut self, bit: u8) {
        self.if_ &= !(1 << bit);
    }
}

/// Joypad button state ($FF00 / P1 / JOYP).
///
/// The hardware register is active-LOW: a 0 bit = pressed. Bits 5-4 select
/// which line is read (bit 5 = action buttons, bit 4 = d-pad); bits 3-0 report
/// the selected line. We store the logical pressed/not-pressed state per button
/// (true = pressed) and synthesize the register on read.
#[derive(Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Joypad {
    // Action buttons (selected when P1 bit 5 == 0).
    pub a: bool,
    pub b: bool,
    pub select: bool,
    pub start: bool,
    // Direction buttons (selected when P1 bit 4 == 0).
    pub right: bool,
    pub left: bool,
    pub up: bool,
    pub down: bool,
    /// The last value written to P1 bits 5-4 (line select). Bit 5 selects
    /// action, bit 4 selects d-pad; a 0 in the bit means that line is selected.
    pub select_bits: u8,
}

/// One of the eight Game Boy buttons, for the public input API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Button {
    A,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down,
}

impl Joypad {
    /// Set a button's pressed state. Returns true if this is a fresh press of a
    /// currently-selected line's button (a high->low edge on a P1 input bit),
    /// which is what raises the joypad interrupt.
    pub fn set_button(&mut self, button: Button, pressed: bool) -> bool {
        let was = self.line_value();
        match button {
            Button::A => self.a = pressed,
            Button::B => self.b = pressed,
            Button::Select => self.select = pressed,
            Button::Start => self.start = pressed,
            Button::Right => self.right = pressed,
            Button::Left => self.left = pressed,
            Button::Up => self.up = pressed,
            Button::Down => self.down = pressed,
        }
        let now = self.line_value();
        // A joypad IRQ fires on any selected-line input bit going high->low
        // (1 = released -> 0 = pressed).
        (was & !now) != 0
    }

    /// The current low nibble (bits 3-0) for the selected line, active-low.
    fn line_value(&self) -> u8 {
        let action_selected = self.select_bits & 0x20 == 0;
        let dpad_selected = self.select_bits & 0x10 == 0;
        let mut v = 0x0F;
        if action_selected {
            if self.a {
                v &= !0x01;
            }
            if self.b {
                v &= !0x02;
            }
            if self.select {
                v &= !0x04;
            }
            if self.start {
                v &= !0x08;
            }
        }
        if dpad_selected {
            if self.right {
                v &= !0x01;
            }
            if self.left {
                v &= !0x02;
            }
            if self.up {
                v &= !0x04;
            }
            if self.down {
                v &= !0x08;
            }
        }
        v
    }

    /// Read the full P1/JOYP register: bits 7-6 always 1, bits 5-4 = line
    /// select, bits 3-0 = active-low button state for the selected line(s).
    pub fn read_p1(&self) -> u8 {
        0xC0 | (self.select_bits & 0x30) | self.line_value()
    }

    /// Write P1: only bits 5-4 (line select) are writable.
    pub fn write_p1(&mut self, value: u8) {
        self.select_bits = value & 0x30;
    }
}

/// CGB clock/speed state.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct CgbState {
    /// True when the machine is CGB hardware, even if the boot ROM selected
    /// DMG-compatibility behavior for a non-CGB cartridge.
    #[serde(default)]
    pub is_cgb: bool,
    /// True when running as a Game Boy Color. DMG has no speed switch, so KEY1
    /// writes are inert and STOP always halts. Set by `Machine::boot_cgb`.
    pub cgb_mode: bool,
    pub double_speed: bool,
    /// Toggles each T so PPU/APU advance every 2nd T in double-speed mode.
    pub t_phase: bool,
}
