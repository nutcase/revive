use super::*;

impl Apu {
    pub fn frame_irq_pending(&self) -> bool {
        self.frame_irq && !self.irq_disable
    }

    pub fn irq_pending(&self) -> bool {
        self.frame_irq_pending() || self.dmc.irq_pending
    }

    pub fn read_register(&mut self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                let mut status = 0;
                if self.pulse1_enabled && self.pulse1.length_counter > 0 {
                    status |= 0x01;
                }
                if self.pulse2_enabled && self.pulse2.length_counter > 0 {
                    status |= 0x02;
                }
                if self.triangle_enabled && self.triangle.length_counter > 0 {
                    status |= 0x04;
                }
                if self.noise_enabled && self.noise.length_counter > 0 {
                    status |= 0x08;
                }
                if self.dmc.bytes_remaining > 0 {
                    status |= 0x10;
                }

                if self.frame_irq {
                    status |= 0x40;
                }

                if self.dmc.irq_pending {
                    status |= 0x80;
                }

                // Reading $4015 clears the frame IRQ flag
                self.frame_irq = false;

                status
            }
            _ => 0,
        }
    }

    pub fn write_register(&mut self, addr: u16, data: u8) {
        match addr {
            // Pulse 1
            0x4000 => self.pulse1.write_control(data),
            0x4001 => self.pulse1.write_sweep(data),
            0x4002 => self.pulse1.write_timer_low(data),
            0x4003 => self.pulse1.write_timer_high(data, self.pulse1_enabled),

            // Pulse 2
            0x4004 => self.pulse2.write_control(data),
            0x4005 => self.pulse2.write_sweep(data),
            0x4006 => self.pulse2.write_timer_low(data),
            0x4007 => self.pulse2.write_timer_high(data, self.pulse2_enabled),

            // Triangle
            0x4008 => self.triangle.write_control(data),
            0x4009 => {}
            0x400A => self.triangle.write_timer_low(data),
            0x400B => self.triangle.write_timer_high(data, self.triangle_enabled),

            // Noise
            0x400C => self.noise.write_control(data),
            0x400D => {}
            0x400E => self.noise.write_period(data),
            0x400F => self.noise.write_length(data, self.noise_enabled),

            // DMC
            0x4010 => self.dmc.write_control(data),
            0x4011 => self.dmc.write_direct_load(data),
            0x4012 => self.dmc.write_sample_address(data),
            0x4013 => self.dmc.write_sample_length(data),

            // Status
            0x4015 => {
                self.pulse1_enabled = data & 0x01 != 0;
                self.pulse2_enabled = data & 0x02 != 0;
                self.triangle_enabled = data & 0x04 != 0;
                self.noise_enabled = data & 0x08 != 0;
                self.dmc_enabled = data & 0x10 != 0;
                self.dmc.irq_pending = false;

                if !self.pulse1_enabled {
                    self.pulse1.length_counter = 0;
                }
                if !self.pulse2_enabled {
                    self.pulse2.length_counter = 0;
                }
                if !self.triangle_enabled {
                    self.triangle.length_counter = 0;
                }
                if !self.noise_enabled {
                    self.noise.length_counter = 0;
                }

                self.dmc.set_enabled(self.dmc_enabled);
            }

            // Frame counter
            0x4017 => {
                self.frame_mode = (data & 0x80) != 0;
                self.irq_disable = (data & 0x40) != 0;

                self.frame_irq = false;
                self.frame_counter = 0;

                // 5-step mode immediately clocks quarter + half frame
                if self.frame_mode {
                    self.clock_half_frame();
                }
            }
            _ => {}
        }
    }

    pub(crate) fn pull_dmc_sample_request(&mut self) -> Option<(u16, u8)> {
        self.dmc.pull_sample_request()
    }

    pub(crate) fn push_dmc_sample(&mut self, data: u8) {
        self.dmc.push_sample(data);
    }
}
