use crate::time::ClockSpine;

pub const TIMER_IRQ: u8 = 0x04;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerRegister {
    Div,
    Tima,
    Tma,
    Tac,
}

impl TimerRegister {
    pub const fn from_addr(addr: u16) -> Option<Self> {
        match addr {
            0xFF04 => Some(Self::Div),
            0xFF05 => Some(Self::Tima),
            0xFF06 => Some(Self::Tma),
            0xFF07 => Some(Self::Tac),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timer {
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    previous_and_result: bool,
    reload: Reload,
    if_request: u8,
    observed_cpu_t: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reload {
    None,
    Pending(u8),
    ReloadedThisTick,
}

impl Timer {
    pub fn power_on(spine: &ClockSpine) -> Self {
        Self {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            previous_and_result: false,
            reload: Reload::None,
            if_request: 0,
            observed_cpu_t: spine.cpu_t,
        }
    }

    pub fn observe_spine(&mut self, spine: &ClockSpine) {
        while self.observed_cpu_t < spine.cpu_t {
            self.observed_cpu_t += 1;
            self.tick_from_spine_cpu_t();
        }
    }

    pub fn read(&self, addr: u16) -> Option<u8> {
        Some(match TimerRegister::from_addr(addr)? {
            TimerRegister::Div => (self.div >> 8) as u8,
            TimerRegister::Tima => self.tima,
            TimerRegister::Tma => self.tma,
            TimerRegister::Tac => 0xF8 | (self.tac & 0x07),
        })
    }

    pub fn write(&mut self, addr: u16, value: u8) -> bool {
        match TimerRegister::from_addr(addr) {
            Some(TimerRegister::Div) => self.write_div(),
            Some(TimerRegister::Tima) => self.write_tima(value),
            Some(TimerRegister::Tma) => self.write_tma(value),
            Some(TimerRegister::Tac) => self.write_tac(value),
            None => return false,
        }
        true
    }

    pub const fn interrupt_request(&self) -> u8 {
        self.if_request
    }

    fn tick_from_spine_cpu_t(&mut self) {
        if self.reload == Reload::ReloadedThisTick {
            self.reload = Reload::None;
        }
        let pending_before_tick = matches!(self.reload, Reload::Pending(_));
        // Necessary spec provenance: Pan Docs Timer Obscure Behaviour defines DIV
        // as the visible system counter; this W6 slice intentionally follows
        // rubc-core's proven CPU-T counter cadence while the spine owns ticking.
        self.div = self.div.wrapping_add(1);
        self.apply_falling_edge_after_div_or_tac_change();
        // Necessary spec provenance: Pan Docs Timer overflow behavior delays TMA
        // reload and IF request by one M-cycle, i.e. four CPU-T spine ticks here.
        if pending_before_tick {
            self.advance_reload();
        }
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
        // Necessary spec provenance: Pan Docs' DMG TAC/DIV circuit feeds
        // enable AND selected counter bit into a falling-edge detector.
        (self.tac & 0b100) != 0 && (self.div & (1u16 << Self::selected_bit(self.tac))) != 0
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

    fn advance_reload(&mut self) {
        match self.reload {
            Reload::Pending(1) => {
                self.tima = self.tma;
                self.if_request |= TIMER_IRQ;
                self.reload = Reload::ReloadedThisTick;
            }
            Reload::Pending(n) => self.reload = Reload::Pending(n - 1),
            Reload::None | Reload::ReloadedThisTick => {}
        }
    }

    fn write_div(&mut self) {
        self.div = 0;
        self.apply_falling_edge_after_div_or_tac_change();
    }

    fn write_tac(&mut self, value: u8) {
        self.tac = value & 0x07;
        self.apply_falling_edge_after_div_or_tac_change();
    }

    fn write_tima(&mut self, value: u8) {
        match self.reload {
            Reload::Pending(_) => {
                self.tima = value;
                self.reload = Reload::None;
            }
            Reload::ReloadedThisTick => {}
            Reload::None => self.tima = value,
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
mod tests {
    use super::*;
    use crate::time::ClockSpine;
    use crate::timing::TimingTable;
    use crate::GbModel;

    fn spine_and_timer() -> (ClockSpine, TimingTable, Timer) {
        let spine = ClockSpine::new();
        let table = TimingTable::for_model(GbModel::DmgB);
        let timer = Timer::power_on(&spine);
        (spine, table, timer)
    }

    fn step_cpu_t(spine: &mut ClockSpine, table: &TimingTable, timer: &mut Timer) {
        let target = spine.cpu_t + 1;
        while spine.cpu_t < target {
            spine.step_subphase(table);
            timer.observe_spine(spine);
        }
    }

    fn step_cpu_t_n(spine: &mut ClockSpine, table: &TimingTable, timer: &mut Timer, count: usize) {
        for _ in 0..count {
            step_cpu_t(spine, table, timer);
        }
    }

    fn set_timer_input(timer: &mut Timer, div: u16, tac: u8) {
        timer.div = div;
        timer.tac = tac & 0x07;
        timer.previous_and_result = timer.timer_input();
        timer.reload = Reload::None;
    }

    #[test]
    fn tima_increments_on_selected_div_falling_edge() {
        let (mut spine, table, mut timer) = spine_and_timer();
        set_timer_input(&mut timer, 0x000F, 0b101);

        step_cpu_t(&mut spine, &table, &mut timer);

        assert_eq!(
            timer.tima, 1,
            "Pan Docs Timer Obscure Behaviour: TIMA increments on selected DIV bit 3 falling edge"
        );
    }

    #[test]
    fn tima_does_not_increment_on_rising_edge_wrong_expected_probe() {
        let (mut spine, table, mut timer) = spine_and_timer();
        set_timer_input(&mut timer, 0x0007, 0b101);

        step_cpu_t(&mut spine, &table, &mut timer);

        assert_ne!(
            timer.tima, 1,
            "falsifiability probe: a wrong rising-edge oracle would incorrectly expect an increment"
        );
        assert_eq!(timer.tima, 0, "rising edge is not a timer tick");
    }

    #[test]
    fn div_write_while_selected_bit_high_causes_spurious_increment() {
        let (_spine, _table, mut timer) = spine_and_timer();
        timer.tima = 0x22;
        set_timer_input(&mut timer, 1 << 9, 0b100);

        assert!(timer.write(0xFF04, 0x99));

        assert_eq!(
            timer.tima, 0x23,
            "Pan Docs Timer Obscure Behaviour: DIV reset drops selected bit high->low and ticks TIMA"
        );
    }

    #[test]
    fn tac_disable_while_dmg_selected_bit_high_causes_spurious_increment() {
        let (_spine, _table, mut timer) = spine_and_timer();
        timer.tima = 0x40;
        set_timer_input(&mut timer, 0x0008, 0b101);

        assert!(timer.write(0xFF07, 0b001));

        assert_eq!(
            timer.tima, 0x41,
            "Pan Docs Timer Obscure Behaviour: DMG TAC disable while selected bit high ticks TIMA"
        );
    }

    #[test]
    fn overflow_reloads_tma_and_requests_if_after_four_cpu_t() {
        let (mut spine, table, mut timer) = spine_and_timer();
        timer.tima = 0xFF;
        timer.tma = 0xA5;
        set_timer_input(&mut timer, 0x000F, 0b101);

        step_cpu_t(&mut spine, &table, &mut timer);
        assert_eq!(
            timer.tima, 0x00,
            "overflow leaves TIMA zero during reload delay"
        );
        assert_eq!(timer.interrupt_request() & TIMER_IRQ, 0x00);

        step_cpu_t_n(&mut spine, &table, &mut timer, 3);
        assert_eq!(
            timer.tima, 0x00,
            "wrong 3-T reload-delay perturbation would reload one tick too early"
        );
        assert_eq!(timer.interrupt_request() & TIMER_IRQ, 0x00);

        step_cpu_t(&mut spine, &table, &mut timer);
        assert_eq!(
            timer.tima, 0xA5,
            "TMA reload occurs on the fourth CPU-T after overflow"
        );
        assert_eq!(timer.interrupt_request() & TIMER_IRQ, TIMER_IRQ);
    }

    #[test]
    fn tima_write_during_pending_reload_cancels_reload_and_interrupt() {
        let (mut spine, table, mut timer) = spine_and_timer();
        timer.tima = 0xFF;
        timer.tma = 0xA5;
        set_timer_input(&mut timer, 0x000F, 0b101);

        step_cpu_t(&mut spine, &table, &mut timer);
        assert!(timer.write(0xFF05, 0x42));
        step_cpu_t_n(&mut spine, &table, &mut timer, 5);

        assert_eq!(
            timer.tima, 0x42,
            "Pan Docs cycle A: TIMA write cancels reload"
        );
        assert_eq!(timer.interrupt_request() & TIMER_IRQ, 0x00);
    }

    #[test]
    fn reload_cycle_ignores_tima_write_and_tma_write_copies_to_tima() {
        let (mut spine, table, mut timer) = spine_and_timer();
        timer.tima = 0xFF;
        timer.tma = 0xA5;
        set_timer_input(&mut timer, 0x000F, 0b101);

        step_cpu_t(&mut spine, &table, &mut timer);
        step_cpu_t_n(&mut spine, &table, &mut timer, 4);

        assert_eq!(timer.tima, 0xA5);
        assert!(timer.write(0xFF05, 0x66));
        assert_eq!(timer.tima, 0xA5, "Pan Docs cycle B: TIMA write is ignored");

        assert!(timer.write(0xFF06, 0x77));
        assert_eq!(
            timer.tima, 0x77,
            "Pan Docs cycle B: TMA write also copies into TIMA"
        );
    }

    #[test]
    fn rapid_toggle_tac_selector_ticks_only_on_true_to_false_mux_change() {
        let (_spine, _table, mut timer) = spine_and_timer();
        timer.tima = 0x10;
        set_timer_input(&mut timer, 0x3FF0, 0b100);

        assert!(timer.write(0xFF07, 0x05));
        assert_eq!(
            timer.tima, 0x11,
            "Pan Docs example: $3FF0/$FC -> TAC $05 ticks"
        );

        assert!(timer.write(0xFF07, 0x07));
        assert_eq!(
            timer.tima, 0x11,
            "same example: switching to a set bit must not tick"
        );
    }
}
