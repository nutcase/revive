use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn bpl(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.branch(bus, !self.status.contains(StatusFlags::NEGATIVE))
    }

    #[inline]
    pub(in crate::cpu) fn bmi(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.branch(bus, self.status.contains(StatusFlags::NEGATIVE))
    }

    #[inline]
    pub(in crate::cpu) fn bvc(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.branch(bus, !self.status.contains(StatusFlags::OVERFLOW))
    }

    #[inline]
    pub(in crate::cpu) fn bvs(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.branch(bus, self.status.contains(StatusFlags::OVERFLOW))
    }

    #[inline]
    pub(in crate::cpu) fn bcc(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.branch(bus, !self.status.contains(StatusFlags::CARRY))
    }

    #[inline]
    pub(in crate::cpu) fn bcs(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.branch(bus, self.status.contains(StatusFlags::CARRY))
    }

    #[inline]
    pub(in crate::cpu) fn bne(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.branch(bus, !self.status.contains(StatusFlags::ZERO))
    }

    #[inline]
    pub(in crate::cpu) fn beq(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.branch(bus, self.status.contains(StatusFlags::ZERO))
    }
}
