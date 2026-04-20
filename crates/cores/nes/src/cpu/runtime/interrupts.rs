use super::super::*;

impl Cpu {
    pub fn nmi(&mut self, bus: &mut dyn CpuBus) -> u8 {
        if self.halted {
            return 0;
        }

        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, self.pc as u8);
        self.push(bus, self.status.bits() & !StatusFlags::BREAK.bits());

        self.status.insert(StatusFlags::INTERRUPT_DISABLE);

        let low = bus.read(0xFFFA) as u16;
        let high = bus.read(0xFFFB) as u16;
        let nmi_vector = (high << 8) | low;
        self.pc = nmi_vector;

        self.cycles += 7;
        7
    }

    pub fn irq(&mut self, bus: &mut dyn CpuBus) -> u8 {
        if self.halted {
            return 0;
        }

        // IRQ is maskable - check interrupt disable flag
        if self.status.contains(StatusFlags::INTERRUPT_DISABLE) {
            return 0;
        }

        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, self.pc as u8);
        self.push(bus, self.status.bits() & !StatusFlags::BREAK.bits());

        self.status.insert(StatusFlags::INTERRUPT_DISABLE);

        // IRQ vector at $FFFE-$FFFF
        let low = bus.read(0xFFFE) as u16;
        let high = bus.read(0xFFFF) as u16;
        let irq_vector = (high << 8) | low;

        self.pc = irq_vector;

        self.cycles += 7;
        7
    }
}
