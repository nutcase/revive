use super::super::super::Cartridge;
use super::Vrc6;

impl Vrc6 {
    fn clock_irq_counter(&mut self) {
        if self.irq_counter == 0xFF {
            self.irq_counter = self.irq_latch;
            self.irq_pending.set(true);
        } else {
            self.irq_counter = self.irq_counter.wrapping_add(1);
        }
    }

    pub(in crate::cartridge) fn clock_irq_mut(&mut self, cycles: u32) {
        if !self.irq_enabled {
            return;
        }

        for _ in 0..cycles {
            if self.irq_cycle_mode {
                self.clock_irq_counter();
            } else {
                self.irq_prescaler -= 3;
                if self.irq_prescaler <= 0 {
                    self.irq_prescaler += 341;
                    self.clock_irq_counter();
                }
            }
        }
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn clock_irq_vrc6(&mut self, cycles: u32) {
        if let Some(vrc6) = self.mappers.vrc6.as_mut() {
            vrc6.clock_irq_mut(cycles);
        }
    }
}
