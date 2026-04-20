use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn sta_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_zero_page_operand(bus, self.a);
        3
    }

    #[inline]
    pub(in crate::cpu) fn sta_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_absolute_operand(bus, self.a);
        4
    }

    #[inline]
    pub(in crate::cpu) fn sta_indexed_indirect(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_indexed_indirect_operand(bus, self.a);
        6
    }

    #[inline]
    pub(in crate::cpu) fn sax_indexed_indirect(&mut self, bus: &mut dyn CpuBus) -> u8 {
        // SAX: Store A & X - unofficial opcode
        self.write_indexed_indirect_operand(bus, self.a & self.x);
        6
    }

    #[inline]
    pub(in crate::cpu) fn sty_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_zero_page_operand(bus, self.y);
        3
    }

    #[inline]
    pub(in crate::cpu) fn stx_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_zero_page_operand(bus, self.x);
        3
    }

    #[inline]
    pub(in crate::cpu) fn sty_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_absolute_operand(bus, self.y);
        4
    }

    #[inline]
    pub(in crate::cpu) fn stx_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_absolute_operand(bus, self.x);
        4
    }

    #[inline]
    pub(in crate::cpu) fn sta_indirect_indexed(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_indirect_indexed_operand(bus, self.a);
        6
    }

    #[inline]
    pub(in crate::cpu) fn sty_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_zero_page_x_operand(bus, self.y);
        4
    }

    #[inline]
    pub(in crate::cpu) fn sta_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_zero_page_x_operand(bus, self.a);
        4
    }

    #[inline]
    pub(in crate::cpu) fn stx_zero_page_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_zero_page_y_operand(bus, self.x);
        4
    }

    #[inline]
    pub(in crate::cpu) fn sta_absolute_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_absolute_y_operand(bus, self.a);
        5
    }

    #[inline]
    pub(in crate::cpu) fn sta_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.write_absolute_x_operand(bus, self.a);
        5
    }
}
