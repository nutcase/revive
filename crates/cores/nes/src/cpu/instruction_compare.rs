use super::*;

impl Cpu {
    #[inline]
    pub(super) fn compare(&mut self, reg: u8, value: u8) {
        let result = reg.wrapping_sub(value);
        self.status.set(StatusFlags::CARRY, reg >= value);
        self.status.set(StatusFlags::ZERO, reg == value);
        self.status.set(StatusFlags::NEGATIVE, result & 0x80 != 0);
    }

    #[inline]
    pub(super) fn cpy_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_immediate_operand(bus);
        self.compare(self.y, value);
        2
    }

    #[inline]
    pub(super) fn cmp_indexed_indirect(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_indexed_indirect_operand(bus);
        self.compare(self.a, value);
        6
    }

    #[inline]
    pub(super) fn cpy_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_zero_page_operand(bus);
        self.compare(self.y, value);
        3
    }

    #[inline]
    pub(super) fn cmp_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_zero_page_operand(bus);
        self.compare(self.a, value);
        3
    }

    #[inline]
    pub(super) fn cmp_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_immediate_operand(bus);
        self.compare(self.a, value);
        2
    }

    #[inline]
    pub(super) fn cpy_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_absolute_operand(bus);
        self.compare(self.y, value);
        4
    }

    #[inline]
    pub(super) fn cmp_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_absolute_operand(bus);
        self.compare(self.a, value);
        4
    }

    #[inline]
    pub(super) fn cmp_indirect_indexed(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_indirect_indexed_operand(bus);
        self.compare(self.a, value);
        if page_crossed {
            6
        } else {
            5
        }
    }

    #[inline]
    pub(super) fn cmp_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_zero_page_x_operand(bus);
        self.compare(self.a, value);
        4
    }

    #[inline]
    pub(super) fn cmp_absolute_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_y_operand(bus);
        self.compare(self.a, value);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(super) fn cmp_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_x_operand(bus);
        self.compare(self.a, value);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(super) fn cpx_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_immediate_operand(bus);
        self.compare(self.x, value);
        2
    }

    #[inline]
    pub(super) fn cpx_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_zero_page_operand(bus);
        self.compare(self.x, value);
        3
    }

    #[inline]
    pub(super) fn cpx_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_absolute_operand(bus);
        self.compare(self.x, value);
        4
    }
}
