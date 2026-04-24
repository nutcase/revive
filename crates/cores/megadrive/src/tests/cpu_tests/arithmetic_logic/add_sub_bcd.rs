use super::super::*;

#[test]
fn add_sub_to_dn_accepts_immediate_effective_address() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // add.w #$1234, d0
    rom[0x102..0x104].copy_from_slice(&0xD07Cu16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x1234u16.to_be_bytes());
    // sub.w #$0020, d0
    rom[0x106..0x108].copy_from_slice(&0x907Cu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0020u16.to_be_bytes());
    // add.l #$00010000, d0
    rom[0x10A..0x10C].copy_from_slice(&0xD0BCu16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x0001_0000u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0x0001_1214);
    assert_eq!(cpu.pc(), 0x0000_0110);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn addx_subx_memory_predecrement_mode_updates_memory() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // addx.b -(a0),-(a1)
    rom[0x100..0x102].copy_from_slice(&0xD308u16.to_be_bytes());
    // subx.b -(a0),-(a1)
    rom[0x102..0x104].copy_from_slice(&0x9308u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.a_regs[0] = 0x00FF_0012;
    cpu.a_regs[1] = 0x00FF_0022;
    memory.write_u8(0x00FF_0011, 0x01);
    memory.write_u8(0x00FF_0021, 0x10);
    memory.write_u8(0x00FF_0010, 0x01);
    memory.write_u8(0x00FF_0020, 0x12);
    cpu.sr &= !CCR_X;
    cpu.sr |= CCR_Z;

    cpu.step(&mut memory);
    assert_eq!(memory.read_u8(0x00FF_0021), 0x11);
    assert_eq!(cpu.a_regs[0], 0x00FF_0011);
    assert_eq!(cpu.a_regs[1], 0x00FF_0021);

    cpu.step(&mut memory);
    assert_eq!(memory.read_u8(0x00FF_0020), 0x11);
    assert_eq!(cpu.a_regs[0], 0x00FF_0010);
    assert_eq!(cpu.a_regs[1], 0x00FF_0020);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}
