use super::Fme7;

impl Fme7 {
    pub(in crate::cartridge) fn clock_irq_mut(&mut self) {
        if self.irq_counter_enabled {
            let old = self.irq_counter;
            self.irq_counter = old.wrapping_sub(1);
            if old == 0 && self.irq_enabled {
                self.irq_pending.set(true);
            }
        }
    }
}
