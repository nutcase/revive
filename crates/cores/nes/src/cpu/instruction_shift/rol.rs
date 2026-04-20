use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn rol(&mut self, value: u8) -> u8 {
        let carry = if self.status.contains(StatusFlags::CARRY) {
            1
        } else {
            0
        };
        self.status.set(StatusFlags::CARRY, value & 0x80 != 0);
        let result = (value << 1) | carry;
        self.set_zero_negative_flags(result);
        result
    }

    #[inline]
    pub(in crate::cpu) fn rol_accumulator(&mut self) -> u8 {
        self.a = self.rol(self.a);
        2
    }

    #[inline]
    pub(in crate::cpu) fn rol_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.read_byte(bus) as u16;
        let value = bus.read(addr);
        let result = self.rol(value);
        bus.write(addr, result);
        5
    }

    #[inline]
    pub(in crate::cpu) fn rol_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.read_word(bus);
        let value = bus.read(addr);
        let result = self.rol(value);
        bus.write(addr, result);
        6
    }

    #[inline]
    pub(in crate::cpu) fn rol_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.get_zero_page_x_addr(bus);
        let value = bus.read(addr);
        let result = self.rol(value);
        bus.write(addr, result);
        6
    }

    #[inline]
    pub(in crate::cpu) fn rol_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (addr, _) = self.get_absolute_x_addr(bus);
        let value = bus.read(addr);
        let result = self.rol(value);
        bus.write(addr, result);
        7
    }
}
