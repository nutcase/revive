use super::*;

#[test]
fn executes_move_word_immediate_to_absolute_long() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$ABCD, $00FF0002
    rom[0x100..0x102].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0xABCDu16.to_be_bytes());
    rom[0x104..0x108].copy_from_slice(&0x00FF_0002u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 20); // MC68000: MOVE.W #imm,xxx.L = 20 (dest_base 16 + src_ea 4)
    assert_eq!(cpu.pc(), 0x0000_0108);
    assert_eq!(memory.read_u16(0xFF0002), 0xABCD);
}

#[test]
fn executes_move_l_imm_dn_and_move_w_dn_abs_l() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

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
fn move_word_supports_immediate_to_dn_and_displacement_addressing() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0030, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0030u32.to_be_bytes());
    // move.w #$ABCD, d0
    rom[0x106..0x108].copy_from_slice(&0x303Cu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0xABCDu16.to_be_bytes());
    // move.w d0, (2,a0)
    rom[0x10A..0x10C].copy_from_slice(&0x3140u16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x0002u16.to_be_bytes());
    // move.w (2,a0), d1
    rom[0x10E..0x110].copy_from_slice(&0x3228u16.to_be_bytes());
    rom[0x110..0x112].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u16(0x00FF_0032), 0xABCD);
    assert_eq!(cpu.d_regs[0] & 0xFFFF, 0xABCD);
    assert_eq!(cpu.d_regs[1] & 0xFFFF, 0xABCD);
}

#[test]
fn move_word_supports_absolute_word_and_long_sources() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    rom[0x20..0x22].copy_from_slice(&0x2468u16.to_be_bytes());

    // move.w #$1357, $00FF0040
    rom[0x100..0x102].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x1357u16.to_be_bytes());
    rom[0x104..0x108].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w $0020.w, d2
    rom[0x108..0x10A].copy_from_slice(&0x3438u16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x0020u16.to_be_bytes());
    // move.w $00FF0040.l, d3
    rom[0x10C..0x10E].copy_from_slice(&0x3639u16.to_be_bytes());
    rom[0x10E..0x112].copy_from_slice(&0x00FF_0040u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[2] & 0xFFFF, 0x2468);
    assert_eq!(cpu.d_regs[3] & 0xFFFF, 0x1357);
}

#[test]
fn move_long_supports_displacement_source_and_destination() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0060, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0060u32.to_be_bytes());
    // move.l #$11223344, (4,a0)
    rom[0x106..0x108].copy_from_slice(&0x217Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x1122_3344u32.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x0004u16.to_be_bytes());
    // move.l (4,a0), d1
    rom[0x10E..0x110].copy_from_slice(&0x2228u16.to_be_bytes());
    rom[0x110..0x112].copy_from_slice(&0x0004u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u32(0x00FF_0064), 0x1122_3344);
    assert_eq!(cpu.d_regs[1], 0x1122_3344);
    assert_eq!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn executes_move_byte_immediate_to_absolute_long() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

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

#[test]
fn executes_move_byte_with_predecrement_and_postincrement() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0010, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0010u32.to_be_bytes());
    // moveq #$7F, d0
    rom[0x106..0x108].copy_from_slice(&0x707Fu16.to_be_bytes());
    // move.b d0, (a0)+
    rom[0x108..0x10A].copy_from_slice(&0x10C0u16.to_be_bytes());
    // move.b -(a0), d1
    rom[0x10A..0x10C].copy_from_slice(&0x1220u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u8(0x00FF_0010), 0x7F);
    assert_eq!(cpu.d_regs[1] & 0xFF, 0x7F);
    assert_eq!(cpu.a_regs[0], 0x00FF_0010);
}

#[test]
fn move_byte_supports_displacement_absolute_and_immediate_to_register() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0030, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0030u32.to_be_bytes());
    // move.b #$80, d0
    rom[0x106..0x108].copy_from_slice(&0x103Cu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0080u16.to_be_bytes());
    // move.b d0, (2,a0)
    rom[0x10A..0x10C].copy_from_slice(&0x1140u16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x0002u16.to_be_bytes());
    // move.b (2,a0), d1
    rom[0x10E..0x110].copy_from_slice(&0x1228u16.to_be_bytes());
    rom[0x110..0x112].copy_from_slice(&0x0002u16.to_be_bytes());
    // move.b d1, $00FF0034
    rom[0x112..0x114].copy_from_slice(&0x13C1u16.to_be_bytes());
    rom[0x114..0x118].copy_from_slice(&0x00FF_0034u32.to_be_bytes());
    // move.b $00FF0034, d2
    rom[0x118..0x11A].copy_from_slice(&0x1439u16.to_be_bytes());
    rom[0x11A..0x11E].copy_from_slice(&0x00FF_0034u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..7 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u8(0x00FF_0032), 0x80);
    assert_eq!(memory.read_u8(0x00FF_0034), 0x80);
    assert_eq!(cpu.d_regs[1] & 0xFF, 0x80);
    assert_eq!(cpu.d_regs[2] & 0xFF, 0x80);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn move_byte_handles_a7_byte_step_as_two() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0100, a7
    rom[0x100..0x102].copy_from_slice(&0x2E7Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0100u32.to_be_bytes());
    // moveq #$55, d0
    rom[0x106..0x108].copy_from_slice(&0x7055u16.to_be_bytes());
    // move.b d0, -(a7)
    rom[0x108..0x10A].copy_from_slice(&0x1F00u16.to_be_bytes());
    // move.b (a7)+, d1
    rom[0x10A..0x10C].copy_from_slice(&0x121Fu16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u8(0x00FF_00FE), 0x55);
    assert_eq!(cpu.d_regs[1] & 0xFF, 0x55);
    assert_eq!(cpu.a_regs[7], 0x00FF_0100);
}

#[test]
fn executes_movea_adda_and_an_addressing_modes() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0010, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0010u32.to_be_bytes());
    // moveq #2, d0 (keep word accesses aligned)
    rom[0x106..0x108].copy_from_slice(&0x7002u16.to_be_bytes());
    // adda.l d0, a0
    rom[0x108..0x10A].copy_from_slice(&0xD1C0u16.to_be_bytes());
    // move.w d0, (a0)+
    rom[0x10A..0x10C].copy_from_slice(&0x30C0u16.to_be_bytes());
    // move.w d0, -(a0)
    rom[0x10C..0x10E].copy_from_slice(&0x3100u16.to_be_bytes());
    // move.w (a0)+, d1
    rom[0x10E..0x110].copy_from_slice(&0x3218u16.to_be_bytes());
    // move.w -(a0), d2
    rom[0x110..0x112].copy_from_slice(&0x3420u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..7 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u16(0x00FF_0012), 0x0002);
    assert_eq!(cpu.d_regs[1] & 0xFFFF, 0x0002);
    assert_eq!(cpu.d_regs[2] & 0xFFFF, 0x0002);
}

#[test]
fn clr_word_on_data_register_clears_low_word_and_sets_zero_flag() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$12345678, d0
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x1234_5678u32.to_be_bytes());
    // clr.w d0
    rom[0x106..0x108].copy_from_slice(&0x4240u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);

    assert_eq!(cpu.d_regs[0], 0x1234_0000);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn can_write_to_vdp_ports_via_move_sequence() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$4000, $00C00004  ; VDP command high word (VRAM write @0)
    rom[0x100..0x102].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x4000u16.to_be_bytes());
    rom[0x104..0x108].copy_from_slice(&0x00C0_0004u32.to_be_bytes());
    // moveq #0, d0
    rom[0x108..0x10A].copy_from_slice(&0x7000u16.to_be_bytes());
    // move.w d0, $00C00004      ; VDP command low word
    rom[0x10A..0x10C].copy_from_slice(&0x33C0u16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00C0_0004u32.to_be_bytes());
    // move.l #$0000ABCD, d0
    rom[0x110..0x112].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0x0000_ABCDu32.to_be_bytes());
    // move.w d0, $00C00000
    rom[0x116..0x118].copy_from_slice(&0x33C0u16.to_be_bytes());
    rom[0x118..0x11C].copy_from_slice(&0x00C0_0000u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..5 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.vdp().read_vram_u8(0), 0xAB);
    assert_eq!(memory.vdp().read_vram_u8(1), 0xCD);
}

#[test]
fn movea_and_adda_support_absolute_and_postincrement_sources() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$00000010, $00FF0020
    rom[0x100..0x102].copy_from_slice(&0x23FCu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_0010u32.to_be_bytes());
    rom[0x106..0x10A].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // movea.l $00FF0020, a1
    rom[0x10A..0x10C].copy_from_slice(&0x2279u16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00FF_0020u32.to_be_bytes());
    // movea.l #$00FF0030, a0
    rom[0x110..0x112].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0x00FF_0030u32.to_be_bytes());
    // move.w #$0003, $00FF0030
    rom[0x116..0x118].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x118..0x11A].copy_from_slice(&0x0003u16.to_be_bytes());
    rom[0x11A..0x11E].copy_from_slice(&0x00FF_0030u32.to_be_bytes());
    // move.w #$0004, $00FF0032
    rom[0x11E..0x120].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x120..0x122].copy_from_slice(&0x0004u16.to_be_bytes());
    rom[0x122..0x126].copy_from_slice(&0x00FF_0032u32.to_be_bytes());
    // adda.w (a0)+, a1
    rom[0x126..0x128].copy_from_slice(&0xD2D8u16.to_be_bytes());
    // adda.w (a0)+, a1
    rom[0x128..0x12A].copy_from_slice(&0xD2D8u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..8 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.a_regs[1], 0x0000_0017);
    assert_eq!(cpu.a_regs[0], 0x00FF_0034);
}

#[test]
fn move_to_and_from_sr_supports_immediate_register_and_memory() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$271F, sr
    rom[0x100..0x102].copy_from_slice(&0x46FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x271Fu16.to_be_bytes());
    // move.w sr, d0
    rom[0x104..0x106].copy_from_slice(&0x40C0u16.to_be_bytes());
    // move.w sr, $00FF0000
    rom[0x106..0x108].copy_from_slice(&0x40F9u16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0000u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.sr(), 0x271F);
    assert_eq!(cpu.d_regs[0] & 0xFFFF, 0x271F);
    assert_eq!(memory.read_u16(0x00FF_0000), 0x271F);
}

#[test]
fn move_usp_transfers_stack_pointer_with_privileged_opcode() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0200, a1
    rom[0x100..0x102].copy_from_slice(&0x227Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0200u32.to_be_bytes());
    // move a1, usp
    rom[0x106..0x108].copy_from_slice(&0x4E61u16.to_be_bytes());
    // movea.l #0, a1
    rom[0x108..0x10A].copy_from_slice(&0x227Cu16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x0000_0000u32.to_be_bytes());
    // move usp, a1
    rom[0x10E..0x110].copy_from_slice(&0x4E69u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..5 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.unknown_opcode_total(), 0);
    assert_eq!(cpu.usp, 0x00FF_0200);
    assert_eq!(cpu.a_regs[1], 0x00FF_0200);
}

#[test]
fn movem_long_predecrement_and_postincrement_round_trip() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0100, a7
    rom[0x100..0x102].copy_from_slice(&0x2E7Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0100u32.to_be_bytes());
    // move.l #$11223344, d0
    rom[0x106..0x108].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x1122_3344u32.to_be_bytes());
    // movea.l #$55667788, a0
    rom[0x10C..0x10E].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x10E..0x112].copy_from_slice(&0x5566_7788u32.to_be_bytes());
    // movem.l d0/a0, -(a7) ; mask uses predecrement bit ordering
    rom[0x112..0x114].copy_from_slice(&0x48E7u16.to_be_bytes());
    rom[0x114..0x116].copy_from_slice(&0x8080u16.to_be_bytes());
    // moveq #0, d0
    rom[0x116..0x118].copy_from_slice(&0x7000u16.to_be_bytes());
    // movea.l #0, a0
    rom[0x118..0x11A].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x11A..0x11E].copy_from_slice(&0x0000_0000u32.to_be_bytes());
    // movem.l (a7)+, d0/a0
    rom[0x11E..0x120].copy_from_slice(&0x4CDFu16.to_be_bytes());
    rom[0x120..0x122].copy_from_slice(&0x0101u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..7 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0x1122_3344);
    assert_eq!(cpu.a_regs[0], 0x5566_7788);
    assert_eq!(cpu.a_regs[7], 0x00FF_0100);
}

#[test]
fn movem_word_from_memory_sign_extends_registers() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$FF80, $00FF0040
    rom[0x100..0x102].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0xFF80u16.to_be_bytes());
    rom[0x104..0x108].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #$007F, $00FF0042
    rom[0x108..0x10A].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x007Fu16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // movea.l #$00FF0040, a0
    rom[0x110..0x112].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // movem.w (a0), d0-d1
    rom[0x116..0x118].copy_from_slice(&0x4C90u16.to_be_bytes());
    rom[0x118..0x11A].copy_from_slice(&0x0003u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0xFFFF_FF80);
    assert_eq!(cpu.d_regs[1], 0x0000_007F);
}

#[test]
fn pea_pushes_effective_addresses_onto_stack() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0100, a7
    rom[0x100..0x102].copy_from_slice(&0x2E7Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0100u32.to_be_bytes());
    // movea.l #$00FF0200, a0
    rom[0x106..0x108].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0200u32.to_be_bytes());
    // pea (4,a0)
    rom[0x10C..0x10E].copy_from_slice(&0x4868u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0004u16.to_be_bytes());
    // pea $00FF0300.l
    rom[0x110..0x112].copy_from_slice(&0x4879u16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0x00FF_0300u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.a_regs[7], 0x00FF_00F8);
    assert_eq!(memory.read_u32(0x00FF_00F8), 0x00FF_0300);
    assert_eq!(memory.read_u32(0x00FF_00FC), 0x00FF_0204);
}

#[test]
fn clr_byte_clears_register_and_postincrement_memory_destination() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$12345678, d0
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x1234_5678u32.to_be_bytes());
    // clr.b d0
    rom[0x106..0x108].copy_from_slice(&0x4200u16.to_be_bytes());
    // movea.l #$00FF0060, a0
    rom[0x108..0x10A].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0060u32.to_be_bytes());
    // clr.b (a0)+
    rom[0x10E..0x110].copy_from_slice(&0x4218u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u8(0x00FF_0060, 0xAA);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0x1234_5600);
    assert_eq!(memory.read_u8(0x00FF_0060), 0x00);
    assert_eq!(cpu.a_regs[0], 0x00FF_0061);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & (CCR_N | CCR_V | CCR_C), 0);
}

#[test]
fn move_word_supports_an_indexed_source_and_destination() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // moveq #2, d1
    rom[0x106..0x108].copy_from_slice(&0x7202u16.to_be_bytes());
    // move.w (6,a0,d1.w), d0
    rom[0x108..0x10A].copy_from_slice(&0x3030u16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x1006u16.to_be_bytes());
    // clr.b (4,a0,d1.w)
    rom[0x10C..0x10E].copy_from_slice(&0x4230u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x1004u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u16(0x00FF_0048, 0xCAFE);
    memory.write_u8(0x00FF_0046, 0xAA);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0] & 0xFFFF, 0xCAFE);
    assert_eq!(memory.read_u8(0x00FF_0046), 0x00);
}

#[test]
fn move_word_supports_pc_relative_and_pc_indexed_sources() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w (12,pc), d0
    rom[0x100..0x102].copy_from_slice(&0x303Au16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x000Cu16.to_be_bytes());
    // moveq #2, d1
    rom[0x104..0x106].copy_from_slice(&0x7202u16.to_be_bytes());
    // move.w (8,pc,d1.w), d2
    rom[0x106..0x108].copy_from_slice(&0x343Bu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x1008u16.to_be_bytes());
    // nop
    rom[0x10A..0x10C].copy_from_slice(&0x4E71u16.to_be_bytes());
    // data words read by PC-relative modes
    rom[0x10E..0x110].copy_from_slice(&0xBEEFu16.to_be_bytes());
    rom[0x112..0x114].copy_from_slice(&0x1234u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0] & 0xFFFF, 0xBEEF);
    assert_eq!(cpu.d_regs[2] & 0xFFFF, 0x1234);
}

#[test]
fn lea_supports_indexed_an_and_pc_relative_modes() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0100, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0100u32.to_be_bytes());
    // moveq #3, d1
    rom[0x106..0x108].copy_from_slice(&0x7203u16.to_be_bytes());
    // lea (4,a0,d1.w), a2
    rom[0x108..0x10A].copy_from_slice(&0x45F0u16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x1004u16.to_be_bytes());
    // lea (6,pc), a3
    rom[0x10C..0x10E].copy_from_slice(&0x47FAu16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0006u16.to_be_bytes());
    // nop
    rom[0x110..0x112].copy_from_slice(&0x4E71u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..5 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.a_regs[2], 0x00FF_0107);
    assert_eq!(cpu.a_regs[3], 0x0000_0114);
}

#[test]
fn move_to_ccr_updates_condition_code_bits() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$0011, d0
    rom[0x100..0x102].copy_from_slice(&0x303Cu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0011u16.to_be_bytes());
    // move.w d0, ccr
    rom[0x104..0x106].copy_from_slice(&0x44C0u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);

    assert_eq!(cpu.sr() & 0x001F, 0x0011);
}

#[test]
fn movep_word_load_and_store_use_interleaved_bytes() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0000, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0000u32.to_be_bytes());
    // movep.w (0,a0), d3
    rom[0x106..0x108].copy_from_slice(&0x0708u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0000u16.to_be_bytes());
    // movep.w d3, (4,a0)
    rom[0x10A..0x10C].copy_from_slice(&0x0788u16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x0004u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u8(0x00FF_0000, 0x12);
    memory.write_u8(0x00FF_0002, 0x34);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[3] & 0xFFFF, 0x1234);
    assert_eq!(memory.read_u8(0x00FF_0004), 0x12);
    assert_eq!(memory.read_u8(0x00FF_0006), 0x34);
}

#[test]
fn movep_long_load_and_store_use_interleaved_bytes() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0000, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0000u32.to_be_bytes());
    // movep.l (0,a0), d3
    rom[0x106..0x108].copy_from_slice(&0x0748u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0000u16.to_be_bytes());
    // movep.l d3, (8,a0)
    rom[0x10A..0x10C].copy_from_slice(&0x07C8u16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x0008u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u8(0x00FF_0000, 0x11);
    memory.write_u8(0x00FF_0002, 0x22);
    memory.write_u8(0x00FF_0004, 0x33);
    memory.write_u8(0x00FF_0006, 0x44);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..3 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[3], 0x1122_3344);
    assert_eq!(memory.read_u8(0x00FF_0008), 0x11);
    assert_eq!(memory.read_u8(0x00FF_000A), 0x22);
    assert_eq!(memory.read_u8(0x00FF_000C), 0x33);
    assert_eq!(memory.read_u8(0x00FF_000E), 0x44);
}

#[test]
fn move_from_ccr_is_illegal_on_68000() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Illegal instruction vector #4.
    rom[0x10..0x14].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    // move from ccr to d0
    rom[0x100..0x102].copy_from_slice(&0x42C0u16.to_be_bytes());

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
fn exception_entry_clears_trace_bit() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // TRAP #0 vector -> 0x0200.
    rom[0x80..0x84].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    // trap #0
    rom[0x100..0x102].copy_from_slice(&0x4E40u16.to_be_bytes());
    // handler nop
    rom[0x200..0x202].copy_from_slice(&0x4E71u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.sr = 0xA700; // trace set, supervisor set

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 34);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.sr() & 0x8000, 0);
}
