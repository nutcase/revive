use super::*;

#[test]
fn test_indexed_indirect_x() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.x = 0x04;
    // Set pointer at ($40,X) = $44
    bus.write(0x44, 0x00);
    bus.write(0x45, 0x20);
    // Set value at $2000
    bus.write(0x2000, 0xDE);

    // LDA ($40,X)
    bus.load_program(&[0xA1, 0x40], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0xDE);
    assert_eq!(cycles, 6);
}

#[test]
fn test_indexed_indirect_x_wraparound() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.x = 0xFF;
    // ($FF,X) = ($FF + $FF) & $FF = $FE
    bus.write(0xFE, 0x00);
    bus.write(0xFF, 0x30);
    bus.write(0x3000, 0xAA);

    // LDA ($FF,X)
    bus.load_program(&[0xA1, 0xFF], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);

    assert_eq!(cpu.a, 0xAA);
}

#[test]
fn test_indirect_indexed_y() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.y = 0x10;
    // Set pointer at $40
    bus.write(0x40, 0x00);
    bus.write(0x41, 0x20);
    // Value at $2000 + Y = $2010
    bus.write(0x2010, 0xF0);

    // LDA ($40),Y
    bus.load_program(&[0xB1, 0x40], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0xF0);
    assert_eq!(cycles, 5); // No page cross
}

#[test]
fn test_indirect_indexed_y_page_cross() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.y = 0xFF;
    // Set pointer at $40
    bus.write(0x40, 0x01);
    bus.write(0x41, 0x20);
    // Value at $2001 + Y = $2100 (page cross)
    bus.write(0x2100, 0x12);

    // LDA ($40),Y
    bus.load_program(&[0xB1, 0x40], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0x12);
    assert_eq!(cycles, 6); // Page cross penalty
}

#[test]
fn test_jmp_indirect() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    // Set indirect address
    bus.write(0x2000, 0x34);
    bus.write(0x2001, 0x12);

    // JMP ($2000)
    bus.load_program(&[0x6C, 0x00, 0x20], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x1234);
    assert_eq!(cycles, 5);
}

#[test]
fn test_jmp_indirect_bug() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    // Test the 6502 JMP indirect bug at page boundary
    bus.write(0x20FF, 0x34);
    bus.write(0x2000, 0x12); // Bug: should read from 0x2100, but reads from 0x2000

    // JMP ($20FF)
    bus.load_program(&[0x6C, 0xFF, 0x20], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x1234);
}
