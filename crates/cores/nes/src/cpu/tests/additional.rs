use super::*;

#[cfg(test)]
mod additional_cpu_tests {
    use super::*;

    #[test]
    fn test_decimal_mode_ignored() {
        // NES 6502は decimal modeを無視する（バグ）
        let (mut cpu, mut bus) = setup_cpu();
        cpu.reset(&mut bus);

        // SED (Set Decimal Mode)
        bus.load_program(&[0xF8], 0x8000);
        cpu.pc = 0x8000;
        cpu.step(&mut bus);

        // ADC should ignore decimal mode on NES
        cpu.a = 0x09;
        bus.load_program(&[0x69, 0x01], 0x8001); // ADC #$01
        cpu.step(&mut bus);

        // Should be 0x0A, not 0x10 (decimal)
        assert_eq!(cpu.a, 0x0A);
    }

    #[test]
    fn test_stack_operations_detailed() {
        let (mut cpu, mut bus) = setup_cpu();
        cpu.reset(&mut bus);

        // Stack starts at $01FF and grows downward
        cpu.sp = 0xFF;
        cpu.a = 0x42;

        // PHA
        bus.load_program(&[0x48], 0x8000);
        cpu.pc = 0x8000;
        cpu.step(&mut bus);

        assert_eq!(cpu.sp, 0xFE);
        assert_eq!(bus.read(0x01FF), 0x42);

        // Modify A
        cpu.a = 0x00;

        // PLA
        bus.load_program(&[0x68], 0x8001);
        cpu.step(&mut bus);

        assert_eq!(cpu.sp, 0xFF);
        assert_eq!(cpu.a, 0x42);
    }

    #[test]
    fn test_nmi_timing() {
        let (mut cpu, mut bus) = setup_cpu();
        cpu.reset(&mut bus);

        // NMI vector
        bus.write(0xFFFA, 0x00);
        bus.write(0xFFFB, 0x90);

        cpu.pc = 0x8000;
        cpu.status = StatusFlags::from_bits_truncate(0x24);
        cpu.sp = 0xFF;

        cpu.nmi(&mut bus);

        // NMI takes 7 cycles total
        // Check that I flag is set
        assert!(cpu.status.contains(StatusFlags::INTERRUPT_DISABLE));

        // Check stack contents
        assert_eq!(bus.read(0x01FF), 0x80); // PCH
        assert_eq!(bus.read(0x01FE), 0x00); // PCL
        assert_eq!(bus.read(0x01FD), 0x24); // Status (without B flag)
    }

    #[test]
    fn test_overflow_flag_detailed() {
        let (mut cpu, mut bus) = setup_cpu();
        cpu.reset(&mut bus);

        // Test overflow: positive + positive = negative
        cpu.a = 0x7F; // 127
        bus.load_program(&[0x69, 0x01], 0x8000); // ADC #$01
        cpu.pc = 0x8000;
        cpu.step(&mut bus);

        assert_eq!(cpu.a, 0x80); // -128 in two's complement
        assert!(cpu.status.contains(StatusFlags::OVERFLOW));
        assert!(cpu.status.contains(StatusFlags::NEGATIVE));

        // Test no overflow: positive + negative
        cpu.status.remove(StatusFlags::OVERFLOW);
        cpu.a = 0x50; // 80
        bus.load_program(&[0x69, 0x90], 0x8002); // ADC #$90 (-112)
        cpu.step(&mut bus);

        assert!(!cpu.status.contains(StatusFlags::OVERFLOW));
    }

    #[test]
    fn test_indirect_jmp_bug_detailed() {
        let (mut cpu, mut bus) = setup_cpu();
        cpu.reset(&mut bus);

        // Set up the bug scenario
        bus.write(0x30FF, 0x80); // Low byte at page boundary
        bus.write(0x3100, 0x50); // This should be high byte
        bus.write(0x3000, 0x40); // But this will be read instead

        // JMP ($30FF) - should go to $4080, not $5080
        bus.load_program(&[0x6C, 0xFF, 0x30], 0x8000);
        cpu.pc = 0x8000;
        cpu.step(&mut bus);

        assert_eq!(cpu.pc, 0x4080); // Bug behavior
    }

    #[test]
    fn test_jam_halts_cpu_until_reset() {
        let (mut cpu, mut bus) = setup_cpu();
        cpu.reset(&mut bus);

        bus.load_program(&[0x02, 0xE8], 0x8000);
        cpu.pc = 0x8000;

        let jam_cycles = cpu.step(&mut bus);
        assert_eq!(jam_cycles, 2);
        assert!(cpu.is_halted());
        assert_eq!(cpu.pc, 0x8001);

        cpu.x = 0x10;
        let halted_cycles = cpu.step(&mut bus);
        assert_eq!(halted_cycles, 1);
        assert_eq!(cpu.pc, 0x8001);
        assert_eq!(cpu.x, 0x10);

        bus.write(0xFFFA, 0x00);
        bus.write(0xFFFB, 0x90);
        assert_eq!(cpu.nmi(&mut bus), 0);
        assert_eq!(cpu.pc, 0x8001);

        cpu.reset(&mut bus);
        assert!(!cpu.is_halted());
        assert_eq!(cpu.pc, 0x8000);
    }
}
