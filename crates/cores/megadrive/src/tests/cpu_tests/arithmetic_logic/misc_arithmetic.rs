use super::super::*;

#[test]
fn clr_word_supports_postincrement_destination() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0020, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // move.w #$BEEF, $00FF0020
    rom[0x106..0x108].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0xBEEFu16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // clr.w (a0)+
    rom[0x10E..0x110].copy_from_slice(&0x4258u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    cpu.step(&mut memory);

    assert_eq!(memory.read_u16(0x00FF_0020), 0x0000);
    assert_eq!(cpu.a_regs[0], 0x00FF_0022);
}

#[test]
fn representative_single_word_opcodes_do_not_fall_back_to_unknown() {
    // Keep this list focused on one-word opcodes across major decode families.
    // If dispatch ordering regresses, one or more of these will hit unknown.
    let opcodes: &[u16] = &[
        0x4E71, // nop
        0x4E70, // reset
        0x4E76, // trapv
        0x7001, // moveq #1,d0
        0x4000, // negx.b d0
        0x4200, // clr.b d0
        0x4400, // neg.b d0
        0x4600, // not.b d0
        0x4A00, // tst.b d0
        0x4840, // swap d0
        0x4880, // ext.w d0
        0x48C0, // ext.l d0
        0x8000, // or.b d0,d0
        0x9000, // sub.b d0,d0
        0xB000, // cmp.b d0,d0
        0xC000, // and.b d0,d0
        0xD000, // add.b d0,d0
        0xE300, // asl.b #1,d0
        0xD100, // addx.b d0,d0
        0x9100, // subx.b d0,d0
        0x4180, // chk.w d0,d0
        0x80C0, // divu.w d0,d0
        0x81C0, // divs.w d0,d0
        0xC0C0, // mulu.w d0,d0
        0xC1C0, // muls.w d0,d0
        0xB0C0, // cmpa.w d0,a0
        0xB1C0, // cmpa.l d0,a0
        0xD0C0, // adda.w d0,a0
        0xD1C0, // adda.l d0,a0
        0x90C0, // suba.w d0,a0
        0x91C0, // suba.l d0,a0
        0xC140, // exg d0,d0
        0xC148, // exg a0,a0
        0xC188, // exg d0,a0
        0x40C0, // move sr,d0
        0x44C0, // move d0,ccr
    ];

    for &opcode in opcodes {
        let mut rom = vec![0u8; 0x400];
        rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
        rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
        rom[0x100..0x102].copy_from_slice(&opcode.to_be_bytes());

        let cart = Cartridge::from_bytes(rom).expect("valid rom");
        let mut memory = MemoryMap::new(cart);
        let mut cpu = M68k::new();
        cpu.reset(&mut memory);
        // Stable non-zero operands for arithmetic/divide/chk families.
        cpu.d_regs[0] = 1;
        cpu.a_regs[0] = 2;

        let cycles = cpu.step(&mut memory);
        assert!(
            cycles > 0,
            "opcode {:04X} must consume positive cycles",
            opcode
        );
        assert_eq!(
            cpu.unknown_opcode_total(),
            0,
            "opcode {:04X} unexpectedly fell back to unknown decode",
            opcode
        );
    }
}

#[test]
fn representative_extension_word_opcodes_do_not_fall_back_to_unknown() {
    // Extension-word coverage across immediate, branch, control-flow and
    // effective-address decoding paths.
    let cases: &[(&str, &[u16])] = &[
        ("ori_to_ccr", &[0x003C, 0x0011]),
        ("ori_to_sr", &[0x007C, 0x2000]),
        ("andi_to_ccr", &[0x023C, 0x001F]),
        ("andi_to_sr", &[0x027C, 0x2700]),
        ("eori_to_ccr", &[0x0A3C, 0x0001]),
        ("eori_to_sr", &[0x0A7C, 0x0001]),
        ("ori_b_imm_d0", &[0x0000, 0x0080]),
        ("andi_w_imm_d0", &[0x0240, 0x00FF]),
        ("subi_w_imm_d0", &[0x0440, 0x0001]),
        ("addi_l_imm_d0", &[0x0680, 0x0000, 0x0001]),
        ("eori_w_imm_d0", &[0x0A40, 0x00FF]),
        ("cmpi_l_imm_d0", &[0x0C80, 0x0000, 0x0001]),
        ("btst_imm_d0", &[0x0800, 0x0000]),
        ("movea_l_imm_a0", &[0x207C, 0x00FF, 0x0000]),
        ("lea_d16_a0", &[0x41E8, 0x0002]),
        ("pea_d16_a0", &[0x4868, 0x0002]),
        ("movem_l_d0_predec_a7", &[0x48E7, 0x0001]),
        ("movem_l_postinc_a7_d0", &[0x4CDF, 0x0001]),
        ("movep_w_mem_to_d0", &[0x0108, 0x0000]),
        ("move_w_abs_l_d0", &[0x3039, 0x00FF, 0x0000]),
        ("bra_w", &[0x6000, 0x0000]),
        ("bsr_w", &[0x6100, 0x0000]),
        ("bne_w", &[0x6600, 0x0000]),
        ("jsr_abs_l", &[0x4EB9, 0x0000, 0x0120]),
        ("jmp_abs_l", &[0x4EF9, 0x0000, 0x0120]),
    ];

    for &(name, words) in cases {
        let mut rom = vec![0u8; 0x600];
        rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
        rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

        for (i, word) in words.iter().enumerate() {
            let offset = 0x100 + i * 2;
            rom[offset..offset + 2].copy_from_slice(&word.to_be_bytes());
        }
        // JSR/JMP target body.
        rom[0x120..0x122].copy_from_slice(&0x4E71u16.to_be_bytes());

        let cart = Cartridge::from_bytes(rom).expect("valid rom");
        let mut memory = MemoryMap::new(cart);
        let mut cpu = M68k::new();
        cpu.reset(&mut memory);

        cpu.d_regs[0] = 1;
        cpu.a_regs[0] = 0x00FF_0000;
        memory.write_u32(0x00FF_0000, 0x1122_3344);
        memory.write_u32(0x00FF_0004, 0x5566_7788);

        let cycles = cpu.step(&mut memory);
        assert!(cycles > 0, "{name}: instruction must consume cycles");
        assert_eq!(
            cpu.unknown_opcode_total(),
            0,
            "{name}: decode unexpectedly fell back to unknown",
        );
    }
}

#[test]
fn tas_does_not_write_back_to_memory() {
    // TAS.B (A0) — opcode 0x4AD0
    // On real Genesis hardware, TAS reads the byte and sets the N/Z flags,
    // but the write-back (setting bit 7) does NOT reach external memory.
    let mut rom = vec![0u8; 0x10400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes()); // SSP
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes()); // PC

    // move.l #$00FF0000, a0  (lea work RAM)
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0000u32.to_be_bytes());
    // tas.b (a0)
    rom[0x106..0x108].copy_from_slice(&0x4AD0u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    // Write a known value to work RAM
    memory.write_u8(0xFF0000, 0x42);

    // Execute move.l #$00FF0000, a0
    cpu.step(&mut memory);
    // Execute tas.b (a0)
    cpu.step(&mut memory);

    // Memory should NOT be modified (Genesis TAS broken write-back)
    assert_eq!(
        memory.read_u8(0xFF0000),
        0x42,
        "TAS should not write back to external memory on Genesis"
    );
    // Flags should still be set based on the read value (0x42)
    let sr = cpu.sr();
    assert!(sr & CCR_Z == 0, "0x42 is not zero");
    assert!(sr & CCR_N == 0, "bit 7 of 0x42 is not set");
}
