use super::*;

#[test]
fn test_relative_addressing_forward() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    // Test forward branch
    bus.load_program(&[0x10, 0x0A], 0x8000); // BPL +10
    cpu.pc = 0x8000;
    cpu.status.remove(StatusFlags::NEGATIVE);

    cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x800C); // 0x8002 + 0x0A
}

#[test]
fn test_relative_addressing_backward() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    // Test backward branch (two's complement)
    bus.load_program(&[0x10, 0xFC], 0x8000); // BPL -4
    cpu.pc = 0x8000;
    cpu.status.remove(StatusFlags::NEGATIVE);

    cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x7FFE); // 0x8002 - 4
}
