use super::*;

#[test]
fn test_jmp_absolute() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.load_program(&[0x4C, 0x34, 0x12], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x1234);
    assert_eq!(cycles, 3);
}

#[test]
fn test_jsr_rts() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.load_program(&[0x20, 0x00, 0x90], 0x8000);
    bus.load_program(&[0x60], 0x9000);
    cpu.pc = 0x8000;
    cpu.sp = 0xFF;

    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x9000);
    assert_eq!(cpu.sp, 0xFD);
    assert_eq!(bus.read(0x01FF), 0x80);
    assert_eq!(bus.read(0x01FE), 0x02);

    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8003);
    assert_eq!(cpu.sp, 0xFF);
}

#[test]
#[cfg(not(feature = "rom-speed-hacks"))]
fn test_jsr_known_wait_loop_uses_normal_cycles_by_default() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.load_program(&[0x20, 0x95, 0x89], 0x8974);
    cpu.pc = 0x8974;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x8995);
    assert_eq!(cycles, 6);
}

#[test]
#[cfg(feature = "rom-speed-hacks")]
fn test_jsr_known_wait_loop_speed_hack_is_opt_in() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    bus.load_program(&[0x20, 0x95, 0x89], 0x8974);
    cpu.pc = 0x8974;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x8995);
    assert_eq!(cycles, 2);
}

#[test]
fn test_beq_taken() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.status.insert(StatusFlags::ZERO);

    bus.load_program(&[0xF0, 0x10], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x8012);
    assert_eq!(cycles, 3);
}

#[test]
fn test_beq_not_taken() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.status.remove(StatusFlags::ZERO);

    bus.load_program(&[0xF0, 0x10], 0x8000);
    cpu.pc = 0x8000;

    let cycles = cpu.step(&mut bus);

    assert_eq!(cpu.pc, 0x8002);
    assert_eq!(cycles, 2);
}

#[test]
fn test_stack_operations() {
    let (mut cpu, mut bus) = setup_cpu();
    cpu.reset(&mut bus);

    cpu.a = 0x42;
    cpu.sp = 0xFF;

    bus.load_program(&[0x48, 0x68], 0x8000);
    cpu.pc = 0x8000;

    cpu.step(&mut bus);
    assert_eq!(cpu.sp, 0xFE);
    assert_eq!(bus.read(0x01FF), 0x42);

    cpu.a = 0x00;

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.sp, 0xFF);
}

#[test]
fn test_nmi_interrupt() {
    let (mut cpu, mut bus) = setup_cpu();

    bus.write(0xFFFA, 0x00);
    bus.write(0xFFFB, 0x90);

    cpu.reset(&mut bus);
    cpu.pc = 0x8000;
    cpu.status = StatusFlags::from_bits_truncate(0x24);
    cpu.sp = 0xFF;

    cpu.nmi(&mut bus);

    assert_eq!(cpu.pc, 0x9000);
    assert_eq!(bus.read(0x01FF), 0x80);
    assert_eq!(bus.read(0x01FE), 0x00);
    assert_eq!(bus.read(0x01FD), 0x24);
    assert_eq!(cpu.sp, 0xFC);
    assert!(cpu.status.contains(StatusFlags::INTERRUPT_DISABLE));
}
