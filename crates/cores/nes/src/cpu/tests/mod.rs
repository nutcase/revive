use super::*;

mod additional;
mod addressing;
mod arithmetic;
mod control;
mod load_store;
mod logic_shift;

struct TestBus {
    memory: [u8; 0x10000],
}

impl TestBus {
    fn new() -> Self {
        Self {
            memory: [0; 0x10000],
        }
    }

    fn load_program(&mut self, program: &[u8], start_addr: u16) {
        for (i, &byte) in program.iter().enumerate() {
            self.memory[start_addr as usize + i] = byte;
        }
    }
}

impl CpuBus for TestBus {
    fn on_reset(&mut self) {}

    fn read(&mut self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}

fn setup_cpu() -> (Cpu, TestBus) {
    let cpu = Cpu::new();
    let mut bus = TestBus::new();
    bus.write(0xFFFC, 0x00);
    bus.write(0xFFFD, 0x80);
    (cpu, bus)
}
