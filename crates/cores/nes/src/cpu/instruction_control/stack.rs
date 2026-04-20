use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn php(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.push(
            bus,
            self.status.bits() | StatusFlags::BREAK.bits() | StatusFlags::UNUSED.bits(),
        );
        3
    }

    #[inline]
    pub(in crate::cpu) fn plp(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.pull(bus);
        self.status = StatusFlags::from_bits_truncate(value & !StatusFlags::BREAK.bits())
            | StatusFlags::UNUSED;
        4
    }

    #[inline]
    pub(in crate::cpu) fn pha(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.push(bus, self.a);
        3
    }

    #[inline]
    pub(in crate::cpu) fn pla(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.a = self.pull(bus);
        self.set_zero_negative_flags(self.a);
        4
    }
}
