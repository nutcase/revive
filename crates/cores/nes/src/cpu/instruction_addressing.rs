use super::*;

impl Cpu {
    #[inline]
    pub(super) fn get_indexed_indirect_addr(&mut self, bus: &mut dyn CpuBus) -> u16 {
        let base = self.read_byte(bus);
        let addr = base.wrapping_add(self.x);
        let low = bus.read(addr as u16) as u16;
        let high = bus.read(addr.wrapping_add(1) as u16) as u16;
        (high << 8) | low
    }

    #[inline]
    pub(super) fn get_indirect_indexed_addr(&mut self, bus: &mut dyn CpuBus) -> (u16, bool) {
        let base = self.read_byte(bus);
        let low = bus.read(base as u16) as u16;
        let high = bus.read(base.wrapping_add(1) as u16) as u16;
        let addr = (high << 8) | low;
        let final_addr = addr.wrapping_add(self.y as u16);
        let page_crossed = (addr & 0xFF00) != (final_addr & 0xFF00);
        (final_addr, page_crossed)
    }

    #[inline]
    pub(super) fn get_zero_page_x_addr(&mut self, bus: &mut dyn CpuBus) -> u16 {
        let base = self.read_byte(bus);
        base.wrapping_add(self.x) as u16
    }

    #[inline]
    pub(super) fn get_zero_page_y_addr(&mut self, bus: &mut dyn CpuBus) -> u16 {
        let base = self.read_byte(bus);
        base.wrapping_add(self.y) as u16
    }

    #[inline]
    pub(super) fn get_absolute_x_addr(&mut self, bus: &mut dyn CpuBus) -> (u16, bool) {
        let base = self.read_word(bus);
        let addr = base.wrapping_add(self.x as u16);
        let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, page_crossed)
    }

    #[inline]
    pub(super) fn get_absolute_y_addr(&mut self, bus: &mut dyn CpuBus) -> (u16, bool) {
        let base = self.read_word(bus);
        let addr = base.wrapping_add(self.y as u16);
        let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, page_crossed)
    }

    // ORA instructions
}
