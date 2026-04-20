use super::*;

#[test]
fn test_inx_iny() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.x = 0x10;
    cpu.y = 0x20;

    bus.load_program(&[0xE8, 0xC8], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);
    assert_eq!(cpu.x, 0x11);

    cpu.step(&mut bus);
    assert_eq!(cpu.y, 0x21);
}

#[test]
fn test_inx_wraparound() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.x = 0xFF;

    bus.load_program(&[0xE8], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);
    assert_eq!(cpu.x, 0x00);
    assert!(cpu.status.contains(StatusFlags::ZERO));
}

#[test]
fn test_adc_no_carry() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0x10;
    cpu.status.remove(StatusFlags::CARRY);

    bus.load_program(&[0x69, 0x20], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x30);
    assert!(!cpu.status.contains(StatusFlags::CARRY));
    assert!(!cpu.status.contains(StatusFlags::OVERFLOW));
}

#[test]
fn test_adc_with_carry() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0xFF;
    cpu.status.remove(StatusFlags::CARRY);

    bus.load_program(&[0x69, 0x01], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.status.contains(StatusFlags::CARRY));
    assert!(cpu.status.contains(StatusFlags::ZERO));
}

#[test]
fn test_sbc() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0x50;
    cpu.status.insert(StatusFlags::CARRY);

    bus.load_program(&[0xE9, 0x20], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x30);
    assert!(cpu.status.contains(StatusFlags::CARRY));
}

#[test]
fn test_cmp() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0x30;

    bus.load_program(&[0xC9, 0x30], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);

    assert!(cpu.status.contains(StatusFlags::CARRY));
    assert!(cpu.status.contains(StatusFlags::ZERO));
    assert!(!cpu.status.contains(StatusFlags::NEGATIVE));
}
