use super::*;

impl Default for Psg {
    fn default() -> Self {
        Self {
            last_data: 0,
            writes: 0,
            latched_channel: 0,
            latched_is_volume: false,
            tone_period: [1, 1, 1],
            tone_output: [true, true, true],
            tone_counter: [1, 1, 1],
            attenuation: [0x0F; 4],
            noise_control: 0,
            noise_lfsr: 0x4000,
            noise_counter: 0x10,
            sample_counter: 0,
        }
    }
}

impl Psg {
    // SN76489 internal clock = master / 16
    const PSG_CLOCK_HZ: u32 = 3_579_545 / 16; // 223,721 Hz

    pub(super) fn write_data(&mut self, value: u8) {
        self.last_data = value;
        self.writes += 1;
        if (value & 0x80) != 0 {
            self.latched_channel = ((value >> 5) & 0x3) as usize;
            self.latched_is_volume = (value & 0x10) != 0;
            let data = value & 0x0F;
            self.apply_latched_data(data);
            return;
        }

        if self.latched_is_volume {
            self.attenuation[self.latched_channel] = value & 0x0F;
        } else if self.latched_channel < 3 {
            let lo = self.tone_period[self.latched_channel] & 0x000F;
            let hi = ((value & 0x3F) as u16) << 4;
            self.tone_period[self.latched_channel] = lo | hi;
        }
    }

    pub fn last_data(&self) -> u8 {
        self.last_data
    }

    pub fn writes(&self) -> u64 {
        self.writes
    }

    pub fn tone_period(&self, channel: usize) -> u16 {
        self.tone_period[channel.min(2)]
    }

    pub fn attenuation(&self, channel: usize) -> u8 {
        self.attenuation[channel.min(3)]
    }

    pub fn noise_control(&self) -> u8 {
        self.noise_control
    }

    pub fn tone_frequency_hz_debug(&self, channel: usize) -> f32 {
        let raw_period = self.tone_period[channel.min(2)] & 0x03FF;
        let period = raw_period.max(1) as f32;
        3_579_545.0 / (32.0 * period)
    }

    fn apply_latched_data(&mut self, data: u8) {
        if self.latched_is_volume {
            self.attenuation[self.latched_channel] = data & 0x0F;
            return;
        }

        if self.latched_channel < 3 {
            let hi = self.tone_period[self.latched_channel] & 0x03F0;
            self.tone_period[self.latched_channel] = hi | data as u16;
        } else {
            self.noise_control = data & 0x07;
            self.noise_lfsr = 0x4000;
            self.noise_counter = Self::noise_period(data & 0x07, self.tone_period[2]);
        }
    }

    fn noise_period(noise_control: u8, tone3_period: u16) -> u16 {
        match noise_control & 0x03 {
            0x00 => 0x10,             // clock/512 → period 16
            0x01 => 0x20,             // clock/1024 → period 32
            0x02 => 0x40,             // clock/2048 → period 64
            _ => tone3_period.max(1), // use tone channel 3 period
        }
    }

    fn clock_noise_lfsr(&mut self) {
        let bit0 = self.noise_lfsr & 1;
        let feedback = if (self.noise_control & 0x04) != 0 {
            let bit3 = (self.noise_lfsr >> 3) & 1;
            bit0 ^ bit3
        } else {
            bit0
        };
        self.noise_lfsr = ((self.noise_lfsr >> 1) | (feedback << 14)) & 0x7FFF;
    }

    /// Advance PSG by one internal clock tick
    fn clock_tick(&mut self) {
        let noise_uses_tone3 = (self.noise_control & 0x03) == 0x03;

        // Advance tone counters
        for ch in 0..3 {
            if self.tone_counter[ch] > 0 {
                self.tone_counter[ch] -= 1;
            }
            if self.tone_counter[ch] == 0 {
                let period = (self.tone_period[ch] & 0x3FF).max(1);
                self.tone_counter[ch] = period;
                let was_high = self.tone_output[ch];
                self.tone_output[ch] = !self.tone_output[ch];
                // Noise channel clocked by tone3 falling edge
                if noise_uses_tone3 && ch == 2 && was_high && !self.tone_output[ch] {
                    self.clock_noise_lfsr();
                }
            }
        }

        // Advance noise counter (independent clock unless using tone3)
        if !noise_uses_tone3 {
            if self.noise_counter > 0 {
                self.noise_counter -= 1;
            }
            if self.noise_counter == 0 {
                self.noise_counter = Self::noise_period(self.noise_control, self.tone_period[2]);
                self.clock_noise_lfsr();
            }
        }
    }

    pub(super) fn next_sample(&mut self, sample_rate_hz: u32) -> i16 {
        // Bresenham resampler: PSG_CLOCK_HZ → sample_rate_hz
        self.sample_counter += Self::PSG_CLOCK_HZ;
        while self.sample_counter >= sample_rate_hz {
            self.sample_counter -= sample_rate_hz;
            self.clock_tick();
        }

        // Mix using pre-computed integer volume table
        let mut mix = 0i32;
        for ch in 0..3 {
            let vol = PSG_VOLUME[self.attenuation[ch].min(15) as usize] as i32;
            mix += if self.tone_output[ch] { vol } else { -vol };
        }
        let noise_vol = PSG_VOLUME[self.attenuation[3].min(15) as usize] as i32;
        mix += if (self.noise_lfsr & 1) != 0 {
            noise_vol
        } else {
            -noise_vol
        };

        // Scale to match previous float output level (~1800.0 * amplitude)
        // PSG_VOLUME[0]=8000, old was 1.0*1800=1800, so scale by 1800/8000 ≈ 9/40
        ((mix * 9) / 40).clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }
}
