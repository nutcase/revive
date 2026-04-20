use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn lda_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.a = self.read_immediate_operand(bus);
        self.set_zero_negative_flags(self.a);
        2
    }

    #[inline]
    pub(in crate::cpu) fn lda_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.a = self.read_zero_page_operand(bus);
        self.set_zero_negative_flags(self.a);
        3
    }

    #[inline]
    pub(in crate::cpu) fn lda_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.a = self.read_absolute_operand(bus);
        self.set_zero_negative_flags(self.a);
        4
    }

    #[inline]
    pub(in crate::cpu) fn ldy_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.y = self.read_immediate_operand(bus);
        self.set_zero_negative_flags(self.y);
        2
    }

    #[inline]
    pub(in crate::cpu) fn lda_indexed_indirect(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.a = self.read_indexed_indirect_operand(bus);
        self.set_zero_negative_flags(self.a);
        6
    }

    #[inline]
    pub(in crate::cpu) fn ldx_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.x = self.read_immediate_operand(bus);
        self.set_zero_negative_flags(self.x);
        2
    }

    #[inline]
    pub(in crate::cpu) fn ldy_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.y = self.read_zero_page_operand(bus);
        self.set_zero_negative_flags(self.y);
        3
    }

    #[inline]
    pub(in crate::cpu) fn ldx_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.x = self.read_zero_page_operand(bus);
        self.set_zero_negative_flags(self.x);
        3
    }

    #[inline]
    pub(in crate::cpu) fn ldy_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.y = self.read_absolute_operand(bus);
        self.set_zero_negative_flags(self.y);
        4
    }

    #[inline]
    pub(in crate::cpu) fn ldx_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.x = self.read_absolute_operand(bus);
        self.set_zero_negative_flags(self.x);
        4
    }

    #[inline]
    pub(in crate::cpu) fn lda_indirect_indexed(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_indirect_indexed_operand(bus);
        self.a = value;
        self.set_zero_negative_flags(self.a);
        if page_crossed {
            6
        } else {
            5
        }
    }

    #[inline]
    pub(in crate::cpu) fn ldy_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.y = self.read_zero_page_x_operand(bus);
        self.set_zero_negative_flags(self.y);
        4
    }

    #[inline]
    pub(in crate::cpu) fn lda_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.a = self.read_zero_page_x_operand(bus);
        self.set_zero_negative_flags(self.a);
        4
    }

    #[inline]
    pub(in crate::cpu) fn ldx_zero_page_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.x = self.read_zero_page_y_operand(bus);
        self.set_zero_negative_flags(self.x);
        4
    }

    #[inline]
    pub(in crate::cpu) fn lda_absolute_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_y_operand(bus);
        self.a = value;
        self.set_zero_negative_flags(self.a);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(in crate::cpu) fn ldy_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_x_operand(bus);
        self.y = value;
        self.set_zero_negative_flags(self.y);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(in crate::cpu) fn lda_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_x_operand(bus);
        self.a = value;
        self.set_zero_negative_flags(self.a);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(in crate::cpu) fn ldx_absolute_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_y_operand(bus);
        self.x = value;
        self.set_zero_negative_flags(self.x);
        if page_crossed {
            5
        } else {
            4
        }
    }
}
