use super::*;

impl Cpu {
    pub(super) fn execute_unofficial_nop(&mut self, opcode: u8, bus: &mut dyn CpuBus) -> u8 {
        match opcode {
            // Common unofficial NOPs
            0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => 2,
            0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 => {
                self.read_byte(bus); // Consume immediate byte
                2
            }
            0x04 | 0x44 | 0x64 => {
                self.read_byte(bus); // Consume zero page byte
                3
            }
            0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => {
                self.read_byte(bus); // Consume zero page,X byte
                4
            }
            0x0C => {
                self.read_word(bus); // Consume absolute address - NOP absolute
                4
            }
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => {
                self.read_word(bus); // Consume absolute,X address - NOP absolute,X
                4 // Could be 5 with page crossing, but we'll use 4
            }
            _ => unreachable!("unhandled unofficial opcode: 0x{opcode:02X}"),
        }
    }
}
