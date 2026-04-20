use super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn clock_irq_namco163(&mut self, cycles: u32) {
        let Some(namco163) = self.mappers.namco163.as_mut() else {
            return;
        };
        if !namco163.irq_enabled || namco163.irq_pending.get() {
            return;
        }
        let remaining = 0x7FFFu32.saturating_sub(namco163.irq_counter as u32);
        if cycles >= remaining {
            namco163.irq_counter = 0x7FFF;
            namco163.irq_pending.set(true);
        } else {
            namco163.irq_counter = ((namco163.irq_counter as u32 + cycles) & 0x7FFF) as u16;
        }
    }
}
