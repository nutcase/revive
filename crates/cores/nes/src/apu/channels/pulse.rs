use super::super::{PulseChannel, LENGTH_TABLE};

impl PulseChannel {
    pub(in crate::apu) fn new(is_pulse1: bool) -> Self {
        PulseChannel {
            duty: 0,
            length_counter: 0,
            envelope_divider: 0,
            envelope_decay: 15,
            envelope_disable: false,
            envelope_start: false,
            volume: 0,
            sweep_enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_reload: false,
            sweep_divider: 0,
            timer: 0,
            timer_reload: 0,
            duty_counter: 0,
            length_enabled: true,
            is_pulse1,
        }
    }

    pub(in crate::apu) fn write_control(&mut self, data: u8) {
        self.duty = (data >> 6) & 0x03;
        self.length_enabled = (data & 0x20) == 0;
        self.envelope_disable = (data & 0x10) != 0;
        self.volume = data & 0x0F;
        // Note: writing to $4000/$4004 does NOT restart the envelope.
        // Only writing to $4003/$4007 (4th register) sets envelope_start.
    }

    pub(in crate::apu) fn write_sweep(&mut self, data: u8) {
        self.sweep_enabled = (data & 0x80) != 0;
        self.sweep_period = (data >> 4) & 0x07;
        self.sweep_negate = (data & 0x08) != 0;
        self.sweep_shift = data & 0x07;
        self.sweep_reload = true;
    }

    pub(in crate::apu) fn write_timer_low(&mut self, data: u8) {
        self.timer_reload = (self.timer_reload & 0xFF00) | data as u16;
    }

    pub(in crate::apu) fn write_timer_high(&mut self, data: u8, enabled: bool) {
        self.timer_reload = (self.timer_reload & 0x00FF) | ((data as u16 & 0x07) << 8);
        if enabled {
            self.length_counter = LENGTH_TABLE[((data >> 3) & 0x1F) as usize];
        }
        self.timer = self.timer_reload;
        self.duty_counter = 0;
        self.envelope_start = true;
    }

    pub(in crate::apu) fn step(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_reload;
            self.duty_counter = (self.duty_counter + 1) % 8;
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
                // Loop mode (length counter halt = loop envelope)
                self.envelope_decay = 15;
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    /// Compute the sweep target period. Used for both period updates and muting.
    pub(in crate::apu) fn sweep_target_period(&self) -> i32 {
        let current = self.timer_reload as i32;
        let change = current >> self.sweep_shift;
        if self.sweep_negate {
            if self.is_pulse1 {
                current - change - 1 // Pulse 1: one's complement (extra -1)
            } else {
                current - change // Pulse 2: two's complement
            }
        } else {
            current + change
        }
    }

    /// Returns true if the channel should be muted due to sweep conditions.
    /// Muting is evaluated continuously regardless of sweep_enabled.
    pub(in crate::apu) fn is_sweep_muting(&self) -> bool {
        self.timer_reload < 8 || self.sweep_target_period() > 0x7FF
    }

    pub(in crate::apu) fn clock_sweep(&mut self) {
        // When reload flag is set: if divider was also 0, fire period update first
        if self.sweep_reload {
            let old_divider = self.sweep_divider;
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
            if old_divider == 0
                && self.sweep_enabled
                && self.sweep_shift > 0
                && !self.is_sweep_muting()
            {
                let target = self.sweep_target_period();
                if target >= 0 {
                    self.timer_reload = target as u16;
                }
            }
        } else if self.sweep_divider == 0 {
            self.sweep_divider = self.sweep_period;
            if self.sweep_enabled && self.sweep_shift > 0 && !self.is_sweep_muting() {
                let target = self.sweep_target_period();
                if target >= 0 {
                    self.timer_reload = target as u16;
                }
            }
        } else {
            self.sweep_divider -= 1;
        }
    }

    pub(in crate::apu) fn clock_length_counter(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub(in crate::apu) fn output(&self) -> f32 {
        // Sweep muting: timer_reload < 8 OR target period > $7FF (continuous check)
        if self.length_counter == 0 || self.is_sweep_muting() {
            return 0.0;
        }

        const DUTY_TABLE: [[u8; 8]; 4] = [
            [0, 1, 0, 0, 0, 0, 0, 0], // 12.5%
            [0, 1, 1, 0, 0, 0, 0, 0], // 25%
            [0, 1, 1, 1, 1, 0, 0, 0], // 50%
            [1, 0, 0, 1, 1, 1, 1, 1], // 75%
        ];

        if DUTY_TABLE[self.duty as usize][self.duty_counter as usize] == 0 {
            return 0.0;
        }

        if self.envelope_disable {
            self.volume as f32
        } else {
            self.envelope_decay as f32
        }
    }
}
