use super::*;

#[test]
fn test_zero_page_addressing() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    // Set value at zero page address
    bus.write(0x42, 0xAB);

    // LDA $42 (zero page)
    bus.load_program(&[0xA5, 0x42], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0xAB);
    assert_eq!(cycles, 3);
}

#[test]
fn test_zero_page_x_addressing() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.x = 0x10;
    bus.write(0x52, 0xCD); // 0x42 + 0x10

    // LDA $42,X
    bus.load_program(&[0xB5, 0x42], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0xCD);
    assert_eq!(cycles, 4);
}

#[test]
fn test_zero_page_x_wraparound() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.x = 0xFF;
    bus.write(0x41, 0xEF); // (0x42 + 0xFF) & 0xFF = 0x41

    // LDA $42,X
    bus.load_program(&[0xB5, 0x42], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);

    assert_eq!(cpu.a, 0xEF);
}
