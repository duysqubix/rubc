#[derive(Debug, Clone)]
pub struct Apu {
    pub t_ticks: u64,
    powered: bool,
    frame_step: u8,
    prev_div_apu_high: bool,
    double_speed_apu_gate: bool,
    ch1: Channel1,
    ch2: PulseChannel,
    ch3: WaveChannel,
    ch4: NoiseChannel,
    nr50: u8,
    nr51: u8,
    wave_ram: [u8; 0x10],
    /// Stereo output samples (interleaved L,R as f32 in [-1.0, 1.0]) collected
    /// at the target output rate via the downsample accumulator below.
    sample_buffer: Vec<f32>,
    /// Fractional accumulator for downsampling the 4.194304 MHz APU tick rate
    /// down to `t_per_sample`. When it crosses `t_per_sample` a stereo sample
    /// is emitted.
    sample_accum: f32,
    /// Ticks per output sample = 4_194_304 / output_sample_rate. 0 disables
    /// sample collection (e.g. headless test runs that only check registers).
    t_per_sample: f32,
}

impl Default for Apu {
    fn default() -> Self {
        Self {
            t_ticks: 0,
            powered: true,
            frame_step: 0,
            prev_div_apu_high: false,
            double_speed_apu_gate: false,
            ch1: Channel1::default(),
            ch2: PulseChannel::default(),
            ch3: WaveChannel::default(),
            ch4: NoiseChannel::default(),
            nr50: 0,
            nr51: 0,
            wave_ram: [0; 0x10],
            sample_buffer: Vec::new(),
            sample_accum: 0.0,
            t_per_sample: 0.0,
        }
    }
}

impl Apu {
    pub fn power(&self) -> bool {
        self.powered
    }

    /// Enable audio sample collection at `rate` Hz (e.g. 48000). Pass 0 to
    /// disable (headless/test runs). `tick_t` runs once per T-cycle = the full
    /// 4_194_304 Hz GameBoy clock (in CGB double-speed the APU is gated to every
    /// 2nd T, the same wall-clock rate).
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.t_per_sample = if rate == 0 {
            0.0
        } else {
            4_194_304.0 / rate as f32
        };
        self.sample_accum = 0.0;
        self.sample_buffer.clear();
    }

    /// Drain the collected stereo samples (interleaved L,R f32 in [-1.0, 1.0]).
    pub fn drain_samples(&mut self, out: &mut Vec<f32>) {
        out.append(&mut self.sample_buffer);
    }

    /// Number of stereo frames currently buffered.
    pub fn buffered_frames(&self) -> usize {
        self.sample_buffer.len() / 2
    }

    /// Mix the four channel DACs into a stereo pair, applying NR51 panning and
    /// NR50 master volume. Output range approximately [-1.0, 1.0].
    fn mix_sample(&self) -> (f32, f32) {
        if !self.powered {
            return (0.0, 0.0);
        }
        let c1 = self.ch1.pulse.dac_output();
        let c2 = self.ch2.dac_output();
        let c3 = self.ch3.dac_output();
        let c4 = self.ch4.dac_output();

        // NR51: bits 0-3 = right channel enables (CH1-4), bits 4-7 = left.
        let n = self.nr51;
        let right = (((n & 0x01) != 0) as u8 as f32) * c1
            + (((n & 0x02) != 0) as u8 as f32) * c2
            + (((n & 0x04) != 0) as u8 as f32) * c3
            + (((n & 0x08) != 0) as u8 as f32) * c4;
        let left = (((n & 0x10) != 0) as u8 as f32) * c1
            + (((n & 0x20) != 0) as u8 as f32) * c2
            + (((n & 0x40) != 0) as u8 as f32) * c3
            + (((n & 0x80) != 0) as u8 as f32) * c4;

        // NR50: bits 0-2 = right master volume, bits 4-6 = left (0-7 -> +1).
        let right_vol = ((self.nr50 & 0x07) + 1) as f32 / 8.0;
        let left_vol = (((self.nr50 >> 4) & 0x07) + 1) as f32 / 8.0;

        // Each side sums up to 4 channels; normalize by 4 to keep within ~[-1,1].
        let l = (left / 4.0) * left_vol;
        let r = (right / 4.0) * right_vol;
        (l, r)
    }

    /// Collect one output sample if the downsample accumulator has crossed the
    /// per-sample T-cycle threshold. Called once per T-cycle from `tick_t`.
    fn collect_sample(&mut self) {
        if self.t_per_sample == 0.0 {
            return;
        }
        self.sample_accum += 1.0;
        if self.sample_accum >= self.t_per_sample {
            self.sample_accum -= self.t_per_sample;
            let (l, r) = self.mix_sample();
            self.sample_buffer.push(l);
            self.sample_buffer.push(r);
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        self.read_for_model(addr, false)
    }

    pub fn read_for_model(&self, addr: u16, cgb: bool) -> u8 {
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
            0xFF30..=0xFF3F => self.read_wave_ram(addr, cgb),
            _ => 0xFF,
        }
    }

    fn read_wave_ram(&self, addr: u16, cgb: bool) -> u8 {
        let Some(idx) = self.wave_ram_access_index(addr, cgb) else {
            return 0xFF;
        };
        self.wave_ram[idx]
    }

    fn write_wave_ram(&mut self, addr: u16, value: u8, cgb: bool) {
        if let Some(idx) = self.wave_ram_access_index(addr, cgb) {
            self.wave_ram[idx] = value;
        }
    }

    fn wave_ram_access_index(&self, addr: u16, cgb: bool) -> Option<usize> {
        if !self.ch3.enabled {
            return Some((addr - 0xFF30) as usize);
        }
        if cgb {
            return Some(self.ch3.current_wave_byte_index());
        }
        self.ch3.wave_access_byte()
    }

    pub fn write(&mut self, addr: u16, value: u8, cgb: bool) {
        if (0xFF30..=0xFF3F).contains(&addr) {
            self.write_wave_ram(addr, value, cgb);
            return;
        }

        if addr == 0xFF26 {
            self.write_nr52(value, cgb);
            return;
        }

        if !self.powered {
            // On monochrome (DMG) models the length-load registers (NRx1)
            // remain writable while the APU is powered off; only the length
            // value (low 6 bits, or all 8 for wave) is honoured. All other
            // registers ignore writes while off. On CGB everything is locked.
            if !cgb {
                match addr {
                    0xFF11 => self.ch1.pulse.write_length_only(value),
                    0xFF16 => self.ch2.write_length_only(value),
                    0xFF1B => self.ch3.write_length(value),
                    0xFF20 => self.ch4.write_length(value),
                    _ => {}
                }
            }
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
                    if !cgb {
                        self.corrupt_wave_ram_on_dmg_wave_trigger();
                    }
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

    /// Advance the APU from the rubc-ng timing spine.
    ///
    /// The waveform timers advance on APU wall-clock T-cycles. In CGB double-speed
    /// the CPU spine ticks twice as fast, so the APU ticks every second CPU T. The
    /// frame sequencer is owned by DIV-APU: Pan Docs Audio_details specifies the
    /// 512 Hz sequencer clocks on the falling edge of bit 4 of visible DIV; because
    /// `$FF04 = internal_div >> 8`, that is bit 12 (`0x1000`) of the 16-bit divider,
    /// or bit 13 (`0x2000`) in CGB double-speed.
    pub fn tick_spine(&mut self, div_counter: u16, double_speed: bool) {
        self.observe_div_apu_counter_change_from_previous(div_counter, double_speed);

        if double_speed {
            self.double_speed_apu_gate = !self.double_speed_apu_gate;
            if self.double_speed_apu_gate {
                return;
            }
        }
        self.tick_t();
    }

    pub fn observe_div_apu_counter_change(&mut self, before: u16, after: u16, double_speed: bool) {
        let before_high = Self::div_apu_bit_high(before, double_speed);
        let after_high = Self::div_apu_bit_high(after, double_speed);
        if before_high && !after_high {
            self.tick_div_apu();
        }
        self.prev_div_apu_high = after_high;
    }

    fn observe_div_apu_counter_change_from_previous(
        &mut self,
        div_counter: u16,
        double_speed: bool,
    ) {
        let div_apu_high = Self::div_apu_bit_high(div_counter, double_speed);
        if self.prev_div_apu_high && !div_apu_high {
            self.tick_div_apu();
        }
        self.prev_div_apu_high = div_apu_high;
    }

    fn div_apu_bit_high(div_counter: u16, double_speed: bool) -> bool {
        let mask = if double_speed { 0x2000 } else { 0x1000 };
        div_counter & mask != 0
    }

    pub fn tick_t(&mut self) {
        self.t_ticks += 1;
        if self.powered {
            self.ch1.pulse.tick_t();
            self.ch2.tick_t();
            self.ch3.tick_t(&self.wave_ram);
            self.ch4.tick_t();
        }
        // Always advance the sample clock so the output rate stays constant even
        // while the APU is powered off (mix_sample emits silence then).
        self.collect_sample();
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

    fn write_nr52(&mut self, value: u8, cgb: bool) {
        let new_power = value & 0x80 != 0;
        match (self.powered, new_power) {
            (true, false) => self.power_off(cgb),
            (false, true) => self.power_on(cgb),
            _ => {}
        }
    }

    fn power_off(&mut self, cgb: bool) {
        self.powered = false;
        self.nr50 = 0;
        self.nr51 = 0;
        // On monochrome (DMG) models the length timers (NRx1) are NOT cleared
        // by powering the APU off; on CGB they are. (Pan Docs Audio_Registers.)
        let preserve_length = !cgb;
        self.ch1.power_off(preserve_length);
        self.ch2.power_off(preserve_length);
        self.ch3.power_off(preserve_length);
        self.ch4.power_off(preserve_length);
    }

    fn power_on(&mut self, cgb: bool) {
        self.powered = true;
        self.nr50 = 0;
        self.nr51 = 0;
        self.frame_step = 0;
        // On DMG the length timers (NRx1) survive a power cycle; on CGB they
        // are cleared. Everything else is reset.
        let preserve_length = !cgb;
        self.ch1.power_off(preserve_length);
        self.ch2.power_off(preserve_length);
        self.ch3.power_off(preserve_length);
        self.ch4.power_off(preserve_length);
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

    fn corrupt_wave_ram_on_dmg_wave_trigger(&mut self) {
        let Some(idx) = self.ch3.wave_trigger_corruption_byte() else {
            return;
        };
        if idx < 4 {
            self.wave_ram[0] = self.wave_ram[idx];
            return;
        }
        let base = idx & !0x03;
        let bytes = [
            self.wave_ram[base],
            self.wave_ram[base + 1],
            self.wave_ram[base + 2],
            self.wave_ram[base + 3],
        ];
        self.wave_ram[..4].copy_from_slice(&bytes);
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

    fn power_off(&mut self, preserve_length: bool) {
        self.pulse.power_off(preserve_length);
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

    /// Write only the length value (low 6 bits), leaving the duty bits
    /// untouched. Used for NRx1 writes while the APU is powered off on DMG,
    /// where the length-load register stays writable but duty does not.
    fn write_length_only(&mut self, value: u8) {
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

    /// Analog DAC output in [0.0, 1.0]. Returns 0.0 when the channel or its DAC
    /// is off. A square wave: the selected duty pattern bit gates the current
    /// envelope volume (0..15) to a normalized level.
    fn dac_output(&self) -> f32 {
        if !self.enabled || !self.envelope.dac_enabled() {
            return 0.0;
        }
        // Duty patterns (bits 7-6 of NRx1) -> 8-step high/low waveform.
        const DUTY: [[u8; 8]; 4] = [
            [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
            [1, 0, 0, 0, 0, 0, 0, 1], // 25%
            [1, 0, 0, 0, 0, 1, 1, 1], // 50%
            [0, 1, 1, 1, 1, 1, 1, 0], // 75%
        ];
        let pattern = (self.duty_length >> 6) & 0x03;
        let high = DUTY[pattern as usize][(self.duty_pos & 0x07) as usize] != 0;
        if high {
            f32::from(self.envelope.volume) / 15.0
        } else {
            0.0
        }
    }

    fn power_off(&mut self, preserve_length: bool) {
        let length = self.length_counter;
        *self = Self::default();
        if preserve_length {
            // Only the internal length-timer value survives; the readable
            // register bytes (duty) are zeroed like every other register.
            self.length_counter = length;
        }
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
    sample_buffer: u8,
    timer: u16,
    wave_access_window: Option<WaveAccessWindow>,
}

#[derive(Debug, Clone, Copy)]
struct WaveAccessWindow {
    byte_index: usize,
    remaining_t: u8,
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
        self.timer = self.period() + 6;
        self.enabled = self.dac_enabled();
    }

    fn tick_t(&mut self, wave_ram: &[u8; 0x10]) {
        self.clock_wave_access_window();
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
            self.read_sample(wave_ram);
        }
    }

    fn read_sample(&mut self, wave_ram: &[u8; 0x10]) {
        let byte_index = self.current_wave_byte_index();
        let byte = wave_ram[byte_index];
        self.sample_buffer = if self.sample_index & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        };
        self.wave_access_window = Some(WaveAccessWindow {
            byte_index,
            remaining_t: 2,
        });
    }

    fn clock_wave_access_window(&mut self) {
        let Some(window) = self.wave_access_window.as_mut() else {
            return;
        };
        if window.remaining_t > 1 {
            window.remaining_t -= 1;
        } else {
            self.wave_access_window = None;
        }
    }

    fn current_wave_byte_index(&self) -> usize {
        (self.sample_index >> 1) as usize
    }

    fn wave_access_byte(&self) -> Option<usize> {
        self.wave_access_window.map(|window| window.byte_index)
    }

    fn wave_trigger_corruption_byte(&self) -> Option<usize> {
        if !self.enabled || !(1..=2).contains(&self.timer) {
            return None;
        }
        Some(((self.sample_index.wrapping_add(1) & 0x1F) >> 1) as usize)
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

    /// Analog DAC output in [0.0, 1.0]. The current 4-bit wave-RAM nibble is
    /// scaled by the volume shift (NR32 bits 6-5: 0=mute, 1=100%, 2=50%, 3=25%).
    fn dac_output(&self) -> f32 {
        if !self.enabled || !self.dac_enabled() {
            return 0.0;
        }
        let shift = match (self.nr32 >> 5) & 0x03 {
            0 => return 0.0, // muted
            1 => 0,
            2 => 1,
            _ => 2,
        };
        f32::from(self.sample_buffer >> shift) / 15.0
    }

    fn power_off(&mut self, preserve_length: bool) {
        let length = self.length_counter;
        *self = Self::default();
        if preserve_length {
            self.length_counter = length;
        }
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

    /// Analog DAC output in [0.0, 1.0]. The inverted LFSR bit 0 gates the
    /// current envelope volume.
    fn dac_output(&self) -> f32 {
        if !self.enabled || !self.envelope.dac_enabled() {
            return 0.0;
        }
        if self.lfsr & 0x01 == 0 {
            f32::from(self.envelope.volume) / 15.0
        } else {
            0.0
        }
    }

    fn power_off(&mut self, preserve_length: bool) {
        let length = self.length_counter;
        *self = Self::default();
        if preserve_length {
            self.length_counter = length;
        }
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

    fn tick_spine_range(apu: &mut Apu, start: u16, end: u16, double_speed: bool) {
        for div in start..=end {
            apu.tick_spine(div, double_speed);
        }
    }

    #[test]
    fn div_apu_clocks_from_visible_div_bit4_not_raw_div_bit4() {
        let mut apu = Apu::default();
        apu.write(0xFF21, 0xF0, false);
        apu.write(0xFF20, 0x3F, false);
        apu.write(0xFF23, 0xC0, false);

        tick_spine_range(&mut apu, 1, 0x20, false);
        assert_eq!(
            apu.read(0xFF26) & 0x08,
            0x08,
            "raw DIV bit 4/5 must not clock frame sequencer"
        );

        tick_spine_range(&mut apu, 0x21, 0x2000, false);
        assert_eq!(
            apu.read(0xFF26) & 0x08,
            0x00,
            "visible DIV bit 4 falling edge must clock length at 512 Hz"
        );
    }

    #[test]
    fn cgb_double_speed_div_apu_uses_visible_div_bit5() {
        let mut apu = Apu::default();
        apu.write(0xFF21, 0xF0, true);
        apu.write(0xFF20, 0x3F, true);
        apu.write(0xFF23, 0xC0, true);

        tick_spine_range(&mut apu, 1, 0x2000, true);
        assert_eq!(apu.read(0xFF26) & 0x08, 0x08);

        tick_spine_range(&mut apu, 0x2001, 0x4000, true);
        assert_eq!(apu.read(0xFF26) & 0x08, 0x00);
    }

    #[test]
    fn envelope_period_clocks_on_frame_sequencer_steps_seven_then_five() {
        let mut apu = Apu::default();
        apu.write(0xFF17, 0x12, false);
        apu.write(0xFF19, 0x80, false);

        assert_eq!(apu.ch2.envelope.volume, 1);
        tick_div_steps(&mut apu, 6);
        assert_eq!(
            apu.ch2.envelope.volume, 1,
            "period-2 envelope must not change on first envelope step"
        );
        tick_div_steps(&mut apu, 2);
        assert_eq!(
            apu.ch2.envelope.volume, 0,
            "second envelope clock reloads period and applies a decreasing envelope"
        );
    }

    #[test]
    fn sweep_period_updates_frequency_on_every_second_sweep_clock() {
        let mut apu = Apu::default();
        apu.write(0xFF10, 0x21, false);
        apu.write(0xFF12, 0xF0, false);
        apu.write(0xFF13, 0x00, false);
        apu.write(0xFF14, 0x82, false);

        assert_eq!(apu.ch1.pulse.frequency(), 0x0200);
        tick_div_steps(&mut apu, 3);
        assert_eq!(
            apu.ch1.pulse.frequency(),
            0x0200,
            "first sweep clock only decrements period timer"
        );
        tick_div_steps(&mut apu, 4);
        assert_eq!(
            apu.ch1.pulse.frequency(),
            0x0300,
            "second sweep clock applies +frequency>>1"
        );
    }

    #[test]
    fn wave_channel_outputs_high_then_low_nibbles_from_wave_ram() {
        let mut apu = Apu::default();
        apu.write(0xFF30, 0xA5, false);
        apu.write(0xFF31, 0x3C, false);
        apu.write(0xFF1A, 0x80, false);
        apu.write(0xFF1C, 0x20, false);
        apu.write(0xFF1D, 0xFF, false);
        apu.write(0xFF1E, 0x87, false);

        for _ in 0..8 {
            apu.tick_t();
        }
        assert_eq!(
            apu.ch3.sample_buffer, 0x05,
            "first fetched sample after trigger is low nibble of byte 0"
        );
        for _ in 0..2 {
            apu.tick_t();
        }
        assert_eq!(
            apu.ch3.sample_buffer, 0x03,
            "next fetched sample is high nibble of byte 1"
        );
    }

    #[test]
    fn noise_lfsr_15_bit_sequence_matches_xor_feedback() {
        let mut apu = Apu::default();
        apu.ch4.lfsr = 0x7FFF;
        apu.ch4.nr43 = 0x00;

        apu.ch4.clock_lfsr();
        assert_eq!(apu.ch4.lfsr, 0x3FFF);
        apu.ch4.clock_lfsr();
        assert_eq!(apu.ch4.lfsr, 0x1FFF);

        apu.ch4.lfsr = 0x7FFF;
        apu.ch4.nr43 = 0x08;
        apu.ch4.clock_lfsr();
        assert_eq!(
            apu.ch4.lfsr, 0x3FBF,
            "7-bit mode mirrors feedback into bit 6"
        );
    }

    #[test]
    fn register_read_masks_match_dmg_bits() {
        let mut apu = Apu::default();

        apu.write(0xFF10, 0x7F, false);
        apu.write(0xFF11, 0x80, false);
        apu.write(0xFF12, 0xA5, false);
        apu.write(0xFF13, 0x12, false);
        apu.write(0xFF14, 0x47, false);
        apu.write(0xFF16, 0x40, false);
        apu.write(0xFF17, 0x5A, false);
        apu.write(0xFF18, 0x34, false);
        apu.write(0xFF19, 0x00, false);
        apu.write(0xFF1A, 0x80, false);
        apu.write(0xFF1B, 0x56, false);
        apu.write(0xFF1C, 0x20, false);
        apu.write(0xFF1D, 0x78, false);
        apu.write(0xFF1E, 0x40, false);
        apu.write(0xFF20, 0x3F, false);
        apu.write(0xFF21, 0xC3, false);
        apu.write(0xFF22, 0x2D, false);
        apu.write(0xFF23, 0x40, false);
        apu.write(0xFF24, 0x77, false);
        apu.write(0xFF25, 0xF3, false);
        apu.write(0xFF30, 0xAC, false);

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

        apu.write(0xFF12, 0xF3, false);
        apu.write(0xFF24, 0x77, false);
        apu.write(0xFF30, 0xA5, false);
        apu.write(0xFF26, 0x00, false);

        assert!(!apu.power());
        assert_eq!(apu.read(0xFF12), 0x00);
        assert_eq!(apu.read(0xFF24), 0x00);
        assert_eq!(apu.read(0xFF26), 0x70);
        assert_eq!(apu.read(0xFF30), 0xA5);

        apu.write(0xFF12, 0xF3, false);
        apu.write(0xFF30, 0x5A, false);

        assert_eq!(apu.read(0xFF12), 0x00);
        assert_eq!(apu.read(0xFF30), 0x5A);

        apu.write(0xFF26, 0x80, false);
        apu.write(0xFF12, 0xF3, false);

        assert!(apu.power());
        assert_eq!(apu.read(0xFF12), 0xF3);
    }

    #[test]
    fn length_counter_decrements_and_disables_channel() {
        let mut apu = Apu::default();

        apu.write(0xFF21, 0xF0, false);
        apu.write(0xFF20, 0x3F, false);
        apu.write(0xFF23, 0xC0, false);

        assert_eq!(apu.read(0xFF26) & 0x08, 0x08);
        assert_eq!(apu.ch4.length_counter, 1);

        apu.tick_div_apu();

        assert_eq!(apu.ch4.length_counter, 0);
        assert_eq!(apu.read(0xFF26) & 0x08, 0x00);
    }

    #[test]
    fn trigger_reloads_expired_length_and_sets_status_when_dac_is_on() {
        let mut apu = Apu::default();

        apu.write(0xFF17, 0xF0, false);
        apu.write(0xFF19, 0x80, false);

        assert_eq!(apu.ch2.length_counter, 64);
        assert_eq!(apu.read(0xFF26) & 0x02, 0x02);
    }

    #[test]
    fn extra_length_clock_fires_when_enabling_before_non_length_step() {
        let mut apu = Apu {
            frame_step: 1,
            ..Apu::default()
        };

        apu.write(0xFF17, 0xF0, false);
        apu.write(0xFF16, 0x3F, false);
        apu.write(0xFF19, 0x80, false);
        apu.write(0xFF19, 0x40, false);

        assert_eq!(apu.ch2.length_counter, 0);
        assert_eq!(apu.read(0xFF26) & 0x02, 0x00);
    }

    #[test]
    fn envelope_clocks_on_steps_five_and_seven() {
        let mut apu = Apu::default();

        apu.write(0xFF17, 0x19, false);
        apu.write(0xFF19, 0x80, false);

        assert_eq!(apu.ch2.envelope.volume, 1);

        tick_div_steps(&mut apu, 6);

        assert_eq!(apu.ch2.envelope.volume, 2);

        tick_div_steps(&mut apu, 2);

        assert_eq!(apu.ch2.envelope.volume, 3);
    }

    #[test]
    fn ch1_sweep_overflow_on_trigger_disables_channel_even_with_zero_pace() {
        let mut apu = Apu::default();

        apu.write(0xFF10, 0x01, false);
        apu.write(0xFF12, 0xF0, false);
        apu.write(0xFF13, 0xDC, false);
        apu.write(0xFF14, 0x85, false);

        assert_eq!(apu.read(0xFF26) & 0x01, 0x00);
        assert!(!apu.ch1.pulse.enabled);
    }

    #[test]
    fn nr52_reports_channel_status_bits() {
        let mut apu = Apu::default();

        apu.write(0xFF12, 0xF0, false);
        apu.write(0xFF14, 0x80, false);
        apu.write(0xFF17, 0xF0, false);
        apu.write(0xFF19, 0x80, false);
        apu.write(0xFF1A, 0x80, false);
        apu.write(0xFF1E, 0x80, false);
        apu.write(0xFF21, 0xF0, false);
        apu.write(0xFF23, 0x80, false);

        assert_eq!(apu.read(0xFF26), 0xFF);

        apu.write(0xFF17, 0x00, false);

        assert_eq!(apu.read(0xFF26) & 0x0F, 0x0D);
    }

    #[test]
    fn dmg_wave_ram_read_while_active_is_locked_out_between_fetches() {
        let mut apu = Apu::default();

        apu.write(0xFF30, 0x12, false);
        apu.write(0xFF1A, 0x80, false);
        apu.write(0xFF1D, 0xFF, false);
        apu.write(0xFF1E, 0x87, false);

        assert_eq!(apu.read(0xFF30), 0xFF);
    }

    #[test]
    fn dmg_wave_ram_access_while_active_hits_fetch_window_only() {
        let mut apu = Apu::default();

        apu.write(0xFF30, 0xAB, false);
        apu.write(0xFF31, 0xCD, false);
        apu.write(0xFF1A, 0x80, false);
        apu.write(0xFF1D, 0xFE, false);
        apu.write(0xFF1E, 0x87, false);

        apu.write(0xFF30, 0x11, false);
        assert_eq!(apu.wave_ram[0], 0xAB);

        for _ in 0..10 {
            apu.tick_t();
        }
        assert_eq!(apu.read(0xFF3F), 0xAB);

        apu.write(0xFF3F, 0x22, false);
        assert_eq!(apu.wave_ram[0], 0x22);

        apu.tick_t();
        assert_eq!(apu.read(0xFF30), 0x22);

        apu.tick_t();
        assert_eq!(apu.read(0xFF30), 0xFF);
    }

    #[test]
    fn cgb_wave_ram_access_while_active_uses_current_byte() {
        let mut apu = Apu::default();

        apu.write(0xFF30, 0x12, true);
        apu.write(0xFF31, 0x34, true);
        apu.write(0xFF1A, 0x80, true);
        apu.write(0xFF1D, 0xFE, true);
        apu.write(0xFF1E, 0x87, true);

        assert_eq!(apu.read_for_model(0xFF3F, true), 0x12);
        apu.write(0xFF3F, 0x56, true);
        assert_eq!(apu.wave_ram[0], 0x56);
    }

    #[test]
    fn dmg_wave_trigger_corrupts_first_byte_from_low_upcoming_fetch_group() {
        let mut apu = Apu::default();
        for i in 0..16 {
            apu.write(0xFF30 + i, i as u8, false);
        }
        apu.write(0xFF1A, 0x80, false);
        apu.write(0xFF1D, 0xFE, false);
        apu.write(0xFF1E, 0x87, false);

        for _ in 0..20 {
            apu.tick_t();
        }
        apu.write(0xFF1E, 0x87, false);

        assert_eq!(apu.wave_ram[0], 2);
        assert_eq!(apu.wave_ram[1], 1);
        assert_eq!(apu.wave_ram[2], 2);
        assert_eq!(apu.wave_ram[3], 3);
    }

    #[test]
    fn dmg_wave_trigger_corrupts_first_four_bytes_from_aligned_upcoming_fetch_group() {
        let mut apu = Apu::default();
        for i in 0..16 {
            apu.write(0xFF30 + i, (0xA0 | i) as u8, false);
        }
        apu.write(0xFF1A, 0x80, false);
        apu.write(0xFF1D, 0xFE, false);
        apu.write(0xFF1E, 0x87, false);

        for _ in 0..76 {
            apu.tick_t();
        }
        apu.write(0xFF1E, 0x87, false);

        assert_eq!(&apu.wave_ram[..4], &[0xA8, 0xA9, 0xAA, 0xAB]);
    }

    #[test]
    fn wave_trigger_keeps_previous_sample_buffer_until_next_fetch() {
        let mut apu = Apu::default();

        apu.write(0xFF30, 0xA5, false);
        apu.write(0xFF1A, 0x80, false);
        apu.write(0xFF1D, 0xFE, false);
        apu.ch3.sample_buffer = 0x0D;
        apu.write(0xFF1E, 0x87, false);

        assert_eq!(apu.ch3.sample_buffer, 0x0D);

        for _ in 0..10 {
            apu.tick_t();
        }

        assert_eq!(apu.ch3.sample_buffer, 0x05);
    }

    #[test]
    fn dac_produces_audible_oscillating_output_for_square_channel() {
        // Drive CH1 (square) with a real tone and prove the mixer emits a
        // non-silent, OSCILLATING stereo stream -- the floor for "can I hear it".
        let mut apu = Apu::default();
        apu.set_sample_rate(48_000);
        apu.write(0xFF26, 0x80, false); // APU on
        apu.write(0xFF25, 0xFF, false); // NR51: all channels to L+R
        apu.write(0xFF24, 0x77, false); // NR50: max master volume L+R
        apu.write(0xFF11, 0x80, false); // NR11: 50% duty
        apu.write(0xFF12, 0xF0, false); // NR12: volume 15, DAC on, no envelope
        apu.write(0xFF13, 0x00, false); // NR13: freq low
        apu.write(0xFF14, 0x87, false); // NR14: trigger + freq high (audible tone)

        // Run ~1/100s of emulated time (10_485 T-cycles) collecting samples.
        for _ in 0..10_485 {
            apu.tick_t();
        }
        let mut samples = Vec::new();
        apu.drain_samples(&mut samples);

        assert!(!samples.is_empty(), "no audio samples were collected");
        let max = samples.iter().cloned().fold(f32::MIN, f32::max);
        let min = samples.iter().cloned().fold(f32::MAX, f32::min);
        assert!(max > 0.0, "output is silent (max amplitude {max})");
        assert!(
            max - min > 0.01,
            "output does not oscillate (min {min}, max {max}); a square wave must swing"
        );
        // All samples must stay within the valid normalized range.
        assert!(
            samples.iter().all(|s| *s >= -1.0 && *s <= 1.0),
            "sample out of [-1.0, 1.0] range"
        );
    }

    #[test]
    fn no_samples_collected_when_rate_disabled() {
        // Headless/test default: sample_rate 0 -> no collection (zero overhead).
        let mut apu = Apu::default();
        apu.write(0xFF26, 0x80, false);
        apu.write(0xFF12, 0xF0, false);
        apu.write(0xFF14, 0x87, false);
        for _ in 0..10_000 {
            apu.tick_t();
        }
        assert_eq!(
            apu.buffered_frames(),
            0,
            "samples collected with rate disabled"
        );
    }

    #[test]
    fn sample_rate_calibrated_to_full_gameboy_clock() {
        // tick_t runs once per T-cycle = 4_194_304 Hz. At 48 kHz exactly one
        // stereo sample must be emitted every 4_194_304/48_000 ~= 87.38 ticks.
        // Run exactly 4_194_304 ticks (1 emulated second) and assert ~48_000
        // stereo frames (within 1%). Guards the divisor (regression: was 4x off).
        let mut apu = Apu::default();
        apu.set_sample_rate(48_000);
        apu.write(0xFF26, 0x80, false);
        for _ in 0..4_194_304u32 {
            apu.tick_t();
        }
        let frames = apu.buffered_frames();
        let lo = 48_000 - 480;
        let hi = 48_000 + 480;
        assert!(
            (lo..=hi).contains(&frames),
            "expected ~48000 stereo frames per emulated second, got {frames}"
        );
    }
}
