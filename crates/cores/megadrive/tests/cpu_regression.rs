use megadrive_core::cartridge::Cartridge;
use megadrive_core::cpu::M68k;
use megadrive_core::memory::MemoryMap;

fn bootable_test_rom(size: usize) -> Vec<u8> {
    let mut rom = vec![0u8; size];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    rom
}

#[test]
fn executes_move_word_immediate_to_absolute_long() {
    let mut rom = bootable_test_rom(0x400);
    // move.w #$ABCD, $00FF0002
    rom[0x100..0x102].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0xABCDu16.to_be_bytes());
    rom[0x104..0x108].copy_from_slice(&0x00FF_0002u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 16);
    assert_eq!(cpu.pc(), 0x0000_0108);
    assert_eq!(memory.read_u16(0xFF0002), 0xABCD);
}

#[test]
fn executes_move_l_imm_dn_and_move_w_dn_abs_l() {
    let mut rom = bootable_test_rom(0x400);
    // move.l #$0000ABCD, d0
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_ABCDu32.to_be_bytes());
    // move.w d0, $00FF0004
    rom[0x106..0x108].copy_from_slice(&0x33C0u16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0004u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);

    assert_eq!(memory.read_u16(0xFF0004), 0xABCD);
    assert_eq!(cpu.pc(), 0x0000_010C);
}

#[test]
fn executes_move_byte_immediate_to_absolute_long() {
    let mut rom = bootable_test_rom(0x400);
    // move.b #$5A, $00FF0003
    rom[0x100..0x102].copy_from_slice(&0x13FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x005Au16.to_be_bytes());
    rom[0x104..0x108].copy_from_slice(&0x00FF_0003u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    assert_eq!(memory.read_u8(0x00FF_0003), 0x5A);
}
