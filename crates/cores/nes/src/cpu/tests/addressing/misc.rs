use super::*;

#[test]
fn test_implied_addressing() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    // Test implied addressing (no operands)
    bus.load_program(&[0xEA], 0x8000); // NOP
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x8001);
    assert_eq!(cycles, 2);
}

#[test]
fn test_accumulator_addressing() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0x40;

    // Test accumulator addressing
    bus.load_program(&[0x0A], 0x8000); // ASL A
    cpu.pc = 0x8000;

    cpu.step(&mut bus);

    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.pc, 0x8001);
}
