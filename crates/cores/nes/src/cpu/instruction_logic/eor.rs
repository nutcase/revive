use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn eor(&mut self, value: u8) {
        self.a ^= value;
        self.set_zero_negative_flags(self.a);
    }

    #[inline]
    pub(in crate::cpu) fn eor_indexed_indirect(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.get_indexed_indirect_addr(bus);
        let value = bus.read(addr);
        self.eor(value);
        6
    }

    #[inline]
    pub(in crate::cpu) fn eor_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.read_byte(bus) as u16;
        let value = bus.read(addr);
        self.eor(value);
        3
    }

    #[inline]
    pub(in crate::cpu) fn eor_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_byte(bus);
        self.eor(value);
        2
    }

    #[inline]
    pub(in crate::cpu) fn eor_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.read_word(bus);
        let value = bus.read(addr);
        self.eor(value);
        4
    }

    #[inline]
    pub(in crate::cpu) fn eor_indirect_indexed(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (addr, page_crossed) = self.get_indirect_indexed_addr(bus);
        let value = bus.read(addr);
        self.eor(value);
        if page_crossed {
            6
        } else {
            5
        }
    }

    #[inline]
    pub(in crate::cpu) fn eor_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.get_zero_page_x_addr(bus);
        let value = bus.read(addr);
        self.eor(value);
        4
    }

    #[inline]
    pub(in crate::cpu) fn eor_absolute_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (addr, page_crossed) = self.get_absolute_y_addr(bus);
        let value = bus.read(addr);
        self.eor(value);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(in crate::cpu) fn eor_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (addr, page_crossed) = self.get_absolute_x_addr(bus);
        let value = bus.read(addr);
        self.eor(value);
        if page_crossed {
            5
        } else {
            4
        }
    }
}
