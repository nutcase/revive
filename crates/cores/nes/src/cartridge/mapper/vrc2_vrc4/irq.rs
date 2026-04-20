use super::super::super::Cartridge;
use super::Vrc2Vrc4;

impl Vrc2Vrc4 {
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
    pub(in crate::cartridge) fn clock_irq_mapper21(&mut self, cycles: u32) {
        self.clock_irq_mapper23(cycles);
    }

    pub(in crate::cartridge) fn clock_irq_mapper23(&mut self, cycles: u32) {
        if let Some(vrc) = self.mappers.vrc2_vrc4.as_mut() {
            vrc.clock_irq_mut(cycles);
        }
    }

    pub(in crate::cartridge) fn clock_irq_mapper25(&mut self, cycles: u32) {
        self.clock_irq_mapper23(cycles);
    }
}
