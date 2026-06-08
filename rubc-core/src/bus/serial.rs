use super::stubs::Interrupts;

const SC_START: u8 = 0x80;
const SC_FAST: u8 = 0x02;
const SC_INTERNAL_CLOCK: u8 = 0x01;
const SERIAL_INTERRUPT_BIT: u8 = 3;
const SERIAL_CLOCK_PHASE: u16 = 52;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Serial {
    sb: u8,
    sc: u8,
    bits_remaining: u8,
    previous_clock_high: bool,
}

impl Serial {
    pub fn read_sb(&self) -> u8 {
        self.sb
    }

    pub fn write_sb(&mut self, value: u8) {
        self.sb = value;
    }

    pub fn read_sc(&self, cgb_mode: bool) -> u8 {
        if cgb_mode {
            self.sc | 0x7C
        } else {
            self.sc | 0x7E
        }
    }

    pub fn write_sc(&mut self, value: u8, cgb_mode: bool, div_counter: u16) {
        let writable = if cgb_mode { 0x83 } else { 0x81 };
        self.sc = value & writable;

        if self.sc & SC_START == 0 {
            self.bits_remaining = 0;
            self.previous_clock_high = self.clock_high(div_counter);
            return;
        }

        self.bits_remaining = 8;
        self.previous_clock_high = self.clock_high(div_counter);
    }

    pub fn tick_t(&mut self, div_counter: u16, irq: &mut Interrupts) {
        self.advance_from_div(div_counter, irq);
    }

    pub fn div_changed(&mut self, div_counter: u16, irq: &mut Interrupts) {
        self.advance_from_div(div_counter, irq);
    }

    fn advance_from_div(&mut self, div_counter: u16, irq: &mut Interrupts) {
        let now = self.clock_high(div_counter);
        let falling_edge = self.previous_clock_high && !now;
        self.previous_clock_high = now;

        if self.internal_transfer_active() && falling_edge {
            self.shift_bit(irq);
        }
    }

    fn internal_transfer_active(&self) -> bool {
        self.sc & (SC_START | SC_INTERNAL_CLOCK) == (SC_START | SC_INTERNAL_CLOCK)
            && self.bits_remaining > 0
    }

    fn clock_high(&self, div_counter: u16) -> bool {
        let mask = if self.sc & SC_FAST != 0 {
            0x0008
        } else {
            0x0100
        };
        div_counter.wrapping_sub(SERIAL_CLOCK_PHASE) & mask != 0
    }

    fn shift_bit(&mut self, irq: &mut Interrupts) {
        self.sb = (self.sb << 1) | 1;
        self.bits_remaining -= 1;

        if self.bits_remaining == 0 {
            self.sc &= !SC_START;
            irq.request(SERIAL_INTERRUPT_BIT);
        }
    }
}
