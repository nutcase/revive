use super::*;

impl Cpu {
    pub(super) fn execute_unofficial_load(&mut self, opcode: u8, bus: &mut dyn CpuBus) -> u8 {
        match opcode {
            // LAX unofficial opcodes (LDA + TAX combined)
            0xA7 | 0xB7 | 0xAF | 0xBF | 0xA3 | 0xB3 | 0xAB => {
                match opcode {
                    0xAB => {
                        // LAX immediate - Load A and X with memory
                        let value = self.read_byte(bus);
                        self.a = value;
                        self.x = value;
                        self.set_zero_negative_flags(value);
                        2
                    }
                    0xA7 => {
                        // LAX zero page
                        let addr = self.read_byte(bus) as u16;
                        let value = bus.read(addr);
                        self.a = value;
                        self.x = value;
                        self.set_zero_negative_flags(value);
                        3
                    }
                    0xB7 => {
                        // LAX zero page,Y
                        let addr = self.get_zero_page_y_addr(bus);
                        let value = bus.read(addr);
                        self.a = value;
                        self.x = value;
                        self.set_zero_negative_flags(value);
                        4
                    }
                    0xAF => {
                        // LAX absolute
                        let addr = self.read_word(bus);
                        let value = bus.read(addr);
                        self.a = value;
                        self.x = value;
                        self.set_zero_negative_flags(value);
                        4
                    }
                    0xBF => {
                        // LAX absolute,Y
                        let addr = self.read_word(bus);
                        let effective_addr = addr.wrapping_add(self.y as u16);
                        let value = bus.read(effective_addr);
                        self.a = value;
                        self.x = value;
                        self.set_zero_negative_flags(value);
                        4
                    }
                    0xA3 => {
                        // LAX (indirect,X)
                        let addr = self.get_indexed_indirect_addr(bus);
                        let value = bus.read(addr);
                        self.a = value;
                        self.x = value;
                        self.set_zero_negative_flags(value);
                        6
                    }
                    0xB3 => {
                        // LAX (indirect),Y
                        let (addr, _) = self.get_indirect_indexed_addr(bus);
                        let value = bus.read(addr);
                        self.a = value;
                        self.x = value;
                        self.set_zero_negative_flags(value);
                        5
                    }
                    _ => 2,
                }
            }
            0xBB => {
                // LAS absolute,Y - Load A, X, S with memory AND S
                let addr = self.read_word(bus);
                let effective_addr = addr.wrapping_add(self.y as u16);
                let value = bus.read(effective_addr) & self.sp;
                self.a = value;
                self.x = value;
                // Only update SP if result is reasonable (>= 0x80)
                if value >= 0x80 {
                    self.sp = value;
                }
                self.set_zero_negative_flags(value);
                4
            }
            _ => unreachable!("unhandled unofficial opcode: 0x{opcode:02X}"),
        }
    }
}
