use super::*;

#[test]
fn test_lda_immediate() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.load_program(&[0xA9, 0x42], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.pc, 0x8002);
    assert_eq!(cycles, 2);
    assert!(!cpu.status.contains(StatusFlags::ZERO));
    assert!(!cpu.status.contains(StatusFlags::NEGATIVE));
}

#[test]
fn test_lda_zero_flag() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.load_program(&[0xA9, 0x00], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);

    assert_eq!(cpu.a, 0x00);
    assert!(cpu.status.contains(StatusFlags::ZERO));
    assert!(!cpu.status.contains(StatusFlags::NEGATIVE));
}

#[test]
fn test_lda_negative_flag() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.load_program(&[0xA9, 0x80], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);

    assert_eq!(cpu.a, 0x80);
    assert!(!cpu.status.contains(StatusFlags::ZERO));
    assert!(cpu.status.contains(StatusFlags::NEGATIVE));
}

#[test]
fn test_sta_zero_page() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0x42;
    bus.load_program(&[0x85, 0x10], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(bus.read(0x0010), 0x42);
    assert_eq!(cpu.pc, 0x8002);
    assert_eq!(cycles, 3);
}

#[test]
fn test_ldx_ldy() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.load_program(&[0xA2, 0x10, 0xA0, 0x20], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);
    assert_eq!(cpu.x, 0x10);

    cpu.step(&mut bus);
    assert_eq!(cpu.y, 0x20);
}
