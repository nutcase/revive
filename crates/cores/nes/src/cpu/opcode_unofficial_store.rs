use super::*;

impl Cpu {
    pub(super) fn execute_unofficial_store(&mut self, opcode: u8, bus: &mut dyn CpuBus) -> u8 {
        match opcode {
            0x87 => {
                // SAX zero page - Store A AND X
                let addr = self.read_byte(bus) as u16;
                let value = self.a & self.x;
                bus.write(addr, value);
                3
            }
            0x8F => {
                // SAX absolute - Store A AND X
                let addr = self.read_word(bus);
                let value = self.a & self.x;
                bus.write(addr, value);
                4
            }
            0x9C => {
                // SHY absolute,X - Store Y AND (high byte of original addr + 1) [UNSTABLE]
                let addr = self.read_word(bus);
                let effective_addr = addr.wrapping_add(self.x as u16);
                let high_byte = (addr >> 8) as u8; // Use original addr, not effective_addr
                let value = self.y & high_byte.wrapping_add(1);
                bus.write(effective_addr, value);
                5
            }
            0x9F => {
                // SHY absolute,X - Store Y AND (high byte of address + 1)
                // Store Y AND (high byte of address + 1)
                let addr = self.read_word(bus); // Read absolute address
                let effective_addr = addr.wrapping_add(self.x as u16);
                let high_byte = (effective_addr >> 8) as u8;
                let store_value = self.y & high_byte.wrapping_add(1);
                bus.write(effective_addr, store_value);
                5 // 5 cycles for absolute,X with write
            }
            0x93 => {
                // SHA/AHX (indirect),Y - Store A AND X AND (H+1) [UNSTABLE]
                // WARNING: This is an unstable instruction - behavior varies between 6502 chips
                // Official spec: A & X & (high byte of target address + 1) → memory
                let (addr, _) = self.get_indirect_indexed_addr(bus);
                let high_byte = (addr >> 8) as u8;
                let value = self.a & self.x & high_byte.wrapping_add(1);
                bus.write(addr, value);
                6
            }
            0x97 => {
                // SAX zero page,Y - Store A AND X
                let addr = self.get_zero_page_y_addr(bus);
                let value = self.a & self.x;
                bus.write(addr, value);
                4
            }
            0x9B => {
                // TAS/XAS absolute,Y - Transfer A AND X to SP, Store A AND X AND (H+1) [UNSTABLE]
                // WARNING: This is an unstable instruction - behavior varies between 6502 chips
                // WARNING: This can corrupt the stack pointer!
                let addr = self.read_word(bus);
                let effective_addr = addr.wrapping_add(self.y as u16);
                // Only update SP if result is reasonable (>= 0x80)
                let new_sp = self.a & self.x;
                if new_sp >= 0x80 {
                    self.sp = new_sp;
                }
                let high_byte = (addr >> 8) as u8;
                let value = self.a & self.x & high_byte.wrapping_add(1);
                bus.write(effective_addr, value);
                5
            }
            0x9E => {
                // SHX absolute,Y - Store X AND (high byte + 1) [UNSTABLE]
                let addr = self.read_word(bus);
                let effective_addr = addr.wrapping_add(self.y as u16);
                let high_byte = (addr >> 8) as u8;
                let value = self.x & high_byte.wrapping_add(1);
                bus.write(effective_addr, value);
                5
            }
            _ => unreachable!("unhandled unofficial opcode: 0x{opcode:02X}"),
        }
    }
}
