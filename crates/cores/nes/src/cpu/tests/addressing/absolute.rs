use super::*;

#[test]
fn test_absolute_addressing() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.write(0x1234, 0x56);

    // LDA $1234
    bus.load_program(&[0xAD, 0x34, 0x12], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0x56);
    assert_eq!(cycles, 4);
}

#[test]
fn test_absolute_x_addressing() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.x = 0x10;
    bus.write(0x1244, 0x78); // 0x1234 + 0x10

    // LDA $1234,X
    bus.load_program(&[0xBD, 0x34, 0x12], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0x78);
    assert_eq!(cycles, 4); // No page cross
}

#[test]
fn test_absolute_x_page_cross() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.x = 0xFF;
    bus.write(0x1333, 0x9A); // 0x1234 + 0xFF = 0x1333 (page cross)

    // LDA $1234,X
    bus.load_program(&[0xBD, 0x34, 0x12], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0x9A);
    assert_eq!(cycles, 5); // Page cross penalty
}

#[test]
fn test_absolute_y_addressing() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.y = 0x20;
    bus.write(0x1254, 0xBC); // 0x1234 + 0x20

    // LDA $1234,Y
    bus.load_program(&[0xB9, 0x34, 0x12], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0xBC);
    assert_eq!(cycles, 4);
}
