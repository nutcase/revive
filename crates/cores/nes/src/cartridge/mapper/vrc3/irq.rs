use super::Vrc3;

impl Vrc3 {
    pub(in crate::cartridge) fn write_latch_nibble(&mut self, nibble: usize, data: u8) {
        let shift = (nibble as u16) * 4;
        let mask = !(0x000Fu16 << shift);
        self.irq_reload = (self.irq_reload & mask) | (((data & 0x0F) as u16) << shift);
    }

    pub(in crate::cartridge) fn write_control(&mut self, data: u8) {
        self.irq_mode_8bit = data & 0x04 != 0;
        self.irq_enable_on_ack = data & 0x01 != 0;
        self.irq_enabled = data & 0x02 != 0;
        self.irq_pending.set(false);
        if self.irq_enabled {
            self.irq_counter = self.irq_reload;
        }
    }

    pub(in crate::cartridge) fn acknowledge(&mut self) {
        self.irq_pending.set(false);
        self.irq_enabled = self.irq_enable_on_ack;
    }

    pub(in crate::cartridge) fn clock_irq_mut(&mut self, cycles: u32) {
        if !self.irq_enabled {
            return;
        }

        for _ in 0..cycles {
            if self.irq_mode_8bit {
                let low = (self.irq_counter & 0x00FF) as u8;
                if low == 0xFF {
                    self.irq_counter = (self.irq_counter & 0xFF00) | (self.irq_reload & 0x00FF);
                    self.irq_pending.set(true);
                } else {
                    self.irq_counter = (self.irq_counter & 0xFF00) | (low.wrapping_add(1) as u16);
                }
            } else if self.irq_counter == 0xFFFF {
                self.irq_counter = self.irq_reload;
                self.irq_pending.set(true);
            } else {
                self.irq_counter = self.irq_counter.wrapping_add(1);
            }
        }
    }
}
