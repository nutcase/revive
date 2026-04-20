use super::*;

impl Cpu {
    fn slo_at(&mut self, bus: &mut dyn CpuBus, addr: u16) {
        let value = bus.read(addr);
        self.status.set(StatusFlags::CARRY, value & 0x80 != 0);
        let shifted = value << 1;
        bus.write(addr, shifted);
        self.a |= shifted;
        self.set_zero_negative_flags(self.a);
    }

    fn sre_at(&mut self, bus: &mut dyn CpuBus, addr: u16) {
        let value = bus.read(addr);
        let shifted = value >> 1;
        bus.write(addr, shifted);
        self.status.set(StatusFlags::CARRY, value & 0x01 != 0);
        self.a ^= shifted;
        self.set_zero_negative_flags(self.a);
    }

    pub(super) fn execute_unofficial_shift(&mut self, opcode: u8, bus: &mut dyn CpuBus) -> u8 {
        match opcode {
            0x03 => {
                // SLO (indirect,X) - Shift Left, OR
                let addr = self.get_indexed_indirect_addr(bus);
                self.slo_at(bus, addr);
                8
            }
            0x07 => {
                // SLO zero page - Shift Left, OR
                let addr = self.read_byte(bus) as u16;
                self.slo_at(bus, addr);
                5
            }
            0x0F => {
                // SLO absolute - Shift Left, OR
                let addr = self.read_word(bus);
                self.slo_at(bus, addr);
                6
            }
            0x13 => {
                // SLO (indirect),Y - Shift Left, OR
                let (addr, _) = self.get_indirect_indexed_addr(bus);
                self.slo_at(bus, addr);
                8
            }
            0x17 => {
                // SLO zero page,X - Shift Left, OR
                let addr = self.get_zero_page_x_addr(bus);
                self.slo_at(bus, addr);
                6
            }
            0x1B => {
                // SLO absolute,Y - Shift Left, OR
                let addr = self.read_word(bus).wrapping_add(self.y as u16);
                self.slo_at(bus, addr);
                7
            }
            0x1F => {
                // SLO absolute,X - Shift Left, OR
                let addr = self.read_word(bus).wrapping_add(self.x as u16);
                self.slo_at(bus, addr);
                7
            }
            0x43 => {
                // SRE (indirect,X) - Shift Right and Exclusive OR
                let addr = self.get_indexed_indirect_addr(bus);
                self.sre_at(bus, addr);
                8
            }
            0x4F => {
                // SRE absolute - Shift Right and Exclusive OR
                let addr = self.read_word(bus);
                self.sre_at(bus, addr);
                6
            }
            0x53 => {
                // SRE (indirect),Y - Shift Right and Exclusive OR
                let (addr, _) = self.get_indirect_indexed_addr(bus);
                self.sre_at(bus, addr);
                8
            }
            0x57 => {
                // SRE zero page,X - Shift Right and Exclusive OR
                let addr = self.get_zero_page_x_addr(bus);
                self.sre_at(bus, addr);
                6
            }
            0x5B => {
                // SRE absolute,Y - Shift Right and Exclusive OR
                let addr = self.read_word(bus).wrapping_add(self.y as u16);
                self.sre_at(bus, addr);
                7
            }
            0x5F => {
                // SRE absolute,X - Shift Right, EOR
                let addr = self.read_word(bus).wrapping_add(self.x as u16);
                self.sre_at(bus, addr);
                7
            }
            _ => unreachable!("unhandled unofficial opcode: 0x{opcode:02X}"),
        }
    }
}
