use super::super::{TriangleChannel, LENGTH_TABLE};

impl TriangleChannel {
    pub(in crate::apu) fn new() -> Self {
        TriangleChannel {
            linear_counter: 0,
            linear_reload: 0,
            linear_control: false,
            linear_reload_flag: false,
            length_counter: 0,
            timer: 0,
            timer_reload: 0,
            sequence_counter: 0,
            length_enabled: true,
        }
    }

    pub(in crate::apu) fn write_control(&mut self, data: u8) {
        self.linear_control = (data & 0x80) != 0;
        self.linear_reload = data & 0x7F;
        self.length_enabled = !self.linear_control;
    }

    pub(in crate::apu) fn write_timer_low(&mut self, data: u8) {
        self.timer_reload = (self.timer_reload & 0xFF00) | data as u16;
    }

    pub(in crate::apu) fn write_timer_high(&mut self, data: u8, enabled: bool) {
        self.timer_reload = (self.timer_reload & 0x00FF) | ((data as u16 & 0x07) << 8);
        if enabled {
            self.length_counter = LENGTH_TABLE[((data >> 3) & 0x1F) as usize];
        }
        self.linear_reload_flag = true;
    }

    pub(in crate::apu) fn step(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_reload;
            if self.linear_counter > 0 && self.length_counter > 0 && self.timer_reload >= 2 {
                self.sequence_counter = (self.sequence_counter + 1) % 32;
            }
        } else {
            self.timer -= 1;
        }
    }

    pub(in crate::apu) fn clock_linear_counter(&mut self) {
        if self.linear_reload_flag {
            self.linear_counter = self.linear_reload;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }

        if !self.linear_control {
            self.linear_reload_flag = false;
        }
    }

    pub(in crate::apu) fn clock_length_counter(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub(in crate::apu) fn output(&self) -> f32 {
        if self.length_counter == 0 || self.linear_counter == 0 || self.timer_reload < 2 {
            return 0.0;
        }

        const TRIANGLE_SEQUENCE: [u8; 32] = [
            15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
            11, 12, 13, 14, 15,
        ];

        TRIANGLE_SEQUENCE[self.sequence_counter as usize] as f32
    }
}
