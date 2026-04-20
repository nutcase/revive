use snes_emulator::cpu::bus::CpuBus;
use snes_emulator::cpu::{Cpu, StatusFlags};

struct MiniBus {
    mem: [u8; 0x10000],
}

impl MiniBus {
    fn new() -> Self {
        Self { mem: [0; 0x10000] }
    }
}

impl CpuBus for MiniBus {
    fn read_u8(&mut self, addr: u32) -> u8 {
        self.mem[(addr as usize) & 0xFFFF]
    }

    fn write_u8(&mut self, addr: u32, value: u8) {
        self.mem[(addr as usize) & 0xFFFF] = value;
    }

    fn poll_irq(&mut self) -> bool {
        false
    }
}

#[test]
fn cpu_executes_basic_program_and_stores_result() {
    let mut bus = MiniBus::new();
    let start = 0x8000usize;
    // CLD; CLC; LDA #$34; ADC #$01; STA $2000
    let prog = [0xD8u8, 0x18, 0xA9, 0x34, 0x69, 0x01, 0x8D, 0x00, 0x20];
    bus.mem[start..start + prog.len()].copy_from_slice(&prog);

    let mut cpu = Cpu::new();
    cpu.reset(start as u16);

    // Execute 5 instructions
    for _ in 0..5 {
        let _ = cpu.step_with_bus(&mut bus);
    }

    assert_eq!(bus.mem[0x2000], 0x35);
    assert_eq!(cpu.a() & 0x00FF, 0x35);
    assert!(!cpu.get_flag(StatusFlags::ZERO));
    assert!(!cpu.get_flag(StatusFlags::NEGATIVE));
}
