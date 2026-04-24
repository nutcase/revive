use super::*;

#[test]
fn and_and_or_ea_to_dn_with_register_source() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #$0F, d0
    rom[0x100..0x102].copy_from_slice(&0x700Fu16.to_be_bytes());
    // moveq #$33, d1
    rom[0x102..0x104].copy_from_slice(&0x7233u16.to_be_bytes());
    // or.b d1, d0
    rom[0x104..0x106].copy_from_slice(&0x8001u16.to_be_bytes());
    // and.b d1, d0
    rom[0x106..0x108].copy_from_slice(&0xC001u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0] & 0xFF, 0x33);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn and_and_or_ea_to_dn_with_displacement_memory_source() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.l #$0F0F00FF, $00FF0044
    rom[0x106..0x108].copy_from_slice(&0x23FCu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x0F0F_00FFu32.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00FF_0044u32.to_be_bytes());
    // move.l #$F0F0FFFF, d0
    rom[0x110..0x112].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0xF0F0_FFFFu32.to_be_bytes());
    // and.l (4,a0), d0
    rom[0x116..0x118].copy_from_slice(&0xC0A8u16.to_be_bytes());
    rom[0x118..0x11A].copy_from_slice(&0x0004u16.to_be_bytes());
    // or.l (4,a0), d0
    rom[0x11A..0x11C].copy_from_slice(&0x80A8u16.to_be_bytes());
    rom[0x11C..0x11E].copy_from_slice(&0x0004u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..5 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0x0F0F_00FF);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn and_and_or_dn_to_ea_with_register_and_memory_destinations() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #$0F, d0
    rom[0x100..0x102].copy_from_slice(&0x700Fu16.to_be_bytes());
    // moveq #$30, d1
    rom[0x102..0x104].copy_from_slice(&0x7230u16.to_be_bytes());
    // or.b d0, d1
    rom[0x104..0x106].copy_from_slice(&0x8101u16.to_be_bytes());
    // and.b d0, d1
    rom[0x106..0x108].copy_from_slice(&0xC101u16.to_be_bytes());
    // movea.l #$00FF0040, a0
    rom[0x108..0x10A].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #$00F0, $00FF0042
    rom[0x10E..0x110].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x110..0x112].copy_from_slice(&0x00F0u16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // or.w d1, (2,a0)
    rom[0x116..0x118].copy_from_slice(&0x8368u16.to_be_bytes());
    rom[0x118..0x11A].copy_from_slice(&0x0002u16.to_be_bytes());
    // and.w d0, (2,a0)
    rom[0x11A..0x11C].copy_from_slice(&0xC168u16.to_be_bytes());
    rom[0x11C..0x11E].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..8 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[1] & 0xFF, 0x0F);
    assert_eq!(memory.read_u16(0x00FF_0042), 0x000F);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn swap_and_ext_transform_register_values_and_flags() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$1234ABCD, d0
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x1234_ABCDu32.to_be_bytes());
    // swap d0
    rom[0x106..0x108].copy_from_slice(&0x4840u16.to_be_bytes());
    // moveq #-128, d1
    rom[0x108..0x10A].copy_from_slice(&0x7280u16.to_be_bytes());
    // ext.w d1
    rom[0x10A..0x10C].copy_from_slice(&0x4881u16.to_be_bytes());
    // ext.l d1
    rom[0x10C..0x10E].copy_from_slice(&0x48C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..5 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0xABCD_1234);
    assert_eq!(cpu.d_regs[1], 0xFFFF_FF80);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn rtr_restores_ccr_and_pc_from_stack() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x4E77u16.to_be_bytes()); // rtr

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    memory.write_u16(cpu.a_regs[7], 0x0015);
    memory.write_u32(cpu.a_regs[7] + 2, 0x0000_0120);
    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 20);
    assert_eq!(cpu.pc(), 0x0000_0120);
    assert_eq!(cpu.sr() & 0x001F, 0x0015);
}
