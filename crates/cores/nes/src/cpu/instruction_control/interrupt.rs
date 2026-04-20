use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn brk(&mut self, bus: &mut dyn CpuBus) -> u8 {
        // BRK is a 2-byte instruction (0x00 + signature byte)
        // PC has already been incremented to point to signature byte
        // Push PC+1 as return address (pointing to instruction after signature byte)
        let return_pc = self.pc.wrapping_add(1);
        self.push(bus, (return_pc >> 8) as u8);
        self.push(bus, return_pc as u8);

        // Push status register with B flag set
        let status_with_break = self.status.bits() | StatusFlags::BREAK.bits();
        self.push(bus, status_with_break);

        // Set interrupt disable flag
        self.status.insert(StatusFlags::INTERRUPT_DISABLE);

        // BRK always proceeds to IRQ handler
        let low = bus.read(0xFFFE) as u16;
        let high = bus.read(0xFFFF) as u16;
        let irq_vector = (high << 8) | low;
        self.pc = irq_vector;

        7
    }

    #[inline]
    pub(in crate::cpu) fn rti(&mut self, bus: &mut dyn CpuBus) -> u8 {
        // RTI stack validation - recover via reset vector if stack is critically low
        if self.sp < 0x20 {
            let reset_low = bus.read(0xFFFC) as u16;
            let reset_high = bus.read(0xFFFD) as u16;
            self.pc = (reset_high << 8) | reset_low;
            self.sp = 0xFD;
            return 6;
        }

        // Pull status register from stack
        let status = self.pull(bus);
        // Restore status flags properly - keep UNUSED always set, clear BREAK flag
        // BREAK flag should never be restored from stack during RTI
        self.status = StatusFlags::from_bits_truncate(status & !StatusFlags::BREAK.bits())
            | StatusFlags::UNUSED;

        // Pull return address from stack
        let low = self.pull(bus) as u16;
        let high = self.pull(bus) as u16;
        let return_addr = (high << 8) | low;

        // RTI address validation - use reset vector for invalid addresses
        if return_addr == 0x0000 || return_addr == 0xFFFF {
            let reset_low = bus.read(0xFFFC) as u16;
            let reset_high = bus.read(0xFFFD) as u16;
            self.pc = (reset_high << 8) | reset_low;
        } else {
            self.pc = return_addr;
        }

        6
    }
}
