use super::*;

impl Cpu {
    pub(super) fn execute_unofficial_immediate(&mut self, opcode: u8, bus: &mut dyn CpuBus) -> u8 {
        match opcode {
            0x0B => {
                // ANC immediate - AND, Copy N to C
                let value = self.read_byte(bus);
                self.a &= value;
                self.set_zero_negative_flags(self.a);
                self.status.set(
                    StatusFlags::CARRY,
                    self.status.contains(StatusFlags::NEGATIVE),
                );
                2
            }
            0x8B => {
                // XAA immediate - (A OR CONST) AND X AND immediate [EXTREMELY UNSTABLE]
                // WARNING: Extremely unstable - behavior varies by temperature, chip, etc.
                // Using magic constant 0xFF for basic compatibility
                let value = self.read_byte(bus);
                let magic_const = 0xFF; // Common fallback value
                self.a = ((self.a | magic_const) & self.x) & value;
                self.set_zero_negative_flags(self.a);
                2
            }
            0x2B => {
                // ANC immediate - AND, Copy N to C (duplicate of 0x0B)
                let value = self.read_byte(bus);
                self.a &= value;
                self.set_zero_negative_flags(self.a);
                self.status.set(
                    StatusFlags::CARRY,
                    self.status.contains(StatusFlags::NEGATIVE),
                );
                2
            }
            0x6B => {
                // ARR immediate - AND with accumulator, then rotate right
                let value = self.read_byte(bus);
                self.a &= value;
                let carry = if self.status.contains(StatusFlags::CARRY) {
                    0x80
                } else {
                    0
                };
                let result = (self.a >> 1) | carry;
                self.status.set(StatusFlags::CARRY, self.a & 0x01 != 0);
                self.status.set(
                    StatusFlags::OVERFLOW,
                    ((result ^ (result << 1)) & 0x40) != 0,
                );
                self.a = result;
                self.set_zero_negative_flags(self.a);
                2
            }
            0x4B => {
                // ALR immediate - AND + LSR
                let value = self.read_byte(bus);
                self.a &= value;
                self.status.set(StatusFlags::CARRY, self.a & 0x01 != 0);
                self.a >>= 1;
                self.set_zero_negative_flags(self.a);
                2
            }
            0xEB => {
                // SBC immediate (unofficial duplicate of 0xE9)
                let value = self.read_byte(bus);
                self.sbc(value);
                2
            }
            0xCB => {
                // AXS immediate - AND X register with accumulator, subtract immediate
                let value = self.read_byte(bus);
                let and_result = self.a & self.x;
                let result = and_result.wrapping_sub(value);
                self.x = result;
                self.status.set(StatusFlags::CARRY, and_result >= value);
                self.set_zero_negative_flags(result);
                2
            }
            _ => unreachable!("unhandled unofficial opcode: 0x{opcode:02X}"),
        }
    }
}
