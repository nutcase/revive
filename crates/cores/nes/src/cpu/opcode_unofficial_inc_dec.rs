use super::*;

impl Cpu {
    pub(super) fn execute_unofficial_inc_dec(&mut self, opcode: u8, bus: &mut dyn CpuBus) -> u8 {
        match opcode {
            0xFB => {
                // ISC absolute,Y - Increment and Subtract with Carry
                let addr = self.read_word(bus);
                let effective_addr = addr.wrapping_add(self.y as u16);
                let value = bus.read(effective_addr);
                let incremented = value.wrapping_add(1);
                bus.write(effective_addr, incremented);
                self.sbc(incremented);
                7
            }
            0xE3 => {
                // ISC (indirect,X) - Increment, Subtract with Carry
                let addr = self.get_indexed_indirect_addr(bus);
                let value = bus.read(addr);
                let incremented = value.wrapping_add(1);
                bus.write(addr, incremented);
                self.sbc(incremented);
                8
            }
            0xF7 => {
                // ISC zero page,X - Increment, Subtract with Carry
                let addr = self.get_zero_page_x_addr(bus);
                let value = bus.read(addr);
                let incremented = value.wrapping_add(1);
                bus.write(addr, incremented);
                self.sbc(incremented);
                6
            }
            0xEF => {
                // ISC absolute - Increment, Subtract with Carry
                let addr = self.read_word(bus);
                let value = bus.read(addr);
                let incremented = value.wrapping_add(1);
                bus.write(addr, incremented);
                self.sbc(incremented);
                6
            }
            0xFF => {
                // ISC absolute,X - unofficial opcode (duplicate implementation)
                let addr = self.read_word(bus);
                let effective_addr = addr.wrapping_add(self.x as u16);
                let value = bus.read(effective_addr);
                let incremented = value.wrapping_add(1);
                bus.write(effective_addr, incremented);
                self.sbc(incremented);
                7
            }
            0xD3 => {
                // DCP (indirect),Y - Decrement, Compare
                let (addr, _) = self.get_indirect_indexed_addr(bus);
                let value = bus.read(addr);
                let decremented = value.wrapping_sub(1);
                bus.write(addr, decremented);
                self.compare(self.a, decremented);
                8
            }
            0xD7 => {
                // DCP zero page,X - Decrement, Compare
                let addr = self.get_zero_page_x_addr(bus);
                let value = bus.read(addr);
                let decremented = value.wrapping_sub(1);
                bus.write(addr, decremented);
                self.compare(self.a, decremented);
                6
            }
            0xDB => {
                // DCP absolute,Y - Decrement, Compare
                let addr = self.read_word(bus);
                let effective_addr = addr.wrapping_add(self.y as u16);
                let value = bus.read(effective_addr);
                let decremented = value.wrapping_sub(1);
                bus.write(effective_addr, decremented);
                self.compare(self.a, decremented);
                7
            }
            0xDF => {
                // DCP absolute,X - Decrement, Compare
                let addr = self.read_word(bus);
                let effective_addr = addr.wrapping_add(self.x as u16);
                let value = bus.read(effective_addr);
                let decremented = value.wrapping_sub(1);
                bus.write(effective_addr, decremented);
                self.compare(self.a, decremented);
                7
            }
            0xE7 => {
                // ISC zero page - Increment, Subtract with Carry
                let addr = self.read_byte(bus) as u16;
                let value = bus.read(addr);
                let incremented = value.wrapping_add(1);
                bus.write(addr, incremented);
                self.sbc(incremented);
                5
            }
            0xC3 => {
                // DCP (indirect,X) - Decrement, Compare
                let addr = self.get_indexed_indirect_addr(bus);
                let value = bus.read(addr);
                let decremented = value.wrapping_sub(1);
                bus.write(addr, decremented);
                self.compare(self.a, decremented);
                8
            }
            0xC7 => {
                // DCP zero page - Decrement, Compare
                let addr = self.read_byte(bus) as u16;
                let value = bus.read(addr);
                let decremented = value.wrapping_sub(1);
                bus.write(addr, decremented);
                self.compare(self.a, decremented);
                5
            }
            0xCF => {
                // DCP absolute - Decrement, Compare
                let addr = self.read_word(bus);
                let value = bus.read(addr);
                let decremented = value.wrapping_sub(1);
                bus.write(addr, decremented);
                self.compare(self.a, decremented);
                6
            }
            0xF3 => {
                // ISC (indirect),Y - Increment, Subtract with Carry
                let (addr, _) = self.get_indirect_indexed_addr(bus);
                let value = bus.read(addr);
                let incremented = value.wrapping_add(1);
                bus.write(addr, incremented);
                self.sbc(incremented);
                8
            }
            _ => unreachable!("unhandled unofficial opcode: 0x{opcode:02X}"),
        }
    }
}
