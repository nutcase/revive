use super::*;

impl Cpu {
    #[inline]
    pub(super) fn read_immediate_operand(&mut self, bus: &mut dyn CpuBus) -> u8 {
        self.read_byte(bus)
    }

    #[inline]
    pub(super) fn read_zero_page_operand(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.read_byte(bus) as u16;
        bus.read(addr)
    }

    #[inline]
    pub(super) fn read_zero_page_x_operand(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.get_zero_page_x_addr(bus);
        bus.read(addr)
    }

    #[inline]
    pub(super) fn read_zero_page_y_operand(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.get_zero_page_y_addr(bus);
        bus.read(addr)
    }

    #[inline]
    pub(super) fn read_absolute_operand(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.read_word(bus);
        bus.read(addr)
    }

    #[inline]
    pub(super) fn read_absolute_x_operand(&mut self, bus: &mut dyn CpuBus) -> (u8, bool) {
        let (addr, page_crossed) = self.get_absolute_x_addr(bus);
        (bus.read(addr), page_crossed)
    }

    #[inline]
    pub(super) fn read_absolute_y_operand(&mut self, bus: &mut dyn CpuBus) -> (u8, bool) {
        let (addr, page_crossed) = self.get_absolute_y_addr(bus);
        (bus.read(addr), page_crossed)
    }

    #[inline]
    pub(super) fn read_indexed_indirect_operand(&mut self, bus: &mut dyn CpuBus) -> u8 {
        let addr = self.get_indexed_indirect_addr(bus);
        bus.read(addr)
    }

    #[inline]
    pub(super) fn read_indirect_indexed_operand(&mut self, bus: &mut dyn CpuBus) -> (u8, bool) {
        let (addr, page_crossed) = self.get_indirect_indexed_addr(bus);
        (bus.read(addr), page_crossed)
    }

    #[inline]
    pub(super) fn write_indexed_indirect_operand(&mut self, bus: &mut dyn CpuBus, value: u8) {
        let addr = self.get_indexed_indirect_addr(bus);
        bus.write(addr, value);
    }

    #[inline]
    pub(super) fn write_zero_page_operand(&mut self, bus: &mut dyn CpuBus, value: u8) {
        let addr = self.read_byte(bus) as u16;
        bus.write(addr, value);
    }

    #[inline]
    pub(super) fn write_zero_page_x_operand(&mut self, bus: &mut dyn CpuBus, value: u8) {
        let addr = self.get_zero_page_x_addr(bus);
        bus.write(addr, value);
    }

    #[inline]
    pub(super) fn write_zero_page_y_operand(&mut self, bus: &mut dyn CpuBus, value: u8) {
        let addr = self.get_zero_page_y_addr(bus);
        bus.write(addr, value);
    }

    #[inline]
    pub(super) fn write_absolute_operand(&mut self, bus: &mut dyn CpuBus, value: u8) {
        let addr = self.read_word(bus);
        bus.write(addr, value);
    }

    #[inline]
    pub(super) fn write_absolute_x_operand(&mut self, bus: &mut dyn CpuBus, value: u8) {
        let (addr, _) = self.get_absolute_x_addr(bus);
        bus.write(addr, value);
    }

    #[inline]
    pub(super) fn write_absolute_y_operand(&mut self, bus: &mut dyn CpuBus, value: u8) {
        let (addr, _) = self.get_absolute_y_addr(bus);
        bus.write(addr, value);
    }

    #[inline]
    pub(super) fn write_indirect_indexed_operand(&mut self, bus: &mut dyn CpuBus, value: u8) {
        let (addr, _) = self.get_indirect_indexed_addr(bus);
        bus.write(addr, value);
    }

    #[inline]
    pub(super) fn modify_zero_page_operand(
        &mut self,
        bus: &mut dyn CpuBus,
        op: impl FnOnce(u8) -> u8,
    ) -> u8 {
        let addr = self.read_byte(bus) as u16;
        self.modify_operand_at(bus, addr, op)
    }

    #[inline]
    pub(super) fn modify_zero_page_x_operand(
        &mut self,
        bus: &mut dyn CpuBus,
        op: impl FnOnce(u8) -> u8,
    ) -> u8 {
        let addr = self.get_zero_page_x_addr(bus);
        self.modify_operand_at(bus, addr, op)
    }

    #[inline]
    pub(super) fn modify_absolute_operand(
        &mut self,
        bus: &mut dyn CpuBus,
        op: impl FnOnce(u8) -> u8,
    ) -> u8 {
        let addr = self.read_word(bus);
        self.modify_operand_at(bus, addr, op)
    }

    #[inline]
    pub(super) fn modify_absolute_x_operand(
        &mut self,
        bus: &mut dyn CpuBus,
        op: impl FnOnce(u8) -> u8,
    ) -> u8 {
        let (addr, _) = self.get_absolute_x_addr(bus);
        self.modify_operand_at(bus, addr, op)
    }

    #[inline]
    fn modify_operand_at(
        &mut self,
        bus: &mut dyn CpuBus,
        addr: u16,
        op: impl FnOnce(u8) -> u8,
    ) -> u8 {
        let value = bus.read(addr);
        let result = op(value);
        bus.write(addr, result);
        result
    }
}
