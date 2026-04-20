use super::BandaiFcg;

impl BandaiFcg {
    pub(in crate::cartridge) fn clock_irq_mut(&mut self) {
        if self.irq_enabled {
            if self.irq_counter == 0 {
                self.irq_pending.set(true);
                self.irq_enabled = false;
            } else {
                self.irq_counter -= 1;
            }
        }
    }
}
