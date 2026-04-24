use super::super::*;

#[test]
fn executes_ori_and_andi_for_data_register_and_memory() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // ori.b #$80, d0
    rom[0x102..0x104].copy_from_slice(&0x0000u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x0080u16.to_be_bytes());
    // andi.b #$0F, d0
    rom[0x106..0x108].copy_from_slice(&0x0200u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x000Fu16.to_be_bytes());
    // move.l #$00F0000F, $00FF0020
    rom[0x10A..0x10C].copy_from_slice(&0x23FCu16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00F0_000Fu32.to_be_bytes());
    rom[0x110..0x114].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // ori.l #$0000F000, $00FF0020
    rom[0x114..0x116].copy_from_slice(&0x00B9u16.to_be_bytes());
    rom[0x116..0x11A].copy_from_slice(&0x0000_F000u32.to_be_bytes());
    rom[0x11A..0x11E].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // andi.l #$0000FF00, $00FF0020
    rom[0x11E..0x120].copy_from_slice(&0x02B9u16.to_be_bytes());
    rom[0x120..0x124].copy_from_slice(&0x0000_FF00u32.to_be_bytes());
    rom[0x124..0x128].copy_from_slice(&0x00FF_0020u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0] & 0xFF, 0x00);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(memory.read_u32(0x00FF_0020), 0x0000_F000);
}

#[test]
fn executes_eori_for_data_register_and_memory() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #$55, d0
    rom[0x100..0x102].copy_from_slice(&0x7055u16.to_be_bytes());
    // eori.b #$FF, d0
    rom[0x102..0x104].copy_from_slice(&0x0A00u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x00FFu16.to_be_bytes());
    // move.l #$00FF00FF, $00FF0020
    rom[0x106..0x108].copy_from_slice(&0x23FCu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_00FFu32.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // eori.l #$00FF0000, $00FF0020
    rom[0x110..0x112].copy_from_slice(&0x0AB9u16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0x00FF_0000u32.to_be_bytes());
    rom[0x116..0x11A].copy_from_slice(&0x00FF_0020u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0] & 0xFF, 0xAA);
    assert_eq!(memory.read_u32(0x00FF_0020), 0x0000_00FF);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn executes_eor_dn_to_ea_for_register_and_memory_destinations() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #$0F, d0
    rom[0x100..0x102].copy_from_slice(&0x700Fu16.to_be_bytes());
    // moveq #$33, d1
    rom[0x102..0x104].copy_from_slice(&0x7233u16.to_be_bytes());
    // eor.b d1, d0
    rom[0x104..0x106].copy_from_slice(&0xB300u16.to_be_bytes());
    // movea.l #$00FF0040, a0
    rom[0x106..0x108].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #$00F0, $00FF0042
    rom[0x10C..0x10E].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x00F0u16.to_be_bytes());
    rom[0x110..0x114].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // eor.w d0, (2,a0)
    rom[0x114..0x116].copy_from_slice(&0xB168u16.to_be_bytes());
    rom[0x116..0x118].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..6 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0] & 0xFF, 0x3C);
    assert_eq!(memory.read_u16(0x00FF_0042), 0x00CC);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn abcd_memory_mode_updates_xc_and_preserves_zero_until_nonzero_result() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // abcd -(a1),-(a0)
    rom[0x100..0x102].copy_from_slice(&0xC109u16.to_be_bytes());
    // abcd -(a1),-(a0)
    rom[0x102..0x104].copy_from_slice(&0xC109u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.a_regs[0] = 0x00FF_0007;
    cpu.a_regs[1] = 0x00FF_0005;
    memory.write_u8(0x00FF_0006, 0x45);
    memory.write_u8(0x00FF_0004, 0x55);
    memory.write_u8(0x00FF_0005, 0x00);
    memory.write_u8(0x00FF_0003, 0x55);
    cpu.sr |= CCR_Z;
    cpu.sr &= !CCR_X;

    let cycles1 = cpu.step(&mut memory);
    assert_eq!(cycles1, 18);
    assert_eq!(memory.read_u8(0x00FF_0006), 0x00);
    assert_ne!(cpu.sr() & CCR_C, 0);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_ne!(cpu.sr() & CCR_Z, 0);

    let cycles2 = cpu.step(&mut memory);
    assert_eq!(cycles2, 18);
    assert_eq!(memory.read_u8(0x00FF_0005), 0x56);
    assert_eq!(cpu.sr() & CCR_C, 0);
    assert_eq!(cpu.sr() & CCR_X, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn sbcd_memory_mode_predecrements_address_registers_and_writes_result() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // sbcd -(a1),-(a0)
    rom[0x100..0x102].copy_from_slice(&0x8109u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.a_regs[0] = 0x00FF_0005;
    cpu.a_regs[1] = 0x00FF_0003;
    memory.write_u8(0x00FF_0004, 0x00);
    memory.write_u8(0x00FF_0002, 0x01);
    cpu.sr |= CCR_X | CCR_Z;

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 18);
    assert_eq!(cpu.a_regs[0], 0x00FF_0004);
    assert_eq!(cpu.a_regs[1], 0x00FF_0002);
    assert_eq!(memory.read_u8(0x00FF_0004), 0x98);
    assert_ne!(cpu.sr() & CCR_C, 0);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn exg_swaps_data_and_address_register_variants() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #1, d0
    rom[0x100..0x102].copy_from_slice(&0x7001u16.to_be_bytes());
    // moveq #2, d1
    rom[0x102..0x104].copy_from_slice(&0x7202u16.to_be_bytes());
    // exg d0,d1
    rom[0x104..0x106].copy_from_slice(&0xC141u16.to_be_bytes());
    // movea.l #$11223344, a0
    rom[0x106..0x108].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x1122_3344u32.to_be_bytes());
    // movea.l #$55667788, a1
    rom[0x10C..0x10E].copy_from_slice(&0x227Cu16.to_be_bytes());
    rom[0x10E..0x112].copy_from_slice(&0x5566_7788u32.to_be_bytes());
    // exg a0,a1
    rom[0x112..0x114].copy_from_slice(&0xC149u16.to_be_bytes());
    // exg d0,a0
    rom[0x114..0x116].copy_from_slice(&0xC188u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..7 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0x5566_7788);
    assert_eq!(cpu.d_regs[1], 0x0000_0001);
    assert_eq!(cpu.a_regs[0], 0x0000_0002);
    assert_eq!(cpu.a_regs[1], 0x1122_3344);
}

#[test]
fn addi_and_subi_byte_update_flags() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #1, d0
    rom[0x100..0x102].copy_from_slice(&0x7001u16.to_be_bytes());
    // addi.b #$7F, d0
    rom[0x102..0x104].copy_from_slice(&0x0600u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x007Fu16.to_be_bytes());
    // subi.b #$80, d0
    rom[0x106..0x108].copy_from_slice(&0x0400u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0080u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // moveq
    cpu.step(&mut memory); // addi.b
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x80);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_ne!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);

    cpu.step(&mut memory); // subi.b
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x00);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn addi_and_subi_long_support_absolute_long_memory_destination() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$00000010, $00FF0020
    rom[0x100..0x102].copy_from_slice(&0x23FCu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_0010u32.to_be_bytes());
    rom[0x106..0x10A].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // addi.l #$00000005, $00FF0020
    rom[0x10A..0x10C].copy_from_slice(&0x06B9u16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x0000_0005u32.to_be_bytes());
    rom[0x110..0x114].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // subi.l #$00000015, $00FF0020
    rom[0x114..0x116].copy_from_slice(&0x04B9u16.to_be_bytes());
    rom[0x116..0x11A].copy_from_slice(&0x0000_0015u32.to_be_bytes());
    rom[0x11A..0x11E].copy_from_slice(&0x00FF_0020u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // move.l
    cpu.step(&mut memory); // addi.l
    assert_eq!(memory.read_u32(0x00FF_0020), 0x0000_0015);

    cpu.step(&mut memory); // subi.l
    assert_eq!(memory.read_u32(0x00FF_0020), 0x0000_0000);
    assert_ne!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn add_and_sub_ea_to_dn_with_register_source() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #5, d0
    rom[0x100..0x102].copy_from_slice(&0x7005u16.to_be_bytes());
    // moveq #3, d1
    rom[0x102..0x104].copy_from_slice(&0x7203u16.to_be_bytes());
    // add.w d1, d0
    rom[0x104..0x106].copy_from_slice(&0xD041u16.to_be_bytes());
    // sub.b d1, d0
    rom[0x106..0x108].copy_from_slice(&0x9001u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0] & 0xFF, 0x05);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn add_and_sub_ea_to_dn_with_displacement_memory_source() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #$0010, $00FF0042
    rom[0x106..0x108].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0010u16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // move.l #$00000020, $00FF0044
    rom[0x10E..0x110].copy_from_slice(&0x23FCu16.to_be_bytes());
    rom[0x110..0x114].copy_from_slice(&0x0000_0020u32.to_be_bytes());
    rom[0x114..0x118].copy_from_slice(&0x00FF_0044u32.to_be_bytes());
    // moveq #1, d0
    rom[0x118..0x11A].copy_from_slice(&0x7001u16.to_be_bytes());
    // add.w (2,a0), d0
    rom[0x11A..0x11C].copy_from_slice(&0xD068u16.to_be_bytes());
    rom[0x11C..0x11E].copy_from_slice(&0x0002u16.to_be_bytes());
    // sub.l (4,a0), d0
    rom[0x11E..0x120].copy_from_slice(&0x90A8u16.to_be_bytes());
    rom[0x120..0x122].copy_from_slice(&0x0004u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..6 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0xFFFF_FFF1);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);
}

#[test]
fn cmp_ea_to_dn_supports_register_and_displacement_memory_sources() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #$10, d0
    rom[0x100..0x102].copy_from_slice(&0x7010u16.to_be_bytes());
    // moveq #$10, d1
    rom[0x102..0x104].copy_from_slice(&0x7210u16.to_be_bytes());
    // cmp.w d1, d0
    rom[0x104..0x106].copy_from_slice(&0xB041u16.to_be_bytes());
    // movea.l #$00FF0040, a0
    rom[0x106..0x108].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #$0011, $00FF0042
    rom[0x10C..0x10E].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0011u16.to_be_bytes());
    rom[0x110..0x114].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // cmp.w (2,a0), d0
    rom[0x114..0x116].copy_from_slice(&0xB068u16.to_be_bytes());
    rom[0x116..0x118].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    cpu.step(&mut memory); // cmp.w d1, d0
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    cpu.step(&mut memory); // cmp.w (2,a0), d0
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);
}

#[test]
fn addq_and_subq_support_register_and_displacement_memory_destinations() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #1, d0
    rom[0x100..0x102].copy_from_slice(&0x7001u16.to_be_bytes());
    // addq.b #8, d0
    rom[0x102..0x104].copy_from_slice(&0x5000u16.to_be_bytes());
    // subq.b #1, d0
    rom[0x104..0x106].copy_from_slice(&0x5300u16.to_be_bytes());
    // movea.l #$00FF0040, a0
    rom[0x106..0x108].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #$0001, $00FF0042
    rom[0x10C..0x10E].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0001u16.to_be_bytes());
    rom[0x110..0x114].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // addq.w #7, (2,a0)
    rom[0x114..0x116].copy_from_slice(&0x5E68u16.to_be_bytes());
    rom[0x116..0x118].copy_from_slice(&0x0002u16.to_be_bytes());
    // subq.w #2, (2,a0)
    rom[0x118..0x11A].copy_from_slice(&0x5568u16.to_be_bytes());
    rom[0x11A..0x11C].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x08);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }
    assert_eq!(memory.read_u16(0x00FF_0042), 0x0006);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn cmppi_and_tst_support_memory_effective_addresses() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$1234, $00FF0010
    rom[0x100..0x102].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x1234u16.to_be_bytes());
    rom[0x104..0x108].copy_from_slice(&0x00FF_0010u32.to_be_bytes());
    // movea.l #$00FF0010, a0
    rom[0x108..0x10A].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0010u32.to_be_bytes());
    // cmpi.w #$1234, (a0)
    rom[0x10E..0x110].copy_from_slice(&0x0C50u16.to_be_bytes());
    rom[0x110..0x112].copy_from_slice(&0x1234u16.to_be_bytes());
    // tst.w (a0)+
    rom[0x112..0x114].copy_from_slice(&0x4A58u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    cpu.step(&mut memory);
    assert_ne!(cpu.sr() & CCR_Z, 0, "CMPI equal should set Z");

    cpu.step(&mut memory);
    assert_eq!(cpu.sr() & CCR_Z, 0, "TST non-zero should clear Z");
    assert_eq!(cpu.a_regs[0], 0x00FF_0012);
}

#[test]
fn suba_word_and_long_immediate_are_decoded() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00000100, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // suba.w #$0004, a0
    rom[0x106..0x108].copy_from_slice(&0x90FCu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0004u16.to_be_bytes());
    // suba.l #$00000010, a0
    rom[0x10A..0x10C].copy_from_slice(&0x91FCu16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x0000_0010u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    cpu.step(&mut memory);

    assert_eq!(cpu.a_regs[0], 0x0000_00EC);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn cmpi_sets_negative_and_carry_on_underflow() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // cmpi.w #1, d0
    rom[0x102..0x104].copy_from_slice(&0x0C40u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x0001u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);

    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn cmp_to_dn_accepts_immediate_effective_address() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #5, d4
    rom[0x100..0x102].copy_from_slice(&0x7805u16.to_be_bytes());
    // cmp.b #$05, d4
    rom[0x102..0x104].copy_from_slice(&0xB83Cu16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x0005u16.to_be_bytes());
    // cmp.w #$0006, d4
    rom[0x106..0x108].copy_from_slice(&0xB87Cu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0006u16.to_be_bytes());
    // cmp.l #$00000005, d4
    rom[0x10A..0x10C].copy_from_slice(&0xB8BCu16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x0000_0005u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    assert_ne!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn and_or_to_dn_accepts_immediate_effective_address() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // or.w #$00F0, d0
    rom[0x102..0x104].copy_from_slice(&0x807Cu16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x00F0u16.to_be_bytes());
    // and.w #$00CC, d0
    rom[0x106..0x108].copy_from_slice(&0xC07Cu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x00CCu16.to_be_bytes());
    // or.l #$00010000, d0
    rom[0x10A..0x10C].copy_from_slice(&0x80BCu16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x0001_0000u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0x0001_00C0);
    assert_eq!(cpu.unknown_opcode_total(), 0);
    assert_eq!(cpu.pc(), 0x0000_0110);
}

#[test]
fn cmpa_long_with_immediate_updates_flags_without_modifying_address_register() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$000001F4, a1
    rom[0x100..0x102].copy_from_slice(&0x227Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_01F4u32.to_be_bytes());
    // cmpa.l #$000001F0, a1
    rom[0x106..0x108].copy_from_slice(&0xB3FCu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x0000_01F0u32.to_be_bytes());
    // cmpa.l #$000001F4, a1
    rom[0x10C..0x10E].copy_from_slice(&0xB3FCu16.to_be_bytes());
    rom[0x10E..0x112].copy_from_slice(&0x0000_01F4u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // movea.l
    cpu.step(&mut memory); // cmpa.l a1 - 0x1F0
    assert_eq!(cpu.a_regs[1], 0x0000_01F4);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);

    cpu.step(&mut memory); // cmpa.l a1 - 0x1F4
    assert_eq!(cpu.a_regs[1], 0x0000_01F4);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn cmpi_byte_supports_memory_destination_and_updates_flags() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // cmpi.b #$20, (a0)
    rom[0x106..0x108].copy_from_slice(&0x0C10u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0020u16.to_be_bytes());
    // cmpi.b #$7F, (a0)
    rom[0x10A..0x10C].copy_from_slice(&0x0C10u16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x007Fu16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u8(0x00FF_0040, 0x20);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // movea.l
    cpu.step(&mut memory); // cmpi.b equal
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);

    cpu.step(&mut memory); // cmpi.b 0x20 - 0x7F
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);
}

#[test]
fn cmp_word_and_long_support_an_source() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00000003, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_0003u32.to_be_bytes());
    // moveq #5, d0
    rom[0x106..0x108].copy_from_slice(&0x7005u16.to_be_bytes());
    // cmp.w a0, d0
    rom[0x108..0x10A].copy_from_slice(&0xB048u16.to_be_bytes());
    // cmp.l a0, d0
    rom[0x10A..0x10C].copy_from_slice(&0xB088u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.unknown_opcode_total(), 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn immediate_sr_operations_are_privileged_and_ccr_operations_are_not() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Privilege violation vector
    rom[0x20..0x24].copy_from_slice(&0x0000_0200u32.to_be_bytes());

    // ori #$0011, ccr
    rom[0x100..0x102].copy_from_slice(&0x003Cu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0011u16.to_be_bytes());
    // andi #$0015, ccr
    rom[0x104..0x106].copy_from_slice(&0x023Cu16.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0x0015u16.to_be_bytes());
    // eori #$0004, ccr
    rom[0x108..0x10A].copy_from_slice(&0x0A3Cu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x0004u16.to_be_bytes());
    // ori #$2000, sr (must trap in user mode)
    rom[0x10C..0x10E].copy_from_slice(&0x007Cu16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x2000u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.sr &= !SR_SUPERVISOR;

    cpu.step(&mut memory); // ori to ccr
    assert_eq!(cpu.sr() & 0x001F, 0x0011);

    cpu.step(&mut memory); // andi to ccr
    assert_eq!(cpu.sr() & 0x001F, 0x0011);

    cpu.step(&mut memory); // eori to ccr
    assert_eq!(cpu.sr() & 0x001F, 0x0015);

    let cycles = cpu.step(&mut memory); // ori to sr => privilege violation
    assert_eq!(cycles, 34);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.exception_histogram.get(&8).copied(), Some(1));
}

#[test]
fn tst_byte_supports_register_and_memory_effective_addresses() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #-1, d0
    rom[0x100..0x102].copy_from_slice(&0x70FFu16.to_be_bytes());
    // tst.b d0
    rom[0x102..0x104].copy_from_slice(&0x4A00u16.to_be_bytes());
    // moveq #0, d0
    rom[0x104..0x106].copy_from_slice(&0x7000u16.to_be_bytes());
    // tst.b d0
    rom[0x106..0x108].copy_from_slice(&0x4A00u16.to_be_bytes());
    // movea.l #$00FF0050, a0
    rom[0x108..0x10A].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0050u32.to_be_bytes());
    // tst.b (a0)
    rom[0x10E..0x110].copy_from_slice(&0x4A10u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u8(0x00FF_0050, 0x80);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // moveq #-1
    cpu.step(&mut memory); // tst.b d0
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory); // moveq #0
    cpu.step(&mut memory); // tst.b d0
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_ne!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory); // movea.l
    cpu.step(&mut memory); // tst.b (a0)
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(memory.read_u8(0x00FF_0050), 0x80);
}

#[test]
fn tst_pc_relative_is_illegal_on_68000() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Illegal instruction vector #4.
    rom[0x10..0x14].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    // tst.b (4,pc)
    rom[0x100..0x102].copy_from_slice(&0x4A3Au16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0004u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 34);
    assert_eq!(cpu.pc(), 0x0000_0180);
    assert_eq!(cpu.unknown_opcode_total(), 1);
}

#[test]
fn neg_and_not_are_decoded_and_update_results() {
    let mut rom = vec![0u8; 0x700];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #1, d6
    rom[0x100..0x102].copy_from_slice(&0x7C01u16.to_be_bytes());
    // neg.w d6 (4446)
    rom[0x102..0x104].copy_from_slice(&0x4446u16.to_be_bytes());
    // moveq #0, d0
    rom[0x104..0x106].copy_from_slice(&0x7000u16.to_be_bytes());
    // not.b d0 (4600)
    rom[0x106..0x108].copy_from_slice(&0x4600u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[6] & 0xFFFF, 0xFFFF);
    assert_eq!(cpu.d_regs[0] & 0xFF, 0xFF);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn neg_and_not_memory_modes_consume_displacement_once() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a1
    rom[0x100..0x102].copy_from_slice(&0x227Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #$0001, $00FF0042
    rom[0x106..0x108].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0001u16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // neg.w (2,a1)
    rom[0x10E..0x110].copy_from_slice(&0x4469u16.to_be_bytes());
    rom[0x110..0x112].copy_from_slice(&0x0002u16.to_be_bytes());
    // not.w (2,a1)
    rom[0x112..0x114].copy_from_slice(&0x4669u16.to_be_bytes());
    rom[0x114..0x116].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u16(0x00FF_0042), 0x0000);
    assert_eq!(cpu.pc(), 0x0000_0116);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn add_sub_word_and_long_support_an_source() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // movea.l #$00000003, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_0003u32.to_be_bytes());
    // moveq #5, d0
    rom[0x106..0x108].copy_from_slice(&0x7005u16.to_be_bytes());
    // add.w a0,d0 ; sub.w a0,d0
    rom[0x108..0x10A].copy_from_slice(&0xD048u16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x9048u16.to_be_bytes());
    // add.l a0,d0 ; sub.l a0,d0
    rom[0x10C..0x10E].copy_from_slice(&0xD088u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x9088u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..6 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.unknown_opcode_total(), 0);
    assert_eq!(cpu.d_regs[0], 0x0000_0005);
}

#[test]
fn cmpi_rejects_non_data_alterable_destinations() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Illegal instruction vector #4.
    rom[0x10..0x14].copy_from_slice(&0x0000_0180u32.to_be_bytes());

    // cmpi.w #$1234,a0 (An direct destination is illegal)
    rom[0x100..0x102].copy_from_slice(&0x0C48u16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x1234u16.to_be_bytes());
    // cmpi.w #$5678,(4,pc) (PC-relative destination is not alterable)
    rom[0x104..0x106].copy_from_slice(&0x0C7Au16.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0x5678u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0004u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.pc = 0x0000_0100;
    cpu.a_regs[7] = cpu.ssp;
    let c1 = cpu.step(&mut memory);
    assert_eq!(c1, 34);
    assert_eq!(cpu.pc(), 0x0000_0180);
    let sp1 = cpu.a7();
    assert_eq!(memory.read_u32(sp1 + 2), 0x0000_0102);

    cpu.pc = 0x0000_0104;
    cpu.a_regs[7] = cpu.ssp;
    let c2 = cpu.step(&mut memory);
    assert_eq!(c2, 34);
    assert_eq!(cpu.pc(), 0x0000_0180);
    let sp2 = cpu.a7();
    assert_eq!(memory.read_u32(sp2 + 2), 0x0000_0106);
}

#[test]
fn negx_byte_uses_extend_and_updates_flags() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x4000u16.to_be_bytes()); // negx.b d0

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.d_regs[0] = 0x0000_0000;
    cpu.sr |= CCR_X | CCR_Z;

    cpu.step(&mut memory);
    assert_eq!(cpu.d_regs[0] & 0xFF, 0xFF);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn nbcd_and_tas_are_decoded() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x4800u16.to_be_bytes()); // nbcd d0
    rom[0x102..0x104].copy_from_slice(&0x4AC1u16.to_be_bytes()); // tas d1

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.d_regs[0] = 0x0000_0001;
    cpu.d_regs[1] = 0x0000_0001;
    cpu.sr |= CCR_Z;

    cpu.step(&mut memory);
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x99);
    assert_ne!(cpu.sr() & CCR_C, 0);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory);
    assert_eq!(cpu.d_regs[1] & 0xFF, 0x81);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn chk_w_raises_vector_6_for_negative_or_out_of_range() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // CHK vector #6
    rom[0x18..0x1C].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    // chk.w d1,d0
    rom[0x100..0x102].copy_from_slice(&0x4181u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.d_regs[1] = 10;

    cpu.d_regs[0] = 5;
    let ok_cycles = cpu.step(&mut memory);
    assert_eq!(ok_cycles, 10);
    assert_eq!(cpu.pc(), 0x0000_0102);

    cpu.pc = 0x0000_0100;
    cpu.d_regs[0] = 11;
    let trap_cycles = cpu.step(&mut memory);
    assert_eq!(trap_cycles, 40);
    assert_eq!(cpu.pc(), 0x0000_0180);

    cpu.pc = 0x0000_0100;
    cpu.a_regs[7] = cpu.ssp;
    cpu.d_regs[0] = 0xFFFF_FFFF;
    let trap_neg_cycles = cpu.step(&mut memory);
    assert_eq!(trap_neg_cycles, 40);
    assert_eq!(cpu.pc(), 0x0000_0180);
}

#[test]
fn addx_and_subx_data_register_mode_are_decoded() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // addx.b d1,d0
    rom[0x100..0x102].copy_from_slice(&0xD101u16.to_be_bytes());
    // subx.b d1,d0
    rom[0x102..0x104].copy_from_slice(&0x9101u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.d_regs[0] = 0x0000_0010;
    cpu.d_regs[1] = 0x0000_0001;
    cpu.sr |= CCR_X | CCR_Z;

    cpu.step(&mut memory);
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x12);
    assert_eq!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory);
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x11);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn cmpm_byte_word_long_are_decoded() {
    let mut rom = vec![0u8; 0x700];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // cmpm.b (a1)+,(a0)+
    rom[0x100..0x102].copy_from_slice(&0xB109u16.to_be_bytes());
    // cmpm.w (a1)+,(a0)+
    rom[0x102..0x104].copy_from_slice(&0xB149u16.to_be_bytes());
    // cmpm.l (a1)+,(a0)+
    rom[0x104..0x106].copy_from_slice(&0xB189u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.a_regs[0] = 0x00FF_0100;
    cpu.a_regs[1] = 0x00FF_0200;
    // byte compare: 0x10 - 0x20 => negative
    memory.write_u8(0x00FF_0100, 0x10);
    memory.write_u8(0x00FF_0200, 0x20);
    // word compare: 0x1234 - 0x1234 => zero
    memory.write_u16(0x00FF_0101, 0x1234);
    memory.write_u16(0x00FF_0201, 0x1234);
    // long compare: 0x00000005 - 0x00000007 => negative
    memory.write_u32(0x00FF_0103, 0x0000_0005);
    memory.write_u32(0x00FF_0203, 0x0000_0007);

    let c1 = cpu.step(&mut memory);
    assert_eq!(c1, 12);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.a_regs[0], 0x00FF_0101);
    assert_eq!(cpu.a_regs[1], 0x00FF_0201);

    let c2 = cpu.step(&mut memory);
    assert_eq!(c2, 12);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.a_regs[0], 0x00FF_0103);
    assert_eq!(cpu.a_regs[1], 0x00FF_0203);

    let c3 = cpu.step(&mut memory);
    assert_eq!(c3, 20);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.a_regs[0], 0x00FF_0107);
    assert_eq!(cpu.a_regs[1], 0x00FF_0207);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn cmpm_byte_on_a7_uses_byte_addr_step() {
    let mut rom = vec![0u8; 0x700];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // cmpm.b (a7)+,(a7)+
    rom[0x100..0x102].copy_from_slice(&0xBF0F_u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.a_regs[7] = 0x00FF_0300;
    memory.write_u8(0x00FF_0300, 0x11);
    memory.write_u8(0x00FF_0302, 0x11);

    cpu.step(&mut memory);
    assert_eq!(cpu.a_regs[7], 0x00FF_0304);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn address_error_on_odd_instruction_fetch_stacks_group0_frame() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    // Start from odd PC to force address error on instruction fetch.
    rom[0x4..0x8].copy_from_slice(&0x0000_0101u32.to_be_bytes());
    // Address error vector.
    rom[0x0C..0x10].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    // Handler body can be anything simple.
    rom[0x200..0x202].copy_from_slice(&0x4E71u16.to_be_bytes()); // nop

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 50);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.exception_histogram.get(&3).copied(), Some(1));

    // 68000 group-0 frame: 7 words.
    let sp = cpu.a7();
    assert_eq!(sp, 0x00FF_1000 - 14);
    assert_eq!(memory.read_u16(sp), 0x0016); // read + instruction + supervisor program FC
    assert_eq!(memory.read_u32(sp + 2), 0x0000_0101); // fault address
    assert_eq!(memory.read_u16(sp + 6), 0x0000); // IR not yet fetched in this path
    assert_eq!(memory.read_u16(sp + 8), 0x2700); // stacked SR
    assert_eq!(memory.read_u32(sp + 10), 0x0000_0101); // stacked PC
}

#[test]
fn address_error_on_misaligned_word_write_marks_data_write_access() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Address error vector.
    rom[0x0C..0x10].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    rom[0x200..0x202].copy_from_slice(&0x4E71u16.to_be_bytes()); // nop

    // move.w #$1234, $00FF0001 (odd destination -> address error)
    rom[0x100..0x102].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x1234u16.to_be_bytes());
    rom[0x104..0x108].copy_from_slice(&0x00FF_0001u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 50);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.exception_histogram.get(&3).copied(), Some(1));

    let sp = cpu.a7();
    assert_eq!(sp, 0x00FF_1000 - 14);
    assert_eq!(memory.read_u16(sp), 0x000D); // write + data + supervisor data FC
    assert_eq!(memory.read_u32(sp + 2), 0x00FF_0001); // fault address
    assert_eq!(memory.read_u16(sp + 6), 0x33FC); // faulting instruction word
    assert_eq!(memory.read_u16(sp + 8), 0x2700); // stacked SR
    assert_eq!(memory.read_u32(sp + 10), 0x0000_0108); // PC after opcode extensions
}

#[test]
fn rte_restores_group0_address_error_stack_frame() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    // Odd PC forces an address-error exception first.
    rom[0x4..0x8].copy_from_slice(&0x0000_0101u32.to_be_bytes());
    // Address error vector.
    rom[0x0C..0x10].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    // Handler: rte
    rom[0x200..0x202].copy_from_slice(&0x4E73u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let fault_cycles = cpu.step(&mut memory);
    assert_eq!(fault_cycles, 50);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.a7(), 0x00FF_1000 - 14);

    let rte_cycles = cpu.step(&mut memory);
    assert_eq!(rte_cycles, 20);
    assert_eq!(cpu.pc(), 0x0000_0101);
    assert_eq!(cpu.sr(), 0x2700);
    assert_eq!(cpu.a7(), 0x00FF_1000);
}

#[test]
fn double_address_error_halts_cpu_until_reset() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    // First instruction fetch from odd PC -> address error.
    rom[0x4..0x8].copy_from_slice(&0x0000_0101u32.to_be_bytes());
    // Address error vector handler at 0x0200.
    rom[0x0C..0x10].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    // Handler body intentionally causes another address error:
    // move.w #$1111, $00FF0001 (odd address write)
    rom[0x200..0x202].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x202..0x204].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x204..0x208].copy_from_slice(&0x00FF_0001u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let first_fault = cpu.step(&mut memory);
    assert_eq!(first_fault, 50);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.exception_histogram.get(&3).copied(), Some(1));

    // Second address error during group-0 handling -> hard halt.
    let second_fault = cpu.step(&mut memory);
    assert_eq!(second_fault, 0);
    assert!(cpu.hard_halted);
    assert_eq!(cpu.exception_histogram.get(&3).copied(), Some(1));

    // Once hard-halted, CPU no longer executes instructions.
    let pc_after_halt = cpu.pc();
    let halted_step = cpu.step(&mut memory);
    assert_eq!(halted_step, 0);
    assert_eq!(cpu.pc(), pc_after_halt);
}

#[test]
fn cpu_reset_recovers_from_double_address_error_halt() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0101u32.to_be_bytes());
    rom[0x0C..0x10].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    // Second fault in handler.
    rom[0x200..0x202].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x202..0x204].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x204..0x208].copy_from_slice(&0x00FF_0001u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // first address error
    cpu.step(&mut memory); // second address error -> hard halt
    assert!(cpu.hard_halted);

    // CPU-level reset acts as external reset and clears hard-halt state.
    cpu.reset(&mut memory);
    assert!(!cpu.hard_halted);
    assert_eq!(cpu.pc(), 0x0000_0101);
}

#[test]
fn representative_exception_and_privileged_opcodes_do_not_fall_back_to_unknown() {
    fn run_case<F>(name: &str, words: &[u16], setup: F)
    where
        F: FnOnce(&mut M68k, &mut MemoryMap),
    {
        let mut rom = vec![0u8; 0x1000];
        rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
        rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

        // Exception vectors used by this test.
        rom[0x10..0x14].copy_from_slice(&0x0000_0300u32.to_be_bytes()); // #4 illegal
        rom[0x18..0x1C].copy_from_slice(&0x0000_0320u32.to_be_bytes()); // #6 CHK
        rom[0x1C..0x20].copy_from_slice(&0x0000_0340u32.to_be_bytes()); // #7 TRAPV
        rom[0x20..0x24].copy_from_slice(&0x0000_0360u32.to_be_bytes()); // #8 privilege
        rom[0x28..0x2C].copy_from_slice(&0x0000_0380u32.to_be_bytes()); // #10 line A
        rom[0x2C..0x30].copy_from_slice(&0x0000_03A0u32.to_be_bytes()); // #11 line F
        rom[0x80..0x84].copy_from_slice(&0x0000_03C0u32.to_be_bytes()); // #32 trap #0
        // Minimal handlers.
        rom[0x300..0x302].copy_from_slice(&0x4E71u16.to_be_bytes());
        rom[0x320..0x322].copy_from_slice(&0x4E71u16.to_be_bytes());
        rom[0x340..0x342].copy_from_slice(&0x4E71u16.to_be_bytes());
        rom[0x360..0x362].copy_from_slice(&0x4E71u16.to_be_bytes());
        rom[0x380..0x382].copy_from_slice(&0x4E71u16.to_be_bytes());
        rom[0x3A0..0x3A2].copy_from_slice(&0x4E71u16.to_be_bytes());
        rom[0x3C0..0x3C2].copy_from_slice(&0x4E71u16.to_be_bytes());

        for (i, word) in words.iter().enumerate() {
            let offset = 0x100 + i * 2;
            rom[offset..offset + 2].copy_from_slice(&word.to_be_bytes());
        }

        let cart = Cartridge::from_bytes(rom).expect("valid rom");
        let mut memory = MemoryMap::new(cart);
        let mut cpu = M68k::new();
        cpu.reset(&mut memory);
        setup(&mut cpu, &mut memory);

        let cycles = cpu.step(&mut memory);
        assert!(
            cycles > 0,
            "{name}: instruction must consume positive cycles"
        );
        assert_eq!(
            cpu.unknown_opcode_total(),
            0,
            "{name}: decode unexpectedly fell back to unknown"
        );
    }

    run_case("reset", &[0x4E70], |_cpu, _memory| {});
    run_case("stop", &[0x4E72, 0x2000], |_cpu, _memory| {});
    run_case("trap_0", &[0x4E40], |_cpu, _memory| {});
    run_case("trapv_clear", &[0x4E76], |_cpu, _memory| {});
    run_case("trapv_set", &[0x4E76], |cpu, _memory| {
        cpu.sr |= CCR_V;
    });
    run_case("rtr", &[0x4E77], |cpu, memory| {
        memory.write_u16(cpu.a_regs[7], 0x0015);
        memory.write_u32(cpu.a_regs[7] + 2, 0x0000_0120);
    });
    run_case("rte", &[0x4E73], |cpu, memory| {
        memory.write_u16(cpu.a_regs[7], 0x2700);
        memory.write_u32(cpu.a_regs[7] + 2, 0x0000_0120);
    });
    run_case("illegal_opcode", &[0x4AFC], |_cpu, _memory| {});
    run_case("bkpt_68000", &[0x4848], |_cpu, _memory| {});
    run_case("line_a", &[0xA000], |_cpu, _memory| {});
    run_case("line_f", &[0xF000], |_cpu, _memory| {});
    run_case("move_to_sr_imm", &[0x46FC, 0x2700], |_cpu, _memory| {});
}
