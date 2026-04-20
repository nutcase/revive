use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn jmp_absolute(&mut self, bus: &mut dyn CpuBus) -> u8 {
        // JMP absolute: read target address and jump directly
        self.pc = self.read_word(bus);
        3
    }

    #[inline]
    pub(in crate::cpu) fn jsr(&mut self, bus: &mut dyn CpuBus) -> u8 {
        // JSR: step() already incremented PC, so we're at opcode+1
        let addr = self.read_word(bus); // This reads 2 bytes and increments PC by 2
        let return_addr = self.pc.wrapping_sub(1); // PC is now at opcode+3, return to opcode+2

        #[cfg(feature = "rom-speed-hacks")]
        {
            // Opt-in compatibility hack for a known busy wait. Default builds
            // keep normal 6502 timing.
            if addr == 0x8995 && return_addr == 0x8976 {
                self.rts_count += 1;
                self.push(bus, (return_addr >> 8) as u8);
                self.push(bus, return_addr as u8);
                self.pc = addr;
                return 2;
            }
        }

        self.push(bus, (return_addr >> 8) as u8);
        self.push(bus, return_addr as u8);
        self.pc = addr;
        6
    }

    #[inline]
    pub(in crate::cpu) fn rts(&mut self, bus: &mut dyn CpuBus) -> u8 {
        // RTS handles PC completely by itself
        let old_pc = self.pc;
        let _old_sp = self.sp;
        let low = self.pull(bus) as u16;
        let high = self.pull(bus) as u16;
        let new_pc = ((high << 8) | low).wrapping_add(1);

        // Check for infinite RTS loop - but be more tolerant
        if old_pc == self.last_rts_pc {
            self.rts_count += 1;
            if self.rts_count == 20 {
                // Frequent RTS loop detected
                self.rts_count = 0;
            }
        } else {
            self.rts_count = 0;
        }
        self.last_rts_pc = old_pc;

        // RTS completed

        self.pc = new_pc;
        6
    }

    #[inline]
    pub(in crate::cpu) fn jmp_indirect(&mut self, bus: &mut dyn CpuBus) -> u8 {
        // JMP indirect: read address, then read target from that address
        let addr = self.read_word(bus);
        // 6502 bug: if addr is 0x??FF, high byte is read from 0x??00 instead of 0x?+1?00
        let low = bus.read(addr) as u16;
        let high = if addr & 0xFF == 0xFF {
            bus.read(addr & 0xFF00) as u16
        } else {
            bus.read(addr + 1) as u16
        };
        let target = (high << 8) | low;
        self.pc = target;
        5
    }
}
