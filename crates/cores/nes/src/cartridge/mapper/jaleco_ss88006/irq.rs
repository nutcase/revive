use crate::cartridge::Cartridge;

use super::JalecoSs88006;

impl JalecoSs88006 {
    fn irq_mask(&self) -> u16 {
        if self.irq_control & 0x08 != 0 {
            0x000F
        } else if self.irq_control & 0x04 != 0 {
            0x00FF
        } else if self.irq_control & 0x02 != 0 {
            0x0FFF
        } else {
            0xFFFF
        }
    }

    pub(in crate::cartridge) fn clock_irq_mut(&mut self, cycles: u32) {
        if self.irq_control & 0x01 == 0 {
            return;
        }

        let mask = self.irq_mask();
        let preserved = !mask;
        for _ in 0..cycles {
            let counter_low = self.irq_counter & mask;
            let counter_high = self.irq_counter & preserved;
            if counter_low == 0 {
                self.irq_counter = counter_high | mask;
                self.irq_pending.set(true);
            } else {
                self.irq_counter = counter_high | ((counter_low - 1) & mask);
            }
        }
    }
}

impl Cartridge {
    pub(super) fn jaleco_ss88006_write_irq_reload_nibble(&mut self, nibble: usize, data: u8) {
        let Some(mapper18) = self.mappers.jaleco_ss88006.as_mut() else {
            return;
        };
        let shift = (nibble as u16) * 4;
        let mask = !(0x000Fu16 << shift);
        mapper18.irq_reload = (mapper18.irq_reload & mask) | (((data & 0x0F) as u16) << shift);
    }

    pub(in crate::cartridge) fn clock_irq_mapper18(&mut self, cycles: u32) {
        if let Some(mapper18) = self.mappers.jaleco_ss88006.as_mut() {
            mapper18.clock_irq_mut(cycles);
        }
    }
}
