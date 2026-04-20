use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn push(&mut self, bus: &mut dyn CpuBus, value: u8) {
        // Push to stack
        let addr = 0x0100 | self.sp as u16;
        bus.write(addr, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    #[inline]
    pub(in crate::cpu) fn pull(&mut self, bus: &mut dyn CpuBus) -> u8 {
        // Pull from stack
        self.sp = self.sp.wrapping_add(1);
        let addr = 0x0100 | self.sp as u16;
        bus.read(addr)
    }
}
