//! Cycle-accurate DMG timer (DIV / TIMA / TMA / TAC).
//!
//! The 16-bit `div` increments every T-cycle; TIMA increments on the FALLING
//! edge of `(selected_DIV_bit AND timer_enable)`. Overflow schedules a reload of
//! TMA + the Timer interrupt exactly 4 T-cycles later (`Reload::Pending(4)` ->
//! reload on T4), with the documented write-cancel/ignore/copy quirks.
//!
//! SCOPE: this implements the DMG edge circuit (enable-AND-selected-bit). The
//! CGB TAC write edge behaves differently in a few cases (e.g. disabling the
//! timer while the selected bit is high); that CGB-specific TAC quirk is tracked
//! as a follow-up and NOT implemented here. `post_boot_dmg` seeds `div = 0xAB00`
//! so the visible DIV ($FF04) reads 0xAB; the hidden low byte is phase-arbitrary
//! (no boot ROM yet) and is not authoritative for boot-DIV-exact tests.

use super::stubs::Interrupts;

#[derive(Debug, Clone)]
pub struct Timer {
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8, // only bits 0..=2 stored
    previous_and_result: bool,
    reload: Reload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reload {
    None,
    Pending(u8),      // ticks remaining before reload
    ReloadedThisTick, // TIMA write ignored; TMA write copies to TIMA
}

impl Timer {
    pub fn power_on() -> Self {
        let mut t = Self {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            previous_and_result: false,
            reload: Reload::None,
        };
        t.previous_and_result = t.timer_input();
        t
    }

    pub fn post_boot_dmg() -> Self {
        let mut t = Self::power_on();
        t.div = 0xAB00; // visible DIV = 0xAB; hidden low byte deferred
        t.previous_and_result = t.timer_input();
        t
    }

    pub fn tick_t(&mut self, irq: &mut Interrupts) {
        if self.reload == Reload::ReloadedThisTick {
            self.reload = Reload::None;
        }
        let pending_before_tick = matches!(self.reload, Reload::Pending(_));
        self.div = self.div.wrapping_add(1);
        self.apply_falling_edge_after_div_or_tac_change();
        if pending_before_tick {
            self.advance_reload(irq);
        } // do NOT decrement a Pending created THIS tick
    }

    fn selected_bit(tac: u8) -> u8 {
        match tac & 0b11 {
            0b00 => 9,
            0b01 => 3,
            0b10 => 5,
            0b11 => 7,
            _ => unreachable!(),
        }
    }

    fn timer_input(&self) -> bool {
        let enabled = self.tac & 0b100 != 0;
        let bit = Self::selected_bit(self.tac);
        enabled && (self.div & (1u16 << bit)) != 0
    }

    fn apply_falling_edge_after_div_or_tac_change(&mut self) {
        let now = self.timer_input();
        if self.previous_and_result && !now {
            self.increment_tima();
        }
        self.previous_and_result = now;
    }

    fn increment_tima(&mut self) {
        let (next, overflowed) = self.tima.overflowing_add(1);
        if overflowed {
            self.tima = 0x00;
            self.reload = Reload::Pending(4);
        } else {
            self.tima = next;
        }
    }

    fn advance_reload(&mut self, irq: &mut Interrupts) {
        match self.reload {
            Reload::Pending(1) => {
                self.tima = self.tma;
                irq.request(2);
                self.reload = Reload::ReloadedThisTick;
            }
            Reload::Pending(n) => {
                self.reload = Reload::Pending(n - 1);
            }
            Reload::None | Reload::ReloadedThisTick => {}
        }
    }

    /// The full 16-bit internal DIV counter. The APU's frame sequencer (DIV-APU)
    /// is clocked by the falling edge of DIV bit 4 (bit 5 in CGB double-speed).
    pub fn div_counter(&self) -> u16 {
        self.div
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => 0xF8 | (self.tac & 0x07),
            _ => unreachable!("not timer register: {addr:#06x}"),
        }
    }

    pub fn write(&mut self, addr: u16, value: u8, _irq: &mut Interrupts) {
        match addr {
            0xFF04 => self.write_div(),
            0xFF05 => self.write_tima(value),
            0xFF06 => self.write_tma(value),
            0xFF07 => self.write_tac(value),
            _ => unreachable!("not timer register: {addr:#06x}"),
        }
    }

    fn write_div(&mut self) {
        // Resetting DIV can drop the selected timer-input bit from 1 to 0, a
        // falling edge that increments TIMA (overflow schedules Reload::Pending,
        // whose IRQ fires later via the tick-driven countdown).
        self.div = 0;
        self.apply_falling_edge_after_div_or_tac_change();
    }

    fn write_tac(&mut self, value: u8) {
        // Changing TAC (enable/selector) can flip the AND result high->low, a
        // falling edge that increments TIMA. rapid_toggle relies on this.
        self.tac = value & 0x07;
        self.apply_falling_edge_after_div_or_tac_change();
    }

    fn write_tima(&mut self, value: u8) {
        match self.reload {
            Reload::Pending(_) => {
                self.tima = value;
                self.reload = Reload::None;
            } // cancel reload + no IF
            Reload::ReloadedThisTick => {} // ignored
            Reload::None => {
                self.tima = value;
            }
        }
    }

    fn write_tma(&mut self, value: u8) {
        self.tma = value;
        if self.reload == Reload::ReloadedThisTick {
            self.tima = value;
        }
    }
}

#[cfg(test)]
impl Timer {
    /// Test-only: seed the timer so the NEXT timer-input falling edge overflows
    /// TIMA. Sets TIMA=0xFF, TMA, enables the timer on the given TAC selector,
    /// and primes `div` + `previous_and_result` to the just-before-falling-edge
    /// state. Used by the bus-level reload-quirk regression.
    pub(crate) fn test_prime_for_overflow(&mut self, tma: u8, tac_low: u8) {
        self.tima = 0xFF;
        self.tma = tma;
        self.tac = 0x04 | (tac_low & 0x03);
        // Prime div so the NEXT increment carries through and clears the selected
        // bit (a 1->0 falling edge). For bit N that means all bits 0..=N set, so
        // div+1 == 1<<(N+1) with bit N falling.
        let bit = Self::selected_bit(self.tac);
        self.div = (1u16 << (bit + 1)) - 1;
        self.previous_and_result = self.timer_input();
        self.reload = Reload::None;
    }

    pub(crate) fn test_tima(&self) -> u8 {
        self.tima
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMER_IRQ: u8 = 0x04;

    fn timer_and_interrupts() -> (Timer, Interrupts) {
        let irq = Interrupts {
            ie: TIMER_IRQ,
            ..Interrupts::default()
        };
        (Timer::power_on(), irq)
    }

    fn tick_n(timer: &mut Timer, irq: &mut Interrupts, ticks: usize) {
        for _ in 0..ticks {
            timer.tick_t(irq);
        }
    }

    fn set_timer_input(timer: &mut Timer, div: u16, tac: u8) {
        timer.div = div;
        timer.tac = tac & 0x07;
        timer.previous_and_result = timer.timer_input();
    }

    fn overflow_to_pending(timer: &mut Timer, irq: &mut Interrupts) {
        timer.tima = 0xFF;
        timer.tma = 0xA5;
        set_timer_input(timer, 0x000F, 0b101);
        timer.tick_t(irq);
        assert_eq!(timer.tima, 0x00);
        assert_eq!(timer.reload, Reload::Pending(4));
    }

    fn assert_no_timer_irq(irq: &mut Interrupts) {
        irq.settle_boundary();
        assert_eq!(irq.pending_mask() & TIMER_IRQ, 0x00);
    }

    fn assert_timer_irq(irq: &mut Interrupts) {
        irq.settle_boundary();
        assert_eq!(irq.pending_mask() & TIMER_IRQ, TIMER_IRQ);
    }

    #[test]
    fn natural_edge_increments_per_selector() {
        for (selector, bit) in [(0b00, 9), (0b01, 3), (0b10, 5), (0b11, 7)] {
            let (mut timer, mut irq) = timer_and_interrupts();
            set_timer_input(&mut timer, (1u16 << bit) - 1, 0b100 | selector);

            timer.tick_t(&mut irq);
            assert_eq!(
                timer.tima, 0,
                "selector {selector:02b}: 0->1 transition must not increment TIMA"
            );

            tick_n(&mut timer, &mut irq, 1usize << bit);
            assert_eq!(
                timer.tima, 1,
                "selector {selector:02b}: 1->0 falling edge increments TIMA"
            );
        }
    }

    #[test]
    fn div_reset_spurious_increment() {
        let (mut timer, mut irq) = timer_and_interrupts();
        timer.tima = 0x22;
        set_timer_input(&mut timer, 1u16 << 9, 0b100);

        timer.write_div();

        assert_eq!(timer.tima, 0x23);
        assert_no_timer_irq(&mut irq);
    }

    #[test]
    fn tac_write_spurious_increment() {
        let (mut timer, mut irq) = timer_and_interrupts();
        timer.tima = 0x40;
        set_timer_input(&mut timer, 0x0008, 0b101);

        timer.write_tac(0x00);

        assert_eq!(timer.tima, 0x41, "true->false TAC write increments TIMA");

        timer.tima = 0x50;
        set_timer_input(&mut timer, 0x0000, 0b101);

        timer.write_tac(0x00);

        assert_eq!(timer.tima, 0x50, "false input TAC write must not increment");
        assert_no_timer_irq(&mut irq);
    }

    #[test]
    fn overflow_reloads_after_four_t_delay() {
        let (mut timer, mut irq) = timer_and_interrupts();

        overflow_to_pending(&mut timer, &mut irq);
        assert_no_timer_irq(&mut irq);

        tick_n(&mut timer, &mut irq, 3);
        assert_eq!(timer.tima, 0x00);
        assert_no_timer_irq(&mut irq);

        timer.tick_t(&mut irq);
        assert_eq!(timer.tima, timer.tma);
        assert_timer_irq(&mut irq);
    }

    #[test]
    fn tima_write_cancels_reload() {
        let (mut timer, mut irq) = timer_and_interrupts();
        overflow_to_pending(&mut timer, &mut irq);

        timer.write_tima(0x42);
        tick_n(&mut timer, &mut irq, 5);

        assert_eq!(timer.tima, 0x42);
        assert_no_timer_irq(&mut irq);
    }

    #[test]
    fn reload_cycle_write_quirks() {
        let (mut timer, mut irq) = timer_and_interrupts();
        overflow_to_pending(&mut timer, &mut irq);

        tick_n(&mut timer, &mut irq, 4);

        assert_eq!(timer.reload, Reload::ReloadedThisTick);
        assert_eq!(timer.tima, 0xA5);

        timer.write_tima(0x66);
        assert_eq!(timer.tima, 0xA5, "TIMA write ignored during reload cycle");

        timer.write_tma(0x77);
        assert_eq!(timer.tima, 0x77, "TMA write copies during reload cycle");
    }

    #[test]
    fn post_boot_dmg_visible_div_is_ab() {
        let timer = Timer::post_boot_dmg();

        assert_eq!(timer.read(0xFF04), 0xAB);
    }

    #[test]
    fn tac_disable_while_selected_bit_high_increments_tima() {
        // mooneye rapid_toggle: disabling the timer (TAC bit 2 -> 0) while the
        // selected DIV bit is HIGH drops the AND result 1->0 = a falling edge =
        // a spurious TIMA increment. Selector 0b01 = bit 3.
        let (mut timer, _irq) = timer_and_interrupts();
        // Enabled, bit 3 high -> AND result currently true.
        set_timer_input(&mut timer, 0x0008, 0b101);
        assert!(timer.timer_input(), "primed: enabled + bit3 high");
        let before = timer.tima;
        // Disable the timer: AND result true->false -> spurious increment.
        timer.write_tac(0b001); // clear bit 2 (enable), keep selector
        assert_eq!(
            timer.tima,
            before.wrapping_add(1),
            "TAC disable while selected bit high increments TIMA"
        );
    }

    #[test]
    fn tac_disable_while_selected_bit_low_does_not_increment() {
        // Disabling while the selected bit is LOW: AND result was already false,
        // so no falling edge, no increment.
        let (mut timer, _irq) = timer_and_interrupts();
        set_timer_input(&mut timer, 0x0000, 0b101); // bit3 low
        assert!(!timer.timer_input());
        let before = timer.tima;
        timer.write_tac(0b001);
        assert_eq!(timer.tima, before, "no edge -> no increment");
    }
}
