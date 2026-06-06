#[derive(Debug, Clone)]
pub struct Apu {
    pub t_ticks: u64,
    powered: bool,
    frame_step: u8,
    ch1: Channel1,
    ch2: PulseChannel,
    ch3: WaveChannel,
    ch4: NoiseChannel,
    nr50: u8,
    nr51: u8,
    wave_ram: [u8; 0x10],
}

impl Default for Apu {
    fn default() -> Self {
        Self {
            t_ticks: 0,
            powered: true,
            frame_step: 0,
            ch1: Channel1::default(),
            ch2: PulseChannel::default(),
            ch3: WaveChannel::default(),
            ch4: NoiseChannel::default(),
            nr50: 0,
            nr51: 0,
            wave_ram: [0; 0x10],
        }
    }
}

impl Apu {
    pub fn power(&self) -> bool {
        self.powered
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF10 => 0x80 | self.ch1.sweep.nr10,
            0xFF11 => self.ch1.pulse.read_duty_length(),
            0xFF12 => self.ch1.pulse.envelope.reg,
            0xFF13 => 0xFF,
            0xFF14 => self.ch1.pulse.read_control(),
            0xFF15 => 0xFF,
            0xFF16 => self.ch2.read_duty_length(),
            0xFF17 => self.ch2.envelope.reg,
            0xFF18 => 0xFF,
            0xFF19 => self.ch2.read_control(),
            0xFF1A => self.ch3.read_dac(),
            0xFF1B => 0xFF,
            0xFF1C => 0x9F | self.ch3.nr32,
            0xFF1D => 0xFF,
            0xFF1E => self.ch3.read_control(),
            0xFF1F => 0xFF,
            0xFF20 => 0xFF,
            0xFF21 => self.ch4.envelope.reg,
            0xFF22 => self.ch4.nr43,
            0xFF23 => self.ch4.read_control(),
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => self.read_nr52(),
            0xFF30..=0xFF3F => self.wave_ram[(addr - 0xFF30) as usize],
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        if (0xFF30..=0xFF3F).contains(&addr) {
            self.wave_ram[(addr - 0xFF30) as usize] = value;
            return;
        }

        if addr == 0xFF26 {
            self.write_nr52(value);
            return;
        }

        if !self.powered {
            return;
        }

        let next_step_clocks_length = Self::step_clocks_length(self.frame_step);
        match addr {
            0xFF10 => self.write_nr10(value),
            0xFF11 => self.ch1.pulse.write_duty_length(value),
            0xFF12 => self.ch1.pulse.write_envelope(value),
            0xFF13 => self.ch1.pulse.freq_low = value,
            0xFF14 => {
                if self.ch1.pulse.write_control(value, next_step_clocks_length) {
                    self.trigger_ch1(next_step_clocks_length);
                }
            }
            0xFF16 => self.ch2.write_duty_length(value),
            0xFF17 => self.ch2.write_envelope(value),
            0xFF18 => self.ch2.freq_low = value,
            0xFF19 => {
                if self.ch2.write_control(value, next_step_clocks_length) {
                    self.ch2.trigger(next_step_clocks_length);
                }
            }
            0xFF1A => self.ch3.write_dac(value),
            0xFF1B => self.ch3.write_length(value),
            0xFF1C => self.ch3.nr32 = value & 0x60,
            0xFF1D => self.ch3.freq_low = value,
            0xFF1E => {
                if self.ch3.write_control(value, next_step_clocks_length) {
                    self.ch3.trigger(next_step_clocks_length);
                }
            }
            0xFF20 => self.ch4.write_length(value),
            0xFF21 => self.ch4.write_envelope(value),
            0xFF22 => self.ch4.nr43 = value,
            0xFF23 => {
                if self.ch4.write_control(value, next_step_clocks_length) {
                    self.ch4.trigger(next_step_clocks_length);
                }
            }
            0xFF24 => self.nr50 = value,
            0xFF25 => self.nr51 = value,
            _ => {}
        }
    }

    pub fn tick_div_apu(&mut self) {
        let step = self.frame_step;
        if self.powered {
            if Self::step_clocks_length(step) {
                self.ch1.pulse.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
            }
            if matches!(step, 2 | 6) {
                self.ch1.clock_sweep();
            }
            if matches!(step, 5 | 7) {
                self.ch1.pulse.envelope.clock();
                self.ch2.envelope.clock();
                self.ch4.envelope.clock();
            }
        }
        self.frame_step = (self.frame_step + 1) & 0x07;
    }

    pub fn tick_t(&mut self) {
        self.t_ticks += 1;
        if !self.powered {
            return;
        }
        self.ch1.pulse.tick_t();
        self.ch2.tick_t();
        self.ch3.tick_t();
        self.ch4.tick_t();
    }

    fn read_nr52(&self) -> u8 {
        let power = if self.powered { 0x80 } else { 0x00 };
        power | 0x70 | self.status_bits()
    }

    fn status_bits(&self) -> u8 {
        if !self.powered {
            return 0;
        }
        (self.ch1.pulse.enabled as u8)
            | ((self.ch2.enabled as u8) << 1)
            | ((self.ch3.enabled as u8) << 2)
            | ((self.ch4.enabled as u8) << 3)
    }

    fn write_nr52(&mut self, value: u8) {
        let new_power = value & 0x80 != 0;
        match (self.powered, new_power) {
            (true, false) => self.power_off(),
            (false, true) => self.power_on(),
            _ => {}
        }
    }

    fn power_off(&mut self) {
        self.powered = false;
        self.nr50 = 0;
        self.nr51 = 0;
        self.ch1.power_off();
        self.ch2.power_off();
        self.ch3.power_off();
        self.ch4.power_off();
    }

    fn power_on(&mut self) {
        self.powered = true;
        self.nr50 = 0;
        self.nr51 = 0;
        self.ch1.power_off();
        self.ch2.power_off();
        self.ch3.power_off();
        self.ch4.power_off();
    }

    fn write_nr10(&mut self, value: u8) {
        let old_negate = self.ch1.sweep.negate();
        self.ch1.sweep.nr10 = value & 0x7F;
        if old_negate && !self.ch1.sweep.negate() && self.ch1.sweep.negate_calculated {
            self.ch1.pulse.enabled = false;
        }
    }

    fn trigger_ch1(&mut self, next_step_clocks_length: bool) {
        self.ch1.pulse.trigger(next_step_clocks_length);
        self.ch1.sweep.trigger(self.ch1.pulse.frequency());
        if self.ch1.sweep.shift() != 0 && self.ch1.calculate_sweep().is_none() {
            self.ch1.pulse.enabled = false;
        }
    }

    fn step_clocks_length(step: u8) -> bool {
        matches!(step, 0 | 2 | 4 | 6)
    }
}

#[derive(Debug, Clone, Default)]
struct Channel1 {
    pulse: PulseChannel,
    sweep: Sweep,
}

impl Channel1 {
    fn clock_sweep(&mut self) {
        if self.sweep.timer > 0 {
            self.sweep.timer -= 1;
        }
        if self.sweep.timer != 0 {
            return;
        }

        self.sweep.timer = self.sweep.period_reload();
        if !self.sweep.enabled || self.sweep.pace() == 0 {
            return;
        }

        let Some(new_frequency) = self.calculate_sweep() else {
            self.pulse.enabled = false;
            return;
        };
        if self.sweep.shift() != 0 {
            self.sweep.shadow_frequency = new_frequency;
            self.pulse.set_frequency(new_frequency);
            if self.calculate_sweep().is_none() {
                self.pulse.enabled = false;
            }
        }
    }

    fn calculate_sweep(&mut self) -> Option<u16> {
        let delta = self.sweep.shadow_frequency >> self.sweep.shift();
        let new_frequency = if self.sweep.negate() {
            self.sweep.negate_calculated = true;
            self.sweep.shadow_frequency.wrapping_sub(delta)
        } else {
            self.sweep.shadow_frequency.wrapping_add(delta)
        };
        if new_frequency > 2047 {
            None
        } else {
            Some(new_frequency)
        }
    }

    fn power_off(&mut self) {
        self.pulse.power_off();
        self.sweep = Sweep::default();
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Sweep {
    nr10: u8,
    shadow_frequency: u16,
    timer: u8,
    enabled: bool,
    negate_calculated: bool,
}

impl Sweep {
    fn pace(&self) -> u8 {
        (self.nr10 >> 4) & 0x07
    }

    fn negate(&self) -> bool {
        self.nr10 & 0x08 != 0
    }

    fn shift(&self) -> u8 {
        self.nr10 & 0x07
    }

    fn period_reload(&self) -> u8 {
        let pace = self.pace();
        if pace == 0 {
            8
        } else {
            pace
        }
    }

    fn trigger(&mut self, frequency: u16) {
        self.shadow_frequency = frequency;
        self.timer = self.period_reload();
        self.enabled = self.pace() != 0 || self.shift() != 0;
        self.negate_calculated = false;
    }
}

#[derive(Debug, Clone, Default)]
struct PulseChannel {
    duty_length: u8,
    envelope: Envelope,
    freq_low: u8,
    control: u8,
    length_counter: u16,
    enabled: bool,
    duty_pos: u8,
    timer: u16,
}

impl PulseChannel {
    fn read_duty_length(&self) -> u8 {
        (self.duty_length & 0xC0) | 0x3F
    }

    fn read_control(&self) -> u8 {
        0xBF | (self.control & 0x40)
    }

    fn write_duty_length(&mut self, value: u8) {
        self.duty_length = value;
        self.length_counter = 64 - u16::from(value & 0x3F);
    }

    fn write_envelope(&mut self, value: u8) {
        self.envelope.reg = value;
        if !self.envelope.dac_enabled() {
            self.enabled = false;
        }
    }

    fn write_control(&mut self, value: u8, next_step_clocks_length: bool) -> bool {
        let old_length_enabled = self.length_enabled();
        self.control = value & 0x47;
        if !old_length_enabled && self.length_enabled() && !next_step_clocks_length {
            self.extra_length_clock();
        }
        value & 0x80 != 0
    }

    fn trigger(&mut self, next_step_clocks_length: bool) {
        if self.length_counter == 0 {
            self.length_counter = 64;
            // Trigger-time length quirk: if length is enabled and the next
            // DIV-APU step won't clock the length timer, the freshly reloaded
            // counter is decremented by one (64 -> 63).
            if self.length_enabled() && !next_step_clocks_length {
                self.length_counter -= 1;
            }
        }
        self.timer = self.period();
        self.envelope.trigger();
        self.enabled = self.envelope.dac_enabled();
    }

    fn tick_t(&mut self) {
        if !self.enabled {
            return;
        }
        if self.timer == 0 {
            self.timer = self.period();
        }
        self.timer -= 1;
        if self.timer == 0 {
            self.timer = self.period();
            self.duty_pos = (self.duty_pos + 1) & 0x07;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled() && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn extra_length_clock(&mut self) {
        if self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn length_enabled(&self) -> bool {
        self.control & 0x40 != 0
    }

    fn frequency(&self) -> u16 {
        (u16::from(self.control & 0x07) << 8) | u16::from(self.freq_low)
    }

    fn set_frequency(&mut self, frequency: u16) {
        let freq = frequency & 0x07FF;
        self.freq_low = freq as u8;
        self.control = (self.control & !0x07) | ((freq >> 8) as u8 & 0x07);
    }

    fn period(&self) -> u16 {
        (2048 - self.frequency()) * 4
    }

    fn power_off(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Envelope {
    reg: u8,
    volume: u8,
    timer: u8,
}

impl Envelope {
    fn dac_enabled(&self) -> bool {
        self.reg & 0xF8 != 0
    }

    fn trigger(&mut self) {
        self.volume = self.reg >> 4;
        self.timer = self.period();
    }

    fn clock(&mut self) {
        let period = self.period();
        if period == 0 {
            return;
        }
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer != 0 {
            return;
        }

        self.timer = period;
        if self.increases() {
            if self.volume < 15 {
                self.volume += 1;
            }
        } else if self.volume > 0 {
            self.volume -= 1;
        }
    }

    fn period(&self) -> u8 {
        self.reg & 0x07
    }

    fn increases(&self) -> bool {
        self.reg & 0x08 != 0
    }
}

#[derive(Debug, Clone, Default)]
struct WaveChannel {
    nr30: u8,
    length_load: u8,
    nr32: u8,
    freq_low: u8,
    control: u8,
    length_counter: u16,
    enabled: bool,
    sample_index: u8,
    timer: u16,
}

impl WaveChannel {
    fn read_dac(&self) -> u8 {
        (self.nr30 & 0x80) | 0x7F
    }

    fn read_control(&self) -> u8 {
        0xBF | (self.control & 0x40)
    }

    fn write_dac(&mut self, value: u8) {
        self.nr30 = value & 0x80;
        if !self.dac_enabled() {
            self.enabled = false;
        }
    }

    fn write_length(&mut self, value: u8) {
        self.length_load = value;
        self.length_counter = 256 - u16::from(value);
    }

    fn write_control(&mut self, value: u8, next_step_clocks_length: bool) -> bool {
        let old_length_enabled = self.length_enabled();
        self.control = value & 0x47;
        if !old_length_enabled && self.length_enabled() && !next_step_clocks_length {
            self.extra_length_clock();
        }
        value & 0x80 != 0
    }

    fn trigger(&mut self, next_step_clocks_length: bool) {
        if self.length_counter == 0 {
            self.length_counter = 256;
            if self.length_enabled() && !next_step_clocks_length {
                self.length_counter -= 1;
            }
        }
        self.sample_index = 0;
        self.timer = self.period();
        self.enabled = self.dac_enabled();
    }

    fn tick_t(&mut self) {
        if !self.enabled {
            return;
        }
        if self.timer == 0 {
            self.timer = self.period();
        }
        self.timer -= 1;
        if self.timer == 0 {
            self.timer = self.period();
            self.sample_index = (self.sample_index + 1) & 0x1F;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled() && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn extra_length_clock(&mut self) {
        if self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn length_enabled(&self) -> bool {
        self.control & 0x40 != 0
    }

    fn dac_enabled(&self) -> bool {
        self.nr30 & 0x80 != 0
    }

    fn frequency(&self) -> u16 {
        (u16::from(self.control & 0x07) << 8) | u16::from(self.freq_low)
    }

    fn period(&self) -> u16 {
        (2048 - self.frequency()) * 2
    }

    fn power_off(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Default)]
struct NoiseChannel {
    length_load: u8,
    envelope: Envelope,
    nr43: u8,
    control: u8,
    length_counter: u16,
    enabled: bool,
    timer: u32,
    lfsr: u16,
}

impl NoiseChannel {
    fn read_control(&self) -> u8 {
        0xBF | (self.control & 0x40)
    }

    fn write_length(&mut self, value: u8) {
        self.length_load = value & 0x3F;
        self.length_counter = 64 - u16::from(value & 0x3F);
    }

    fn write_envelope(&mut self, value: u8) {
        self.envelope.reg = value;
        if !self.envelope.dac_enabled() {
            self.enabled = false;
        }
    }

    fn write_control(&mut self, value: u8, next_step_clocks_length: bool) -> bool {
        let old_length_enabled = self.length_enabled();
        self.control = value & 0x40;
        if !old_length_enabled && self.length_enabled() && !next_step_clocks_length {
            self.extra_length_clock();
        }
        value & 0x80 != 0
    }

    fn trigger(&mut self, next_step_clocks_length: bool) {
        if self.length_counter == 0 {
            self.length_counter = 64;
            if self.length_enabled() && !next_step_clocks_length {
                self.length_counter -= 1;
            }
        }
        self.envelope.trigger();
        self.timer = self.period();
        self.lfsr = 0;
        self.enabled = self.envelope.dac_enabled();
    }

    fn tick_t(&mut self) {
        if !self.enabled {
            return;
        }
        if self.timer == 0 {
            self.timer = self.period();
        }
        self.timer -= 1;
        if self.timer == 0 {
            self.timer = self.period();
            self.clock_lfsr();
        }
    }

    fn clock_lfsr(&mut self) {
        let feedback = (self.lfsr ^ (self.lfsr >> 1)) & 0x01;
        self.lfsr = (self.lfsr >> 1) | (feedback << 14);
        if self.nr43 & 0x08 != 0 {
            self.lfsr = (self.lfsr & !(1 << 6)) | (feedback << 6);
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled() && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn extra_length_clock(&mut self) {
        if self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn length_enabled(&self) -> bool {
        self.control & 0x40 != 0
    }

    fn period(&self) -> u32 {
        const DIVISORS: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];
        let divisor = DIVISORS[(self.nr43 & 0x07) as usize];
        divisor << (self.nr43 >> 4)
    }

    fn power_off(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick_div_steps(apu: &mut Apu, steps: usize) {
        for _ in 0..steps {
            apu.tick_div_apu();
        }
    }

    #[test]
    fn register_read_masks_match_dmg_bits() {
        let mut apu = Apu::default();

        apu.write(0xFF10, 0x7F);
        apu.write(0xFF11, 0x80);
        apu.write(0xFF12, 0xA5);
        apu.write(0xFF13, 0x12);
        apu.write(0xFF14, 0x47);
        apu.write(0xFF16, 0x40);
        apu.write(0xFF17, 0x5A);
        apu.write(0xFF18, 0x34);
        apu.write(0xFF19, 0x00);
        apu.write(0xFF1A, 0x80);
        apu.write(0xFF1B, 0x56);
        apu.write(0xFF1C, 0x20);
        apu.write(0xFF1D, 0x78);
        apu.write(0xFF1E, 0x40);
        apu.write(0xFF20, 0x3F);
        apu.write(0xFF21, 0xC3);
        apu.write(0xFF22, 0x2D);
        apu.write(0xFF23, 0x40);
        apu.write(0xFF24, 0x77);
        apu.write(0xFF25, 0xF3);
        apu.write(0xFF30, 0xAC);

        assert_eq!(apu.read(0xFF10), 0xFF);
        assert_eq!(apu.read(0xFF11), 0xBF);
        assert_eq!(apu.read(0xFF12), 0xA5);
        assert_eq!(apu.read(0xFF13), 0xFF);
        assert_eq!(apu.read(0xFF14), 0xFF);
        assert_eq!(apu.read(0xFF16), 0x7F);
        assert_eq!(apu.read(0xFF17), 0x5A);
        assert_eq!(apu.read(0xFF18), 0xFF);
        assert_eq!(apu.read(0xFF19), 0xBF);
        assert_eq!(apu.read(0xFF1A), 0xFF);
        assert_eq!(apu.read(0xFF1B), 0xFF);
        assert_eq!(apu.read(0xFF1C), 0xBF);
        assert_eq!(apu.read(0xFF1D), 0xFF);
        assert_eq!(apu.read(0xFF1E), 0xFF);
        assert_eq!(apu.read(0xFF20), 0xFF);
        assert_eq!(apu.read(0xFF21), 0xC3);
        assert_eq!(apu.read(0xFF22), 0x2D);
        assert_eq!(apu.read(0xFF23), 0xFF);
        assert_eq!(apu.read(0xFF24), 0x77);
        assert_eq!(apu.read(0xFF25), 0xF3);
        assert_eq!(apu.read(0xFF26), 0xF0);
        assert_eq!(apu.read(0xFF30), 0xAC);
    }

    #[test]
    fn power_off_clears_registers_ignores_nr_writes_and_preserves_wave_ram() {
        let mut apu = Apu::default();

        apu.write(0xFF12, 0xF3);
        apu.write(0xFF24, 0x77);
        apu.write(0xFF30, 0xA5);
        apu.write(0xFF26, 0x00);

        assert!(!apu.power());
        assert_eq!(apu.read(0xFF12), 0x00);
        assert_eq!(apu.read(0xFF24), 0x00);
        assert_eq!(apu.read(0xFF26), 0x70);
        assert_eq!(apu.read(0xFF30), 0xA5);

        apu.write(0xFF12, 0xF3);
        apu.write(0xFF30, 0x5A);

        assert_eq!(apu.read(0xFF12), 0x00);
        assert_eq!(apu.read(0xFF30), 0x5A);

        apu.write(0xFF26, 0x80);
        apu.write(0xFF12, 0xF3);

        assert!(apu.power());
        assert_eq!(apu.read(0xFF12), 0xF3);
    }

    #[test]
    fn length_counter_decrements_and_disables_channel() {
        let mut apu = Apu::default();

        apu.write(0xFF21, 0xF0);
        apu.write(0xFF20, 0x3F);
        apu.write(0xFF23, 0xC0);

        assert_eq!(apu.read(0xFF26) & 0x08, 0x08);
        assert_eq!(apu.ch4.length_counter, 1);

        apu.tick_div_apu();

        assert_eq!(apu.ch4.length_counter, 0);
        assert_eq!(apu.read(0xFF26) & 0x08, 0x00);
    }

    #[test]
    fn trigger_reloads_expired_length_and_sets_status_when_dac_is_on() {
        let mut apu = Apu::default();

        apu.write(0xFF17, 0xF0);
        apu.write(0xFF19, 0x80);

        assert_eq!(apu.ch2.length_counter, 64);
        assert_eq!(apu.read(0xFF26) & 0x02, 0x02);
    }

    #[test]
    fn extra_length_clock_fires_when_enabling_before_non_length_step() {
        let mut apu = Apu::default();
        apu.frame_step = 1;

        apu.write(0xFF17, 0xF0);
        apu.write(0xFF16, 0x3F);
        apu.write(0xFF19, 0x80);
        apu.write(0xFF19, 0x40);

        assert_eq!(apu.ch2.length_counter, 0);
        assert_eq!(apu.read(0xFF26) & 0x02, 0x00);
    }

    #[test]
    fn envelope_clocks_on_steps_five_and_seven() {
        let mut apu = Apu::default();

        apu.write(0xFF17, 0x19);
        apu.write(0xFF19, 0x80);

        assert_eq!(apu.ch2.envelope.volume, 1);

        tick_div_steps(&mut apu, 6);

        assert_eq!(apu.ch2.envelope.volume, 2);

        tick_div_steps(&mut apu, 2);

        assert_eq!(apu.ch2.envelope.volume, 3);
    }

    #[test]
    fn ch1_sweep_overflow_on_trigger_disables_channel_even_with_zero_pace() {
        let mut apu = Apu::default();

        apu.write(0xFF10, 0x01);
        apu.write(0xFF12, 0xF0);
        apu.write(0xFF13, 0xDC);
        apu.write(0xFF14, 0x85);

        assert_eq!(apu.read(0xFF26) & 0x01, 0x00);
        assert!(!apu.ch1.pulse.enabled);
    }

    #[test]
    fn nr52_reports_channel_status_bits() {
        let mut apu = Apu::default();

        apu.write(0xFF12, 0xF0);
        apu.write(0xFF14, 0x80);
        apu.write(0xFF17, 0xF0);
        apu.write(0xFF19, 0x80);
        apu.write(0xFF1A, 0x80);
        apu.write(0xFF1E, 0x80);
        apu.write(0xFF21, 0xF0);
        apu.write(0xFF23, 0x80);

        assert_eq!(apu.read(0xFF26), 0xFF);

        apu.write(0xFF17, 0x00);

        assert_eq!(apu.read(0xFF26) & 0x0F, 0x0D);
    }
}
