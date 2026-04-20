use super::*;

impl Cpu {
    fn rla_at(&mut self, bus: &mut dyn CpuBus, addr: u16) {
        let value = bus.read(addr);
        let carry = u8::from(self.status.contains(StatusFlags::CARRY));
        let rotated = (value << 1) | carry;
        self.status.set(StatusFlags::CARRY, value & 0x80 != 0);
        bus.write(addr, rotated);
        self.a &= rotated;
        self.set_zero_negative_flags(self.a);
    }

    fn rra_at(&mut self, bus: &mut dyn CpuBus, addr: u16) {
        let value = bus.read(addr);
        let carry = if self.status.contains(StatusFlags::CARRY) {
            0x80
        } else {
            0
        };
        let rotated = (value >> 1) | carry;
        self.status.set(StatusFlags::CARRY, value & 0x01 != 0);
        bus.write(addr, rotated);
        self.adc(rotated);
    }

    pub(super) fn execute_unofficial_rotate(&mut self, opcode: u8, bus: &mut dyn CpuBus) -> u8 {
        match opcode {
            0x23 => {
                // RLA (indirect,X) - Rotate Left, AND
                let addr = self.get_indexed_indirect_addr(bus);
                self.rla_at(bus, addr);
                8
            }
            0x27 => {
                // RLA zero page - Rotate Left, AND
                let addr = self.read_byte(bus) as u16;
                self.rla_at(bus, addr);
                5
            }
            0x2F => {
                // RLA absolute - Rotate Left, AND
                let addr = self.read_word(bus);
                self.rla_at(bus, addr);
                6
            }
            0x33 => {
                // RLA (indirect),Y - Rotate Left, AND
                let (addr, _) = self.get_indirect_indexed_addr(bus);
                self.rla_at(bus, addr);
                8
            }
            0x37 => {
                // RLA zero page,X - Rotate Left, AND
                let addr = self.get_zero_page_x_addr(bus);
                self.rla_at(bus, addr);
                6
            }
            0x3B => {
                // RLA absolute,Y - Rotate Left, AND
                let addr = self.read_word(bus).wrapping_add(self.y as u16);
                self.rla_at(bus, addr);
                7
            }
            0x3F => {
                // RLA absolute,X - Rotate Left, AND
                let addr = self.read_word(bus).wrapping_add(self.x as u16);
                self.rla_at(bus, addr);
                7
            }
            0x63 => {
                // RRA (indirect,X) - Rotate Right, Add
                let addr = self.get_indexed_indirect_addr(bus);
                self.rra_at(bus, addr);
                8
            }
            0x67 => {
                // RRA zero page - Rotate Right, Add
                let addr = self.read_byte(bus) as u16;
                self.rra_at(bus, addr);
                5
            }
            0x6F => {
                // RRA absolute - Rotate Right, Add
                let addr = self.read_word(bus);
                self.rra_at(bus, addr);
                6
            }
            0x73 => {
                // RRA (indirect),Y - Rotate Right, Add
                let (addr, _) = self.get_indirect_indexed_addr(bus);
                self.rra_at(bus, addr);
                8
            }
            0x77 => {
                // RRA zero page,X - Rotate Right, Add
                let addr = self.get_zero_page_x_addr(bus);
                self.rra_at(bus, addr);
                6
            }
            0x7B => {
                // RRA absolute,Y - Rotate Right, Add
                let addr = self.read_word(bus).wrapping_add(self.y as u16);
                self.rra_at(bus, addr);
                7
            }
            0x7F => {
                // RRA absolute,X - Rotate Right, Add
                let addr = self.read_word(bus).wrapping_add(self.x as u16);
                self.rra_at(bus, addr);
                7
            }
            _ => unreachable!("unhandled unofficial opcode: 0x{opcode:02X}"),
        }
    }
}
