use super::Mmc3;

impl Mmc3 {
    pub(in crate::cartridge) fn clock_irq_mut(&mut self) {
        let counter_was_zero = self.irq_counter == 0;
        if counter_was_zero || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending.set(true);
        }
    }
}
