use super::super::*;

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: StatusFlags::from_bits_truncate(0x24),
            cycles: 0,
            halted: false,
            rts_count: 0,
            last_rts_pc: 0,
        }
    }

    pub fn reset(&mut self, bus: &mut dyn CpuBus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = StatusFlags::from_bits_truncate(0x24);
        self.halted = false;

        bus.on_reset();
        let low = bus.read(0xFFFC) as u16;
        let high = bus.read(0xFFFD) as u16;
        self.pc = (high << 8) | low;
        self.cycles = 8;
    }

    pub fn step(&mut self, bus: &mut dyn CpuBus) -> u8 {
        if self.halted {
            self.cycles += 1;
            return 1;
        }

        let opcode = bus.read(self.pc);

        // Increment PC for most instructions - special ones handle it themselves
        self.pc = self.pc.wrapping_add(1);

        let cycles = self.execute_instruction(opcode, bus);

        // Safety check: ensure we're making progress
        if cycles == 0 {
            return 2; // Return minimum cycles to prevent infinite loop
        }

        self.cycles += cycles as u64;
        cycles
    }

    pub fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn set_halted(&mut self, halted: bool) {
        self.halted = halted;
    }

    pub fn total_cycles(&self) -> u64 {
        self.cycles
    }

    pub fn set_total_cycles(&mut self, cycles: u64) {
        self.cycles = cycles;
    }
}
