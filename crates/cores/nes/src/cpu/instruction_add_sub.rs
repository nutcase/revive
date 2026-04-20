use super::*;

impl Cpu {
    #[inline]
    pub(super) fn adc_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_immediate_operand(bus);
        self.adc(value);
        2
    }

    #[inline]
    pub(super) fn adc(&mut self, value: u8) {
        let carry = if self.status.contains(StatusFlags::CARRY) {
            1
        } else {
            0
        };
        let result = self.a as u16 + value as u16 + carry;

        self.status.set(StatusFlags::CARRY, result > 0xFF);
        self.status.set(
            StatusFlags::OVERFLOW,
            (self.a ^ result as u8) & (value ^ result as u8) & 0x80 != 0,
        );

        self.a = result as u8;
        self.set_zero_negative_flags(self.a);
    }

    #[inline]
    pub(super) fn adc_indexed_indirect(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_indexed_indirect_operand(bus);
        self.adc(value);
        6
    }

    #[inline]
    pub(super) fn adc_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_zero_page_operand(bus);
        self.adc(value);
        3
    }

    #[inline]
    pub(super) fn adc_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_absolute_operand(bus);
        self.adc(value);
        4
    }

    #[inline]
    pub(super) fn adc_indirect_indexed(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_indirect_indexed_operand(bus);
        self.adc(value);
        if page_crossed {
            6
        } else {
            5
        }
    }

    #[inline]
    pub(super) fn adc_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_zero_page_x_operand(bus);
        self.adc(value);
        4
    }

    #[inline]
    pub(super) fn adc_absolute_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_y_operand(bus);
        self.adc(value);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(super) fn adc_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_x_operand(bus);
        self.adc(value);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(super) fn sbc(&mut self, value: u8) {
        self.adc(!value);
    }

    #[inline]
    pub(super) fn sbc_indexed_indirect(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_indexed_indirect_operand(bus);
        self.sbc(value);
        6
    }

    #[inline]
    pub(super) fn sbc_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_zero_page_operand(bus);
        self.sbc(value);
        3
    }

    #[inline]
    pub(super) fn sbc_immediate(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_immediate_operand(bus);
        self.sbc(value);
        2
    }

    #[inline]
    pub(super) fn sbc_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_absolute_operand(bus);
        self.sbc(value);
        4
    }

    #[inline]
    pub(super) fn sbc_indirect_indexed(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_indirect_indexed_operand(bus);
        self.sbc(value);
        if page_crossed {
            6
        } else {
            5
        }
    }

    #[inline]
    pub(super) fn sbc_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let value = self.read_zero_page_x_operand(bus);
        self.sbc(value);
        4
    }

    #[inline]
    pub(super) fn sbc_absolute_y(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_y_operand(bus);
        self.sbc(value);
        if page_crossed {
            5
        } else {
            4
        }
    }

    #[inline]
    pub(super) fn sbc_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let (value, page_crossed) = self.read_absolute_x_operand(bus);
        self.sbc(value);
        if page_crossed {
            5
        } else {
            4
        }
    }
}
