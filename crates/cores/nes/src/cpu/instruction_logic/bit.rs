use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn bit(&mut self, value: u8) {
        self.status.set(StatusFlags::ZERO, (self.a & value) == 0);
        self.status.set(StatusFlags::OVERFLOW, value & 0x40 != 0);
        self.status.set(StatusFlags::NEGATIVE, value & 0x80 != 0);
    }

    #[inline]
    pub(in crate::cpu) fn bit_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.read_byte(bus) as u16;
        let value = bus.read(addr);
        self.bit(value);
        3
    }

    #[inline]
    pub(in crate::cpu) fn bit_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.read_word(bus);
        let value = bus.read(addr);
        self.bit(value);
        4
    }
}
