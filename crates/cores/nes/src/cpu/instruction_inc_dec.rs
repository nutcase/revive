use super::*;

impl Cpu {
    #[inline]
    pub(super) fn dey(&mut self) -> u8 {
        self.y = self.y.wrapping_sub(1);
        self.set_zero_negative_flags(self.y);
        2
    }

    #[inline]
    pub(super) fn dec_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let result = self.modify_zero_page_operand(bus, |value| value.wrapping_sub(1));
        self.set_zero_negative_flags(result);
        5
    }

    #[inline]
    pub(super) fn iny(&mut self) -> u8 {
        self.y = self.y.wrapping_add(1);
        self.set_zero_negative_flags(self.y);
        2
    }

    #[inline]
    pub(super) fn dex(&mut self) -> u8 {
        self.x = self.x.wrapping_sub(1);
        self.set_zero_negative_flags(self.x);
        2
    }

    #[inline]
    pub(super) fn dec_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let result = self.modify_absolute_operand(bus, |value| value.wrapping_sub(1));
        self.set_zero_negative_flags(result);
        6
    }

    #[inline]
    pub(super) fn dec_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let result = self.modify_zero_page_x_operand(bus, |value| value.wrapping_sub(1));
        self.set_zero_negative_flags(result);
        6
    }

    #[inline]
    pub(super) fn dec_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let result = self.modify_absolute_x_operand(bus, |value| value.wrapping_sub(1));
        self.set_zero_negative_flags(result);
        7
    }

    #[inline]
    pub(super) fn inc_zero_page(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let result = self.modify_zero_page_operand(bus, |value| value.wrapping_add(1));
        self.set_zero_negative_flags(result);
        5
    }

    #[inline]
    pub(super) fn inx(&mut self) -> u8 {
        self.x = self.x.wrapping_add(1);
        self.set_zero_negative_flags(self.x);
        2
    }

    #[inline]
    pub(super) fn inc_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let result = self.modify_absolute_operand(bus, |value| value.wrapping_add(1));
        self.set_zero_negative_flags(result);
        6
    }

    #[inline]
    pub(super) fn inc_zero_page_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let result = self.modify_zero_page_x_operand(bus, |value| value.wrapping_add(1));
        self.set_zero_negative_flags(result);
        6
    }

    #[inline]
    pub(super) fn inc_absolute_x(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let result = self.modify_absolute_x_operand(bus, |value| value.wrapping_add(1));
        self.set_zero_negative_flags(result);
        7
    }
}
