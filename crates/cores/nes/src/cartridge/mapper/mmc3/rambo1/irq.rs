use super::super::Mmc3;

impl Mmc3 {
    pub(in crate::cartridge) fn clock_irq_rambo1_mut(&mut self) {
        if self.irq_reload {
            self.irq_counter = if self.irq_latch == 0 {
                0
            } else {
                self.irq_latch | 0x01
            };
            self.irq_reload = false;
        } else if self.irq_counter == 0 {
            self.irq_counter = self.irq_latch;
        } else {
            self.irq_counter = self.irq_counter.wrapping_sub(1);
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_delay = 4;
        }
    }

    pub(in crate::cartridge) fn clock_irq_rambo1_cycle(&mut self) {
        if self.irq_delay > 0 {
            self.irq_delay -= 1;
            if self.irq_delay == 0 {
                self.irq_pending.set(true);
            }
        }

        if self.irq_cycle_mode {
            if self.irq_prescaler > 1 {
                self.irq_prescaler -= 1;
            } else {
                self.irq_prescaler = 4;
                self.clock_irq_rambo1_mut();
            }
        }
    }
}
