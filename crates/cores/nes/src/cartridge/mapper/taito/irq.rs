use super::TaitoTc0190;

impl TaitoTc0190 {
    pub(in crate::cartridge) fn clock_irq_mut(&mut self) {
        let counter_was_zero = self.irq_counter == 0;
        if counter_was_zero || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter = self.irq_counter.wrapping_sub(1);
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_delay = 4;
        }
    }

    pub(in crate::cartridge) fn clock_irq_delay_mut(&mut self, cycles: u32) {
        for _ in 0..cycles {
            if self.irq_delay == 0 {
                break;
            }
            self.irq_delay -= 1;
            if self.irq_delay == 0 && self.irq_enabled {
                self.irq_pending.set(true);
            }
        }
    }
}
