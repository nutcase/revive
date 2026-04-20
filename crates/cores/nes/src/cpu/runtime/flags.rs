use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn set_zero_negative_flags(&mut self, value: u8) {
        self.status.set(StatusFlags::ZERO, value == 0);
        self.status.set(StatusFlags::NEGATIVE, value & 0x80 != 0);
    }

    #[inline]
    pub(in crate::cpu) fn branch(&mut self, bus: &mut dyn CpuBus, condition: bool) -> u8 {
        // Branch instructions: read offset byte and conditionally branch
        let branch_pc = self.pc.wrapping_sub(1); // PC where branch instruction was located
        let offset = self.read_byte(bus) as i8;
        if condition {
            let new_pc = self.pc.wrapping_add(offset as u16);

            // Page crossing check should use the branch instruction PC vs destination PC
            let cycles = if (branch_pc & 0xFF00) != (new_pc & 0xFF00) {
                4
            } else {
                3
            };
            self.pc = new_pc;
            cycles
        } else {
            2
        }
    }
}
