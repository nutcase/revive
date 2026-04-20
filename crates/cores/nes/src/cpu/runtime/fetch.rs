use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn read_byte(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let byte = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        byte
    }

    #[inline]
    pub(in crate::cpu) fn read_word(&mut self, bus: &mut dyn CpuBus) -> u16 {
        let low = self.read_byte(bus) as u16;
        let high = self.read_byte(bus) as u16;
        (high << 8) | low
    }
}
