use super::*;

#[test]
fn test_bit_instruction() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0x0F;
    bus.write(0x10, 0xF0);

    bus.load_program(&[0x24, 0x10], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);

    assert!(cpu.status.contains(StatusFlags::ZERO));
    assert!(cpu.status.contains(StatusFlags::NEGATIVE));
    assert!(cpu.status.contains(StatusFlags::OVERFLOW));
}

#[test]
fn test_and_or_eor() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0xFF;
    bus.load_program(&[0x29, 0x0F], 0x8000);
    cpu.pc = 0x8000;
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x0F);

    bus.load_program(&[0x09, 0xF0], 0x8002);
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0xFF);

    bus.load_program(&[0x49, 0xFF], 0x8004);
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.status.contains(StatusFlags::ZERO));
}

#[test]
fn test_shift_operations() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0x81;
    bus.load_program(&[0x0A], 0x8000);
    cpu.pc = 0x8000;
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x02);
    assert!(cpu.status.contains(StatusFlags::CARRY));

    cpu.a = 0x81;
    bus.load_program(&[0x4A], 0x8001);
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x40);
    assert!(cpu.status.contains(StatusFlags::CARRY));

    cpu.a = 0x80;
    cpu.status.insert(StatusFlags::CARRY);
    bus.load_program(&[0x2A], 0x8002);
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x01);
    assert!(cpu.status.contains(StatusFlags::CARRY));

    cpu.a = 0x01;
    cpu.status.insert(StatusFlags::CARRY);
    bus.load_program(&[0x6A], 0x8003);
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x80);
    assert!(cpu.status.contains(StatusFlags::CARRY));
}
