use super::*;
use crate::bus::{IRQ_REQUEST_IRQ1, IRQ_REQUEST_IRQ2, IRQ_REQUEST_TIMER, PAGE_SIZE};

fn setup_cpu_with_program(program: &[u8]) -> (Cpu, Bus) {
    let mut bus = Bus::new();
    bus.load(0x8000, program);
    bus.write_u16(0xFFFC, 0x8000);

    let mut cpu = Cpu::new();
    cpu.reset(&mut bus);
    (cpu, bus)
}

fn block_transfer_program(opcode: u8, source: u16, dest: u16, length: u16) -> [u8; 7] {
    [
        opcode,
        (source & 0x00FF) as u8,
        (source >> 8) as u8,
        (dest & 0x00FF) as u8,
        (dest >> 8) as u8,
        (length & 0x00FF) as u8,
        (length >> 8) as u8,
    ]
}

#[test]
fn opcode_cycle_table_covers_implemented_dispatch_set() {
    let implemented = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1C, 0x1D, 0x1E,
        0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D,
        0x2E, 0x2F, 0x30, 0x31, 0x32, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3C, 0x3D, 0x3E,
        0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4C, 0x4D, 0x4E,
        0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5D, 0x5E, 0x5F,
        0x60, 0x61, 0x62, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x6C, 0x6D, 0x6E, 0x6F, 0x70,
        0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F,
        0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x8C, 0x8D, 0x8E, 0x8F,
        0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9C, 0x9D, 0x9E, 0x9F,
        0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAC, 0xAD, 0xAE, 0xAF,
        0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBC, 0xBD, 0xBE, 0xBF,
        0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE,
        0xCF, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDD, 0xDE,
        0xDF, 0xE0, 0xE1, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE,
        0xEF, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFD, 0xFE, 0xFF,
        0x1B, 0x33, 0x3B, 0x4B, 0x5B, 0x5C, 0x63, 0x6B, 0x8B, 0x9B, 0xAB, 0xBB, 0xDC, 0xE2, 0xFB,
        0xFC,
    ];
    assert_eq!(implemented.len(), 256);

    let mut implemented_mask = [false; 256];
    for opcode in implemented {
        implemented_mask[opcode as usize] = true;
        assert_ne!(
            Cpu::opcode_base_cycles(opcode),
            0,
            "implemented opcode {:02X} has zero cycle entry",
            opcode
        );
    }

    for opcode in 0u8..=u8::MAX {
        let cycles = Cpu::opcode_base_cycles(opcode);
        if implemented_mask[opcode as usize] {
            assert_ne!(cycles, 0, "missing cycle entry for opcode {:02X}", opcode);
        } else {
            assert_eq!(
                cycles, 0,
                "unexpected cycle entry for opcode {:02X}",
                opcode
            );
        }
    }

    for (opcode, expected) in [(0xA9, 2), (0xB1, 7), (0x7B, 8), (0x44, 8), (0x73, 17)] {
        assert_eq!(Cpu::opcode_base_cycles(opcode), expected);
    }
}

#[test]
fn undefined_opcodes_behave_as_nops() {
    let program = [
        0x1B, 0x33, 0x3B, 0x4B, 0x5B, 0x5C, 0x63, 0x6B, 0x8B, 0x9B, 0xAB, 0xBB, 0xDC, 0xE2, 0xFB,
        0xFC, 0x00,
    ];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x12;
    cpu.x = 0x34;
    cpu.y = 0x56;
    cpu.sp = 0xEF;
    cpu.status = FLAG_CARRY | FLAG_OVERFLOW;
    let start_pc = cpu.pc;

    for i in 0..16u16 {
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 2, "opcode at index {i} should be 2-cycle NOP");
        assert_eq!(cpu.pc, start_pc + i + 1);
        assert_eq!(cpu.a, 0x12);
        assert_eq!(cpu.x, 0x34);
        assert_eq!(cpu.y, 0x56);
        assert_eq!(cpu.sp, 0xEF);
        assert_eq!(cpu.status, FLAG_CARRY | FLAG_OVERFLOW);
        assert!(!cpu.halted);
    }
}

#[test]
fn cycle_timing_reference_subset() {
    {
        let program = [0xA9, 0x01, 0x00]; // LDA #$01
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 2, "LDA #imm should take 2 cycles");
    }
    {
        let program = [0xA5, 0x10, 0x00]; // LDA $10
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x55);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4, "LDA zp should take 4 cycles");
    }
    {
        let program = [0xAD, 0x34, 0x12, 0x00]; // LDA $1234
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x1234, 0x77);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 5, "LDA abs should take 5 cycles");
    }
    {
        let program = [0x24, 0x10, 0x00]; // BIT $10
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.a = 0x0F;
        bus.write(0x0010, 0xF0);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4, "BIT zp should take 4 cycles");
    }
    {
        let program = [0x66, 0x10, 0x00]; // ROR $10
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x0010, 0x80);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 6, "ROR zp should take 6 cycles");
    }
    {
        let program = [0xD0, 0x02, 0x00]; // BNE +2
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.set_flag(FLAG_ZERO, true);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 2, "BNE not-taken should take 2 cycles");
    }
    {
        let program = [0xD0, 0x02, 0x00, 0x00]; // BNE +2
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        cpu.set_flag(FLAG_ZERO, false);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4, "BNE taken should take 4 cycles");
    }
    {
        let program = [0x93, 0x0F, 0x34, 0x12, 0x00]; // TST #$0F, $1234
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        bus.write(0x1234, 0xF0);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8, "TST abs should take 8 cycles");
    }
    {
        let program = block_transfer_program(0x73, 0x9000, 0x4000, 0x0003); // TII len=3
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        for i in 0..4u16 {
            bus.write(0x9000 + i, 0x10 + i as u8);
        }
        let cycles = cpu.step(&mut bus);
        assert_eq!(
            cycles,
            17 + 6 * 3u32,
            "TII length=3 should report full transfer cycles"
        );
    }
}

#[test]
fn vdc_accesses_add_one_cpu_cycle() {
    let program = [0xAD, 0x00, 0x00, 0x00]; // LDA $0000 (VDC status when MPR0=$FF)
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.set_mpr(0, 0xFF);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 6, "VDC access should add one extra cycle");
}

#[test]
fn vce_accesses_add_one_cpu_cycle() {
    let program = [0x8D, 0x04, 0x04, 0x00]; // STA $0404 (VCE data low when MPR0=$FF)
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.set_mpr(0, 0xFF);
    cpu.a = 0x56;

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 6, "VCE access should add one extra cycle");
    assert_eq!(bus.vce_palette_word(0), 0x0056);
}

#[test]
fn timer_access_does_not_take_vdc_vce_extra_cycle() {
    let program = [0xAD, 0x00, 0x0C, 0x00]; // LDA $0C00 (timer counter when MPR0=$FF)
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.set_mpr(0, 0xFF);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 5, "timer access should keep base timing");
}

#[test]
fn opcode_base_cycles_reference_groups() {
    // 2-cycle group
    assert_eq!(Cpu::opcode_base_cycles(0xEA), 2); // NOP
    assert_eq!(Cpu::opcode_base_cycles(0x80), 2); // BRA base

    // 3-cycle group
    assert_eq!(Cpu::opcode_base_cycles(0x48), 3); // PHA
    assert_eq!(Cpu::opcode_base_cycles(0xDA), 3); // PHX

    // 4-cycle group
    assert_eq!(Cpu::opcode_base_cycles(0xA5), 4); // LDA zp
    assert_eq!(Cpu::opcode_base_cycles(0x4C), 4); // JMP abs

    // 5-cycle group
    assert_eq!(Cpu::opcode_base_cycles(0xAD), 5); // LDA abs
    assert_eq!(Cpu::opcode_base_cycles(0x9D), 5); // STA abs,X

    // 6-cycle group
    assert_eq!(Cpu::opcode_base_cycles(0x66), 6); // ROR zp
    assert_eq!(Cpu::opcode_base_cycles(0x0F), 6); // BBR0 zp,rel (not-taken base)

    // 7-cycle group
    assert_eq!(Cpu::opcode_base_cycles(0x6C), 7); // JMP (abs)
    assert_eq!(Cpu::opcode_base_cycles(0x7C), 7); // JMP (abs,X)
    assert_eq!(Cpu::opcode_base_cycles(0x20), 7); // JSR abs
    assert_eq!(Cpu::opcode_base_cycles(0x60), 7); // RTS
    assert_eq!(Cpu::opcode_base_cycles(0x40), 7); // RTI

    // 8-cycle group
    assert_eq!(Cpu::opcode_base_cycles(0x00), 8); // BRK
    assert_eq!(Cpu::opcode_base_cycles(0x83), 8); // TST zp
}

#[test]
fn adc_handles_carry_and_overflow() {
    let program = [0x69, 0x01, 0x69, 0x80, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x7F;

    cpu.step(&mut bus); // ADC #$01 => 0x80
    assert_eq!(cpu.a, 0x80);
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(cpu.flag(FLAG_OVERFLOW));

    cpu.step(&mut bus); // ADC #$80 => 0x00 with carry
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.flag(FLAG_CARRY));
}

#[test]
fn adc_decimal_mode_adds_bcd_values() {
    let program = [0xF8, 0x69, 0x34, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x12;
    cpu.set_flag(FLAG_CARRY, false);

    cpu.step(&mut bus); // SED
    cpu.step(&mut bus); // ADC #$34
    assert_eq!(cpu.a, 0x46);
    assert!(!cpu.flag(FLAG_CARRY));
}

#[test]
fn adc_decimal_mode_handles_digit_carry() {
    let program = [0xF8, 0x69, 0x27, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x19;
    cpu.set_flag(FLAG_CARRY, false);

    cpu.step(&mut bus); // SED
    cpu.step(&mut bus); // ADC #$27
    assert_eq!(cpu.a, 0x46);
    assert!(!cpu.flag(FLAG_CARRY));
}

#[test]
fn adc_decimal_mode_uses_input_carry() {
    let program = [0xF8, 0x69, 0x00, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x99;
    cpu.set_flag(FLAG_CARRY, true);

    cpu.step(&mut bus); // SED
    cpu.step(&mut bus); // ADC #$00 with carry-in
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.flag(FLAG_CARRY));
    assert!(cpu.flag(FLAG_ZERO));
}

#[test]
fn sbc_decimal_mode_subtracts_bcd_values() {
    let program = [0xF8, 0xE9, 0x29, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x50;
    cpu.set_flag(FLAG_CARRY, true);

    cpu.step(&mut bus); // SED
    cpu.step(&mut bus); // SBC #$29
    assert_eq!(cpu.a, 0x21);
    assert!(cpu.flag(FLAG_CARRY));
}

#[test]
fn sbc_decimal_mode_handles_borrow() {
    let program = [0xF8, 0xE9, 0x01, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x00;
    cpu.set_flag(FLAG_CARRY, true);

    cpu.step(&mut bus); // SED
    cpu.step(&mut bus); // SBC #$01
    assert_eq!(cpu.a, 0x99);
    assert!(!cpu.flag(FLAG_CARRY));
}

#[test]
fn branch_taken_adds_cycles_and_adjusts_pc() {
    // BNE +2 to skip BRK, then immediate BRK to halt.
    let program = [0xD0, 0x02, 0x00, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    cpu.status &= !FLAG_ZERO;
    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 4); // HuC6280: branch taken = base 2 + 2 penalty
    assert_eq!(cpu.pc, 0x8004);
}

#[test]
fn jsr_and_rts_round_trip() {
    // JSR $8004 ; LDA #$42 ; RTS ; BRK
    let program = [0x20, 0x04, 0x80, 0x00, 0xA9, 0x42, 0x60, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    cpu.step(&mut bus); // JSR
    assert_eq!(cpu.pc, 0x8004);
    assert_eq!(bus.read(0x01FC), 0x02);
    assert_eq!(bus.read(0x01FD), 0x80);
    cpu.step(&mut bus); // LDA
    assert_eq!(cpu.a, 0x42);
    cpu.step(&mut bus); // RTS
    assert_eq!(cpu.pc, 0x8003); // return to byte after JSR operand
}

#[test]
fn lda_indexed_indirect_x_reads_correct_value() {
    let program = [0xA1, 0x10, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.x = 0x05;
    bus.write(0x0015, 0x00);
    bus.write(0x0016, 0x90);
    bus.write(0x9000, 0xAB);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cpu.a, 0xAB);
    assert_eq!(cycles, 7);
}

#[test]
fn lda_indirect_y_page_cross_adds_cycle() {
    // HuC6280: (ind),Y is always 7 cycles, no page-crossing penalty
    let program = [0xB1, 0x20, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0020, 0xFF);
    bus.write(0x0021, 0x80);
    bus.write(0x8100, 0x34);
    cpu.y = 0x01;

    let cycles = cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x34);
    assert_eq!(cycles, 7); // no extra cycle for page cross on HuC6280
}

#[test]
fn sta_indirect_y_stores_value() {
    let program = [0x91, 0x30, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x77;
    cpu.y = 0x05;
    bus.write(0x0030, 0x00);
    bus.write(0x0031, 0x44);

    let cycles = cpu.step(&mut bus);
    assert_eq!(bus.read(0x4405), 0x77);
    assert_eq!(cycles, 7);
}

#[test]
fn zero_page_indirect_logic_adc_cmp_opcodes_work() {
    let program = [
        0xA9, 0x10, // LDA #$10
        0x12, 0x20, // ORA ($20) -> 0x13
        0x32, 0x22, // AND ($22) -> 0x03
        0x52, 0x24, // EOR ($24) -> 0xFC
        0x18, // CLC
        0x72, 0x26, // ADC ($26) -> 0xFD
        0xD2, 0x28, // CMP ($28) -> equal
        0x00,
    ];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.set_flag(FLAG_T, false);
    bus.write(0x0020, 0x00);
    bus.write(0x0021, 0x90);
    bus.write(0x0022, 0x01);
    bus.write(0x0023, 0x90);
    bus.write(0x0024, 0x02);
    bus.write(0x0025, 0x90);
    bus.write(0x0026, 0x03);
    bus.write(0x0027, 0x90);
    bus.write(0x0028, 0x04);
    bus.write(0x0029, 0x90);
    bus.write(0x9000, 0x03);
    bus.write(0x9001, 0x0F);
    bus.write(0x9002, 0xFF);
    bus.write(0x9003, 0x01);
    bus.write(0x9004, 0xFD);

    cpu.step(&mut bus); // LDA
    let cycles = cpu.step(&mut bus); // ORA (zp)
    assert_eq!(cycles, 7);
    assert_eq!(cpu.a, 0x13);

    let cycles = cpu.step(&mut bus); // AND (zp)
    assert_eq!(cycles, 7);
    assert_eq!(cpu.a, 0x03);

    let cycles = cpu.step(&mut bus); // EOR (zp)
    assert_eq!(cycles, 7);
    assert_eq!(cpu.a, 0xFC);

    cpu.step(&mut bus); // CLC
    let cycles = cpu.step(&mut bus); // ADC (zp)
    assert_eq!(cycles, 7);
    assert_eq!(cpu.a, 0xFD);
    assert!(!cpu.flag(FLAG_CARRY));

    let cycles = cpu.step(&mut bus); // CMP (zp)
    assert_eq!(cycles, 7);
    assert!(cpu.flag(FLAG_ZERO));
    assert!(cpu.flag(FLAG_CARRY));
}

#[test]
fn bit_immediate_updates_flags_without_touching_accumulator() {
    let program = [0x89, 0xC0, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0xFF;

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 2);
    assert_eq!(cpu.a, 0xFF);
    assert!(!cpu.flag(FLAG_ZERO));
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(cpu.flag(FLAG_OVERFLOW));
}

#[test]
fn bit_zeropage_sets_zero_when_mask_clears_bits() {
    let program = [0x24, 0x40, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x10;
    bus.write(0x0040, 0x04);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 4); // HuC6280: ZP read is 4 cycles
    assert!(cpu.flag(FLAG_ZERO));
    assert!(!cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_OVERFLOW));
}

#[test]
fn anc_immediate_updates_carry_from_sign_bit() {
    let program = [0xA9, 0xFF, 0x2B, 0x80, 0x0B, 0x01, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    cpu.step(&mut bus); // LDA #$FF
    let cycles = cpu.step(&mut bus); // ANC #$80
    assert_eq!(cycles, 2);
    assert_eq!(cpu.a, 0x80);
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(cpu.flag(FLAG_CARRY));
    assert!(!cpu.flag(FLAG_ZERO));

    let cycles = cpu.step(&mut bus); // ANC #$01 via 0x0B alias
    assert_eq!(cycles, 2);
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.flag(FLAG_ZERO));
    assert!(!cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_CARRY));
}

#[test]
fn asl_accumulator_sets_carry() {
    let program = [0x0A, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x81;

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 2);
    assert_eq!(cpu.a, 0x02);
    assert!(cpu.flag(FLAG_CARRY));
    assert!(!cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_ZERO));
}

#[test]
fn ror_zeropage_rotates_through_carry() {
    let program = [0x66, 0x10, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.status |= FLAG_CARRY;
    bus.write(0x0010, 0x02);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 6); // HuC6280: ZP RMW is 6 cycles
    assert_eq!(bus.read(0x0010), 0x81);
    assert!(!cpu.flag(FLAG_CARRY));
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_ZERO));
}

#[test]
fn rra_absolute_y_rotates_and_adds() {
    let program = [0x7B, 0x00, 0x90, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x10;
    cpu.y = 0x05;
    cpu.status |= FLAG_CARRY;
    bus.write(0x9005, 0x04);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(bus.read(0x9005), 0x82);
    assert_eq!(cpu.a, 0x92);
    assert!(!cpu.flag(FLAG_CARRY));
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_ZERO));
}

#[test]
fn pha_pla_round_trip() {
    let program = [0xA9, 0x12, 0x48, 0xA9, 0x00, 0x68, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    cpu.step(&mut bus); // LDA #$12
    assert_eq!(cpu.a, 0x12);

    cpu.step(&mut bus); // PHA
    assert_eq!(bus.read(0x01FD), 0x12);
    assert_eq!(cpu.sp, 0xFC);

    cpu.step(&mut bus); // LDA #$00
    assert_eq!(cpu.a, 0x00);

    cpu.step(&mut bus); // PLA
    assert_eq!(cpu.a, 0x12);
    assert_eq!(cpu.sp, 0xFD);
    assert!(!cpu.flag(FLAG_ZERO));
    assert!(!cpu.flag(FLAG_NEGATIVE));
}

#[test]
fn php_pushes_status_with_break_bit() {
    let program = [0x08, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.status = FLAG_CARRY;

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 3);
    assert_eq!(cpu.sp, 0xFC);
    let pushed = bus.read(0x01FD);
    assert_eq!(pushed, FLAG_CARRY | FLAG_BREAK);
}

#[test]
fn plp_restores_flags_from_stack() {
    let program = [0x28, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.push_byte(&mut bus, FLAG_NEGATIVE | FLAG_CARRY);
    cpu.status = 0;

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 4);
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(cpu.flag(FLAG_CARRY));
    // T is cleared by step() before PLP executes, and PLP preserves
    // the current T (not the stack value), so T remains false.
    assert!(!cpu.flag(FLAG_T));
}

#[test]
fn plp_preserves_break_and_t_bits() {
    let program = [0x28, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    // Stack value has B and T set, but current status has neither
    cpu.push_byte(&mut bus, FLAG_BREAK | FLAG_T | FLAG_ZERO);
    cpu.status = 0;

    cpu.step(&mut bus);
    assert!(cpu.flag(FLAG_ZERO), "Z should be restored from stack");
    assert!(
        !cpu.flag(FLAG_BREAK),
        "B (bit 4) should be preserved from current status, not stack"
    );
    assert!(
        !cpu.flag(FLAG_T),
        "T (bit 5) should be preserved from current status, not stack"
    );
}

#[test]
fn stz_zeroes_memory_without_touching_a() {
    let program = [0xA9, 0xFF, 0x64, 0x10, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    cpu.step(&mut bus); // LDA #$FF
    cpu.step(&mut bus); // STZ $10

    assert_eq!(cpu.a, 0xFF);
    assert_eq!(bus.read(0x0010), 0x00);
}

#[test]
fn tsb_sets_bits_and_updates_zero_flag() {
    let program = [0xA9, 0x0F, 0x04, 0x20, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0020, 0xF3);

    cpu.step(&mut bus); // LDA #$0F
    cpu.step(&mut bus); // TSB $20

    assert_eq!(bus.read(0x0020), 0xFF);
    assert!(!cpu.flag(FLAG_ZERO));
}

#[test]
fn trb_sets_zero_flag_when_no_overlap() {
    let program = [0xA9, 0xF0, 0x14, 0x30, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0030, 0x0F);

    cpu.step(&mut bus); // LDA #$F0
    cpu.step(&mut bus); // TRB $30 (no overlap)
    assert_eq!(bus.read(0x0030), 0x0F);
    assert!(cpu.flag(FLAG_ZERO));
}

#[test]
fn trb_clears_bits_when_overlap_exists() {
    let program = [0xA9, 0xF0, 0x14, 0x30, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0030, 0xF3);

    cpu.step(&mut bus); // LDA #$F0
    cpu.step(&mut bus); // TRB $30 (overlap)

    assert_eq!(bus.read(0x0030), 0x03);
    assert!(!cpu.flag(FLAG_ZERO));
}

#[test]
fn tii_transfers_incrementing_addresses() {
    let program = [0x73, 0x00, 0x90, 0x00, 0x40, 0x03, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x9000, 0x11);
    bus.write(0x9001, 0x22);
    bus.write(0x9002, 0x33);

    cpu.step(&mut bus);

    assert_eq!(bus.read(0x4000), 0x11);
    assert_eq!(bus.read(0x4001), 0x22);
    assert_eq!(bus.read(0x4002), 0x33);
    assert_eq!(cpu.sp, 0xFD);
}

#[test]
fn tin_leaves_destination_fixed() {
    let program = [0xD3, 0x00, 0x90, 0x00, 0x40, 0x02, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x9000, 0xAA);
    bus.write(0x9001, 0xBB);

    cpu.step(&mut bus);

    assert_eq!(bus.read(0x4000), 0xBB);
    assert_eq!(bus.read(0x4001), 0x00);
    assert_eq!(cpu.sp, 0xFD);
}

#[test]
fn tia_alternates_destination_bytes() {
    let program = [0xE3, 0x00, 0x90, 0x00, 0x40, 0x02, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x9000, 0x5A);
    bus.write(0x9001, 0xC3);

    cpu.step(&mut bus);

    assert_eq!(bus.read(0x4000), 0x5A);
    assert_eq!(bus.read(0x4001), 0xC3);
    assert_eq!(cpu.sp, 0xFD);
}

#[test]
fn tdd_transfers_decrementing_addresses() {
    let program = [0xC3, 0x02, 0x90, 0x02, 0x40, 0x03, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x9002, 0x11);
    bus.write(0x9001, 0x22);
    bus.write(0x9000, 0x33);

    cpu.step(&mut bus);

    assert_eq!(bus.read(0x4002), 0x11);
    assert_eq!(bus.read(0x4001), 0x22);
    assert_eq!(bus.read(0x4000), 0x33);
    assert_eq!(cpu.sp, 0xFD);
}

#[test]
fn tai_reads_alternating_source_bytes() {
    let program = [0xF3, 0x00, 0x90, 0x00, 0x30, 0x04, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x9000, 0xAA);
    bus.write(0x9001, 0xBB);
    bus.write(0x9002, 0xCC);

    cpu.step(&mut bus);

    assert_eq!(bus.read(0x3000), 0xAA);
    assert_eq!(bus.read(0x3001), 0xBB);
    assert_eq!(bus.read(0x3002), 0xAA);
    assert_eq!(bus.read(0x3003), 0xBB);
    assert_eq!(cpu.sp, 0xFD);
}

#[test]
fn block_moves_treat_zero_length_as_65536_iterations() {
    let program = block_transfer_program(0x73, 0x9000, 0x2000, 0x0000);
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x9000, 0x42);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 17 + 6 * 0x1_0000u32);
    assert_eq!(cpu.sp, 0xFD);
}

#[test]
fn block_moves_report_full_cycle_counts() {
    let cases = [
        (0x73, 0x9000, 0x4000), // TII
        (0xC3, 0x9002, 0x4002), // TDD
        (0xD3, 0x9000, 0x4000), // TIN
        (0xE3, 0x9000, 0x4000), // TIA
        (0xF3, 0x9000, 0x4000), // TAI
    ];
    for (opcode, source, dest) in cases {
        let program = block_transfer_program(opcode, source, dest, 0x0003);
        let (mut cpu, mut bus) = setup_cpu_with_program(&program);
        for i in 0..4u16 {
            bus.write(source.wrapping_add(i), (i as u8).wrapping_add(0x10));
        }
        let cycles = cpu.step(&mut bus);
        assert_eq!(
            cycles,
            17 + 6 * 3u32,
            "opcode {:02X} returned unexpected cycle count",
            opcode
        );
    }
}

#[test]
fn block_move_can_target_timer_io_registers() {
    let program = block_transfer_program(0x73, 0x9000, 0x0C00, 0x0002);
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.set_mpr(0, 0xFF);
    bus.write(0x9000, 0x02); // timer reload
    bus.write(0x9001, 0x01); // timer start

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 17 + 6 * 2u32);
    assert_eq!(bus.read(0x0C00), 0x02);
    assert_eq!(bus.read(0x0C01) & 0x01, 0x01);

    // Confirm the timer side effect is live after DMA-style register writes.
    bus.tick(1024u32 * 3, true);
    assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_TIMER, 0);
}

#[test]
fn ina_dea_adjust_accumulator_and_flags() {
    let program = [0x1A, 0x1A, 0x3A, 0x3A, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x7F;
    cpu.step(&mut bus); // INA -> 0x80
    assert_eq!(cpu.a, 0x80);
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_ZERO));

    cpu.step(&mut bus); // INA -> 0x81
    assert_eq!(cpu.a, 0x81);

    cpu.step(&mut bus); // DEA -> 0x80
    assert_eq!(cpu.a, 0x80);
    assert!(cpu.flag(FLAG_NEGATIVE));

    cpu.step(&mut bus); // DEA -> 0x7F
    assert_eq!(cpu.a, 0x7F);
    assert!(!cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_ZERO));
}

#[test]
fn phx_plx_and_phy_ply_round_trip_registers() {
    let program = [0xDA, 0xFA, 0x5A, 0x7A, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.x = 0x42;
    cpu.y = 0x80;
    cpu.step(&mut bus); // PHX (push at $01FD, sp -> 0xFC)
    assert_eq!(bus.read(0x01FD), 0x42);
    cpu.x = 0x00;
    cpu.step(&mut bus); // PLX
    assert_eq!(cpu.x, 0x42);
    assert!(!cpu.flag(FLAG_ZERO));

    cpu.step(&mut bus); // PHY (rewrites $01FD, sp -> 0xFC)
    assert_eq!(bus.read(0x01FD), 0x80);
    cpu.y = 0x00;
    cpu.step(&mut bus); // PLY
    assert_eq!(cpu.y, 0x80);
    assert!(cpu.flag(FLAG_NEGATIVE));
}

#[test]
fn sta_zero_page_indirect_stores_value() {
    let program = [0xA9, 0x5A, 0x92, 0x10, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0010, 0x00);
    bus.write(0x0011, 0xC0);

    while !cpu.halted {
        cpu.step(&mut bus);
    }

    assert_eq!(bus.read(0xC000), 0x5A);
}

#[test]
fn jmp_absolute_sets_pc() {
    let program = [0x4C, 0x05, 0x80, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8005);
}

#[test]
fn jmp_indirect_crosses_page_boundary() {
    // 65C02/HuC6280 fixed the 6502 page-wrap bug: high byte is read
    // from ptr+1 even when ptr is at a page boundary ($xxFF).
    let program = [0x6C, 0xFF, 0x82, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x82FF, 0x34);
    bus.write(0x8300, 0x12); // 65C02 reads from $8300, not $8200
    bus.write(0x8200, 0xFF); // decoy: 6502 would read this
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x1234);
}

#[test]
fn jmp_indirect_indexed_uses_offset() {
    let program = [0xA2, 0x02, 0x7C, 0x00, 0x90, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.load(0x9002, &[0x78, 0x56]);
    cpu.step(&mut bus); // LDX #$02
    cpu.step(&mut bus); // JMP ($9000,X)
    assert_eq!(cpu.pc, 0x5678);
}

#[test]
fn tam_updates_mprs_and_remaps_page() {
    let program = [
        0xA9, 0xF8, // LDA #$F8 (internal RAM window)
        0x53, 0x01, // TAM #$01 (MPR0)
        0xA9, 0x5A, // LDA #$5A
        0x8D, 0x00, 0x00, // STA $0000 -> maps to page selected by MPR0
        0x00,
    ];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    while !cpu.halted {
        cpu.step(&mut bus);
    }

    assert_eq!(bus.mpr(0), 0xF8);
    assert_eq!(bus.read(0x0000), 0x5A);
}

#[test]
fn tma_reads_from_selected_mpr() {
    let program = [0xA9, 0x00, 0x43, 0x08, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.set_mpr(3, 0x44);

    while !cpu.halted {
        cpu.step(&mut bus);
    }

    assert_eq!(cpu.a, 0x44);
    // TMA does not affect flags
}

#[test]
fn rmb_clears_bit_in_zero_page() {
    let program = [0xA9, 0xFF, 0x85, 0x10, 0x07, 0x10, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    cpu.step(&mut bus); // LDA #$FF
    cpu.step(&mut bus); // STA $10
    cpu.step(&mut bus); // RMB0 $10

    assert_eq!(bus.read(0x0010), 0xFE);
}

#[test]
fn smb_sets_bit_in_zero_page() {
    let program = [0xA9, 0x00, 0x85, 0x11, 0xC7, 0x11, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    cpu.step(&mut bus); // LDA #$00
    cpu.step(&mut bus); // STA $11
    cpu.step(&mut bus); // SMB4 $11

    assert_eq!(bus.read(0x0011), 0x10);
}

#[test]
fn bbr_branches_when_bit_reset() {
    let program = [0x0F, 0x10, 0x01, 0xEA, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0010, 0x00);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8); // HuC6280: BBR base 6 + 2 taken
    assert_eq!(cpu.pc, 0x8004);
    cpu.step(&mut bus);
    assert!(cpu.halted);
}

#[test]
fn bbs_skips_when_bit_clear() {
    let program = [0x8F, 0x10, 0x01, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0010, 0x00);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 6); // HuC6280: BBS not-taken base is 6
    assert_eq!(cpu.pc, 0x8003);
    cpu.step(&mut bus);
    assert!(cpu.halted);
}

#[test]
fn bbs_branches_when_bit_set() {
    let program = [0x8F, 0x10, 0x01, 0xEA, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0010, 0x01);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8); // HuC6280: BBS base 6 + 2 taken
    assert_eq!(cpu.pc, 0x8004);
    cpu.step(&mut bus);
    assert!(cpu.halted);
}

#[test]
fn bbr_taken_cross_page_costs_extra_cycle() {
    let mut program = vec![0xEA; 0xFC];
    program.extend([0x0F, 0x10, 0x02, 0xEA, 0x00, 0x00]);
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0010, 0x00);
    cpu.pc = 0x80FC;

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.pc, 0x8101);
    assert_eq!(bus.read(0x0010), 0x00);
    cpu.step(&mut bus);
    assert!(cpu.halted);
}

#[test]
fn bbs_taken_cross_page_costs_extra_cycle() {
    let mut program = vec![0xEA; 0xFC];
    program.extend([0x8F, 0x10, 0x02, 0xEA, 0x00, 0x00]);
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0010, 0x01);
    cpu.pc = 0x80FC;

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.pc, 0x8101);
    assert_eq!(bus.read(0x0010), 0x01);
    cpu.step(&mut bus);
    assert!(cpu.halted);
}

#[test]
fn tst_zp_sets_flags_based_on_mask_and_value() {
    let program = [0x83, 0xF0, 0x20, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0020, 0xF0);
    cpu.a = 0x00; // TST does not use A but ensure non-zero

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8); // HuC6280: TST zp is 8 cycles
    assert!(!cpu.flag(FLAG_ZERO));
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(cpu.flag(FLAG_OVERFLOW));
}

#[test]
fn tst_abs_sets_zero_when_masked_out() {
    // mask=0x0F, memory=0xF0: AND result = 0x00 => Z=1
    // N and V come from memory value (0xF0): N=1, V=1
    let program = [0x93, 0x0F, 0x00, 0x90, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x9000, 0xF0);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8);
    assert!(cpu.flag(FLAG_ZERO));
    assert!(cpu.flag(FLAG_NEGATIVE), "N comes from memory bit 7");
    assert!(cpu.flag(FLAG_OVERFLOW), "V comes from memory bit 6");
}

#[test]
fn cla_clx_cly_clear_registers() {
    let program = [0x62, 0x82, 0xC2, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0xFF;
    cpu.x = 0x80;
    cpu.y = 0x01;
    cpu.set_flag(FLAG_NEGATIVE, true);
    cpu.set_flag(FLAG_ZERO, false);

    cpu.step(&mut bus); // CLA
    assert_eq!(cpu.a, 0);
    assert!(cpu.flag(FLAG_NEGATIVE), "CLA should not affect flags");
    assert!(!cpu.flag(FLAG_ZERO), "CLA should not affect flags");

    cpu.step(&mut bus); // CLX
    assert_eq!(cpu.x, 0);
    assert!(cpu.flag(FLAG_NEGATIVE), "CLX should not affect flags");

    cpu.step(&mut bus); // CLY
    assert_eq!(cpu.y, 0);
    assert!(cpu.flag(FLAG_NEGATIVE), "CLY should not affect flags");
}

#[test]
fn sax_say_sxy_swap_registers() {
    let program = [0x22, 0x42, 0x02, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x12;
    cpu.x = 0x34;
    cpu.y = 0x56;

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x34);
    assert_eq!(cpu.x, 0x12);

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x56);
    assert_eq!(cpu.y, 0x34);

    cpu.step(&mut bus);
    assert_eq!(cpu.x, 0x34);
    assert_eq!(cpu.y, 0x12);
}

#[test]
fn set_and_clock_switch_instructions() {
    let program = [0xF4, 0xD4, 0x54, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.set_flag(FLAG_T, false);

    cpu.step(&mut bus);
    assert!(cpu.flag(FLAG_T));

    cpu.step(&mut bus);
    assert!(!cpu.flag(FLAG_T));
    assert!(cpu.clock_high_speed);

    cpu.step(&mut bus);
    assert!(!cpu.flag(FLAG_T));
    assert!(!cpu.clock_high_speed);
}

#[test]
fn t_mode_and_writes_back_to_mpr1_x_and_keeps_a() {
    let program = [0xF4, 0x25, 0x10, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x0F;
    cpu.x = 0x10;
    bus.write(0x0010, 0xA5);
    bus.write(0x2010, 0xF0);

    cpu.step(&mut bus); // SET
    assert!(cpu.flag(FLAG_T));

    cpu.step(&mut bus); // AND zp in T-mode
    assert_eq!(cpu.a, 0x0F, "T-mode AND should not modify A");
    assert_eq!(
        bus.read(0x2010),
        0xA0,
        "T-mode AND should write result to [MPR1:X]"
    );
    assert!(!cpu.flag(FLAG_ZERO));
    assert!(cpu.flag(FLAG_NEGATIVE));
    assert!(!cpu.flag(FLAG_T), "T flag should clear after ALU op");
}

#[test]
fn t_mode_adc_writes_back_to_mpr1_x_and_clears_t() {
    let program = [0xF4, 0x65, 0x20, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x7F;
    cpu.x = 0x20;
    bus.write(0x0020, 0x10);
    cpu.set_flag(FLAG_CARRY, false);
    bus.write(0x2020, 0x20);

    cpu.step(&mut bus); // SET
    assert!(cpu.flag(FLAG_T));

    cpu.step(&mut bus); // ADC zp in T-mode
    assert_eq!(cpu.a, 0x7F, "T-mode ADC should not modify A");
    assert_eq!(bus.read(0x2020), 0x30);
    assert!(!cpu.flag(FLAG_ZERO));
    assert!(!cpu.flag(FLAG_T));
}

#[test]
fn t_mode_ora_uses_operand_value_instead_of_a() {
    let program = [0xF4, 0x05, 0x10, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x01;
    cpu.x = 0x10;
    bus.write(0x0010, 0x20);
    bus.write(0x2010, 0x40);

    cpu.step(&mut bus); // SET
    cpu.step(&mut bus); // ORA zp in T-mode
    assert_eq!(cpu.a, 0x01);
    assert_eq!(bus.read(0x2010), 0x60);
    assert!(!cpu.flag(FLAG_T));
}

#[test]
fn t_mode_eor_uses_operand_value_instead_of_a() {
    let program = [0xF4, 0x45, 0x11, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0xF0;
    cpu.x = 0x11;
    bus.write(0x0011, 0xAA);
    bus.write(0x2011, 0x0F);

    cpu.step(&mut bus); // SET
    cpu.step(&mut bus); // EOR zp in T-mode
    assert_eq!(cpu.a, 0xF0);
    assert_eq!(bus.read(0x2011), 0xA5);
    assert!(!cpu.flag(FLAG_T));
}

#[test]
fn t_mode_sbc_uses_operand_value_instead_of_a() {
    let program = [0xF4, 0xE5, 0x12, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x01;
    cpu.x = 0x12;
    cpu.set_flag(FLAG_CARRY, true);
    bus.write(0x0012, 0x20);
    bus.write(0x2012, 0x50);

    cpu.step(&mut bus); // SET
    cpu.step(&mut bus); // SBC zp in T-mode
    assert_eq!(cpu.a, 0x01);
    assert_eq!(bus.read(0x2012), 0x30);
    assert!(cpu.flag(FLAG_CARRY));
    assert!(!cpu.flag(FLAG_T));
}

#[test]
fn t_mode_adc_overflow_flag_uses_transfer_operands() {
    let program = [0xF4, 0x65, 0x13, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x00;
    cpu.x = 0x13;
    cpu.set_flag(FLAG_CARRY, false);
    bus.write(0x0013, 0x01);
    bus.write(0x2013, 0x7F);

    cpu.step(&mut bus); // SET
    cpu.step(&mut bus); // ADC zp in T-mode
    assert_eq!(bus.read(0x2013), 0x80);
    assert!(cpu.flag(FLAG_OVERFLOW));
}

#[test]
fn t_mode_adc_overflow_flag_clears_without_signed_wrap() {
    let program = [0xF4, 0x65, 0x14, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x00;
    cpu.x = 0x14;
    cpu.set_flag(FLAG_CARRY, false);
    bus.write(0x0014, 0x10);
    bus.write(0x2014, 0x20);

    cpu.step(&mut bus); // SET
    cpu.step(&mut bus); // ADC zp in T-mode
    assert_eq!(bus.read(0x2014), 0x30);
    assert!(!cpu.flag(FLAG_OVERFLOW));
}

#[test]
fn t_mode_sbc_overflow_flag_uses_transfer_operands() {
    let program = [0xF4, 0xE5, 0x15, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0xFF;
    cpu.x = 0x15;
    cpu.set_flag(FLAG_CARRY, true);
    bus.write(0x0015, 0x01);
    bus.write(0x2015, 0x80);

    cpu.step(&mut bus); // SET
    cpu.step(&mut bus); // SBC zp in T-mode
    assert_eq!(bus.read(0x2015), 0x7F);
    assert!(cpu.flag(FLAG_OVERFLOW));
}

#[test]
fn immediate_alu_op_clears_t_without_memory_transfer() {
    let program = [0xF4, 0x29, 0xF0, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x0F;
    cpu.x = 0x10;
    bus.write(0x2010, 0xAA);

    cpu.step(&mut bus); // SET
    assert!(cpu.flag(FLAG_T));

    cpu.step(&mut bus); // AND #imm
    assert_eq!(cpu.a, 0x00, "immediate AND should operate on A");
    assert_eq!(
        bus.read(0x2010),
        0xAA,
        "immediate op should not touch [MPR1:X]"
    );
    assert!(cpu.flag(FLAG_ZERO));
    assert!(!cpu.flag(FLAG_T));
}

#[test]
fn bsr_pushes_return_address() {
    // BSR pushes PC-1 (last byte of instruction), matching JSR/RTS convention.
    // RTS adds 1 to popped address, so BSR at $8000 (2 bytes: 44 02)
    // returns to $8002 (the byte after the BSR instruction).
    let program = [0x44, 0x02, 0x00, 0xEA, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.pc, 0x8004);
    assert_eq!(cpu.sp, 0xFB);
    let lo = bus.read(0x01FC);
    let hi = bus.read(0x01FD);
    assert_eq!(lo, 0x01);
    assert_eq!(hi, 0x80);
}

#[test]
fn st_ports_write_immediate_values() {
    let program = [0x03, 0xAA, 0x13, 0xBB, 0x23, 0xCC, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    let st0_cycles = cpu.step(&mut bus);
    let st1_cycles = cpu.step(&mut bus);
    let st2_cycles = cpu.step(&mut bus);

    assert_eq!(bus.st_port(0), 0xAA);
    assert_eq!(bus.st_port(1), 0xBB);
    assert_eq!(bus.st_port(2), 0xCC);
    assert_eq!(st0_cycles, Cpu::opcode_base_cycles(0x03) as u32 + 1);
    assert_eq!(st1_cycles, Cpu::opcode_base_cycles(0x13) as u32 + 1);
    assert_eq!(st2_cycles, Cpu::opcode_base_cycles(0x23) as u32 + 1);
}

#[test]
fn stp_halts_cpu() {
    let program = [0xDB, 0xEA];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 3);
    assert!(cpu.halted);

    let next_cycles = cpu.step(&mut bus);
    assert_eq!(next_cycles, 0);
}

#[test]
fn writing_mpr_via_memory_updates_mapping() {
    let program = [0xA9, 0x08, 0x8D, 0x80, 0xFF, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    // MPR registers at $FF80-$FFBF are only accessible when the
    // address maps to the hardware page.
    bus.set_mpr(7, 0xFF);

    cpu.step(&mut bus); // LDA #$08
    cpu.step(&mut bus); // STA $FF80

    assert_eq!(bus.mpr(0), 0x08);

    bus.load_rom_image(vec![0x11; PAGE_SIZE * 4]);

    assert_eq!(bus.read(0x0000), 0x11);
}

#[test]
fn wai_pauses_until_irq() {
    let program = [0xCB, 0xEA, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write_u16(0xFFFA, 0x9000);
    bus.load(0x9000, &[0xEA, 0x00]);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 3);
    assert_eq!(cpu.pc, 0x8001);

    let idle_cycles = cpu.step(&mut bus);
    assert_eq!(idle_cycles, 0);
    assert_eq!(cpu.pc, 0x8001);

    bus.tick(64, true);
    bus.raise_irq(IRQ_REQUEST_TIMER);
    let irq_cycles = cpu.step(&mut bus);
    assert_eq!(irq_cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
    assert_eq!(cpu.pc, 0x9000);
}

#[test]
fn irq_and_rti_restore_state() {
    let program = [0xEA, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write_u16(0xFFFA, 0x9000);
    bus.load(0x9000, &[0x40, 0x00]);

    cpu.status = FLAG_CARRY;
    bus.raise_irq(IRQ_REQUEST_TIMER);
    let irq_cycles = cpu.step(&mut bus);
    assert_eq!(irq_cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
    assert_eq!(cpu.pc, 0x9000);
    assert_eq!(cpu.sp, 0xFA);

    // Stack order: status pushed last at current SP+1 (0x01FB)
    assert_eq!(bus.read(0x01FB), FLAG_CARRY);
    assert_eq!(bus.read(0x01FC), 0x00); // PCL
    assert_eq!(bus.read(0x01FD), 0x80); // PCH

    let rti_cycles = cpu.step(&mut bus);
    assert_eq!(rti_cycles, 7); // HuC6280: RTI is 7 cycles
    assert_eq!(cpu.pc, 0x8000);
    assert_eq!(cpu.sp, 0xFD);
    assert!(cpu.flag(FLAG_CARRY));
}

#[test]
fn multiple_irq_sources_preserve_lower_priority() {
    let program = [0xEA, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write_u16(0xFFFA, 0x9000);
    bus.write_u16(0xFFF8, 0x9100);
    bus.write_u16(0xFFF6, 0x9200);
    bus.load(0x9000, &[0x40, 0x00]);
    bus.load(0x9100, &[0x40, 0x00]);
    bus.load(0x9200, &[0x40, 0x00]);

    cpu.status &= !FLAG_INTERRUPT_DISABLE;
    bus.raise_irq(IRQ_REQUEST_IRQ1 | IRQ_REQUEST_IRQ2 | IRQ_REQUEST_TIMER);

    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
    assert_eq!(cpu.pc, 0x9000);
    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
        IRQ_REQUEST_IRQ1
    );
    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ2,
        IRQ_REQUEST_IRQ2
    );

    let _ = cpu.step(&mut bus); // RTI from timer handler
    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
        IRQ_REQUEST_IRQ1
    );
    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ2,
        IRQ_REQUEST_IRQ2
    );

    let cycles = cpu.step(&mut bus); // service IRQ1
    assert_eq!(cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
    assert_eq!(cpu.pc, 0x9100);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ2,
        IRQ_REQUEST_IRQ2
    );

    let _ = cpu.step(&mut bus); // RTI from IRQ1 handler
    let cycles = cpu.step(&mut bus); // service IRQ2
    assert_eq!(cycles, 8); // HuC6280: IRQ vectoring is 8 cycles
    assert_eq!(cpu.pc, 0x9200);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ2, 0);
}

#[test]
fn reset_prefers_huc6280_reset_vector_slot() {
    let mut bus = Bus::new();
    bus.write_u16(0xFFFE, 0x8123);
    bus.write_u16(0xFFFC, 0x9000);

    let mut cpu = Cpu::new();
    cpu.reset(&mut bus);

    assert_eq!(cpu.pc, 0x8123);
}

// --- T-mode SBC overflow: no overflow when signs are the same ---

#[test]
fn t_mode_sbc_no_overflow_when_signs_same() {
    // mem=0x50 (positive), value=0x10 (positive) => 0x50-0x10=0x40 (positive)
    // Same-sign operands subtracted: no overflow expected.
    let program = [0xF4, 0xE5, 0x16, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0xFF;
    cpu.x = 0x16;
    cpu.set_flag(FLAG_CARRY, true);
    bus.write(0x0016, 0x10); // value (subtrahend)
    bus.write(0x2016, 0x50); // mem (minuend)

    cpu.step(&mut bus); // SET
    cpu.step(&mut bus); // SBC zp in T-mode
    assert_eq!(bus.read(0x2016), 0x40);
    assert!(!cpu.flag(FLAG_OVERFLOW));
    assert!(cpu.flag(FLAG_CARRY));
}

// --- BCD mode tests ---

#[test]
fn adc_bcd_mode_basic_addition() {
    // 0x15 + 0x27 = 0x42 in BCD
    let program = [0x69, 0x27, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x15;
    cpu.set_flag(FLAG_DECIMAL, true);
    cpu.set_flag(FLAG_CARRY, false);

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x42);
    assert!(!cpu.flag(FLAG_CARRY));
}

#[test]
fn adc_bcd_mode_carry_out() {
    // 0x99 + 0x01 = 0x00 with carry in BCD
    let program = [0x69, 0x01, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x99;
    cpu.set_flag(FLAG_DECIMAL, true);
    cpu.set_flag(FLAG_CARRY, false);

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.flag(FLAG_CARRY));
}

#[test]
fn adc_bcd_mode_with_carry_in() {
    // 0x58 + 0x01 + carry = 0x60 in BCD
    let program = [0x69, 0x01, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x58;
    cpu.set_flag(FLAG_DECIMAL, true);
    cpu.set_flag(FLAG_CARRY, true);

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x60);
    assert!(!cpu.flag(FLAG_CARRY));
}

#[test]
fn sbc_bcd_mode_basic_subtraction() {
    // 0x42 - 0x15 = 0x27 in BCD
    let program = [0xE9, 0x15, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x42;
    cpu.set_flag(FLAG_DECIMAL, true);
    cpu.set_flag(FLAG_CARRY, true); // no borrow

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x27);
    assert!(cpu.flag(FLAG_CARRY));
}

#[test]
fn sbc_bcd_mode_borrow() {
    // 0x10 - 0x20 = 0x90 with borrow in BCD
    let program = [0xE9, 0x20, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x10;
    cpu.set_flag(FLAG_DECIMAL, true);
    cpu.set_flag(FLAG_CARRY, true); // no borrow

    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0x90);
    assert!(!cpu.flag(FLAG_CARRY));
}

// --- TST N/V from memory value ---

#[test]
fn tst_nv_flags_come_from_memory_value() {
    // mask=0xFF, memory=0x40: AND=0x40 (non-zero) => Z=0
    // N from memory bit 7 = 0, V from memory bit 6 = 1
    let program = [0x83, 0xFF, 0x10, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write(0x0010, 0x40);

    cpu.step(&mut bus);
    assert!(!cpu.flag(FLAG_ZERO));
    assert!(!cpu.flag(FLAG_NEGATIVE), "N = memory bit 7 = 0");
    assert!(cpu.flag(FLAG_OVERFLOW), "V = memory bit 6 = 1");
}

// --- SAX/SAY/SXY do not affect flags ---

#[test]
fn sax_say_sxy_do_not_affect_flags() {
    let program = [0x22, 0x42, 0x02, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.a = 0x80;
    cpu.x = 0x00;
    cpu.y = 0x01;
    cpu.set_flag(FLAG_ZERO, false);
    cpu.set_flag(FLAG_NEGATIVE, false);

    cpu.step(&mut bus); // SAX: A=0x00, X=0x80
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.x, 0x80);
    assert!(!cpu.flag(FLAG_ZERO), "SAX should not set Z");
    assert!(!cpu.flag(FLAG_NEGATIVE), "SAX should not set N");

    cpu.step(&mut bus); // SAY: A=0x01, Y=0x00
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.y, 0x00);
    assert!(!cpu.flag(FLAG_ZERO), "SAY should not set Z");

    cpu.step(&mut bus); // SXY: X=0x00, Y=0x80
    assert_eq!(cpu.x, 0x00);
    assert_eq!(cpu.y, 0x80);
    assert!(!cpu.flag(FLAG_NEGATIVE), "SXY should not set N");
}

// --- WAI does not modify I flag ---

#[test]
fn wai_does_not_set_interrupt_disable() {
    let program = [0xCB, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    cpu.set_flag(FLAG_INTERRUPT_DISABLE, false);

    cpu.step(&mut bus); // WAI
    assert!(cpu.waiting);
    assert!(
        !cpu.flag(FLAG_INTERRUPT_DISABLE),
        "WAI should not modify I flag"
    );
}

// --- Interrupt clears D flag ---

#[test]
fn interrupt_clears_decimal_flag() {
    let program = [0xEA, 0x00];
    let (mut cpu, mut bus) = setup_cpu_with_program(&program);
    bus.write_u16(VECTOR_IRQ2_BRK, 0x9000);
    bus.load(0x9000, &[0xEA, 0x00]);
    cpu.set_flag(FLAG_DECIMAL, true);
    cpu.set_flag(FLAG_INTERRUPT_DISABLE, false);

    bus.tick(64, true);
    bus.raise_irq(IRQ_REQUEST_TIMER);
    cpu.step(&mut bus);

    assert!(
        !cpu.flag(FLAG_DECIMAL),
        "Interrupt should clear D flag (65C02 behavior)"
    );
}
