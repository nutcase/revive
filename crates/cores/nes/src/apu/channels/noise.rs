use super::super::{NoiseChannel, LENGTH_TABLE, NOISE_PERIOD_TABLE};

impl NoiseChannel {
    pub(in crate::apu) fn new() -> Self {
        NoiseChannel {
            length_counter: 0,
            envelope_divider: 0,
            envelope_decay: 0,
            envelope_disable: false,
            envelope_start: false,
            volume: 0,
            mode: false,
            timer: 0,
            timer_reload: 0,
            shift_register: 1,
            length_enabled: true,
        }
    }

    pub(in crate::apu) fn write_control(&mut self, data: u8) {
        self.length_enabled = (data & 0x20) == 0;
        self.envelope_disable = (data & 0x10) != 0;
        self.volume = data & 0x0F;
        // Note: writing to $400C does NOT restart the envelope.
        // Only writing to $400F (4th register) sets envelope_start.
    }

    pub(in crate::apu) fn write_period(&mut self, data: u8) {
        self.mode = (data & 0x80) != 0;
        self.timer_reload = NOISE_PERIOD_TABLE[(data & 0x0F) as usize];
    }

    pub(in crate::apu) fn write_length(&mut self, data: u8, enabled: bool) {
        if enabled {
            self.length_counter = LENGTH_TABLE[((data >> 3) & 0x1F) as usize];
        }
        self.envelope_start = true;
    }

    pub(in crate::apu) fn step(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_reload;

            let feedback_bit = if self.mode {
                (self.shift_register ^ (self.shift_register >> 6)) & 1
            } else {
                (self.shift_register ^ (self.shift_register >> 1)) & 1
            };

            self.shift_register >>= 1;
            self.shift_register |= feedback_bit << 14;
        } else {
            self.timer -= 1;
        }
    }

    pub(in crate::apu) fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.volume;
        } else if self.envelope_divider == 0 {
            self.envelope_divider = self.volume;
            if self.envelope_decay > 0 {
                self.envelope_decay -= 1;
            } else if !self.length_enabled {
                self.envelope_decay = 15;
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    pub(in crate::apu) fn clock_length_counter(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub(in crate::apu) fn output(&self) -> f32 {
        if self.length_counter == 0 {
            return 0.0;
        }

        // Noise output: bit 0 of shift register inverted
        if (self.shift_register & 1) != 0 {
            return 0.0;
        }

        if self.envelope_disable {
            self.volume as f32
        } else {
            self.envelope_decay as f32
        }
    }
}
