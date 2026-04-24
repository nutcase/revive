use super::super::*;

#[test]
fn mulu_word_with_register_source_updates_result_and_flags() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #6, d0
    rom[0x100..0x102].copy_from_slice(&0x7006u16.to_be_bytes());
    // moveq #7, d1
    rom[0x102..0x104].copy_from_slice(&0x7207u16.to_be_bytes());
    // mulu.w d1, d0
    rom[0x104..0x106].copy_from_slice(&0xC0C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let mul_cycles = cpu.step(&mut memory);

    assert_eq!(cpu.d_regs[0], 42);
    assert_eq!(mul_cycles, 44);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn mulu_word_with_displacement_memory_source_sets_zero() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #0, $00FF0042
    rom[0x106..0x108].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0000u16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // moveq #3, d0
    rom[0x10E..0x110].copy_from_slice(&0x7003u16.to_be_bytes());
    // mulu.w (2,a0), d0
    rom[0x110..0x112].copy_from_slice(&0xC0E8u16.to_be_bytes());
    rom[0x112..0x114].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let mul_cycles = cpu.step(&mut memory);

    assert_eq!(cpu.d_regs[0], 0);
    assert_eq!(mul_cycles, 46);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn mulu_word_cycles_follow_38_plus_2n_rule() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$0000, d1 ; moveq #1, d0 ; mulu.w d1,d0
    rom[0x100..0x102].copy_from_slice(&0x323Cu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0000u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x7001u16.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0xC0C1u16.to_be_bytes());
    // move.w #$FFFF, d1 ; moveq #1, d0 ; mulu.w d1,d0
    rom[0x108..0x10A].copy_from_slice(&0x323Cu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0xFFFFu16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x7001u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0xC0C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c1 = cpu.step(&mut memory);
    assert_eq!(c1, 38);
    assert_eq!(cpu.d_regs[0], 0);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c2 = cpu.step(&mut memory);
    assert_eq!(c2, 70);
    assert_eq!(cpu.d_regs[0], 0x0000_FFFF);
}

#[test]
fn muls_word_with_register_source_handles_negative_result() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #-2, d0
    rom[0x100..0x102].copy_from_slice(&0x70FEu16.to_be_bytes());
    // moveq #3, d1
    rom[0x102..0x104].copy_from_slice(&0x7203u16.to_be_bytes());
    // muls.w d1, d0
    rom[0x104..0x106].copy_from_slice(&0xC1C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let mul_cycles = cpu.step(&mut memory);

    assert_eq!(cpu.d_regs[0], 0xFFFF_FFFA);
    assert_eq!(mul_cycles, 42);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn muls_word_cycles_follow_38_plus_2n_rule() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$0000, d1 ; moveq #2, d0 ; muls.w d1,d0
    rom[0x100..0x102].copy_from_slice(&0x323Cu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0000u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x7002u16.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0xC1C1u16.to_be_bytes());
    // move.w #$5555, d1 ; moveq #1, d0 ; muls.w d1,d0
    rom[0x108..0x10A].copy_from_slice(&0x323Cu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x5555u16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x7001u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0xC1C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c1 = cpu.step(&mut memory);
    assert_eq!(c1, 38);
    assert_eq!(cpu.d_regs[0], 0);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c2 = cpu.step(&mut memory);
    assert_eq!(c2, 70);
    assert_eq!(cpu.d_regs[0], 0x0000_5555);
}

#[test]
fn muls_word_with_memory_source_sets_zero() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.w #-5, $00FF0042
    rom[0x106..0x108].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0xFFFBu16.to_be_bytes());
    rom[0x10A..0x10E].copy_from_slice(&0x00FF_0042u32.to_be_bytes());
    // moveq #0, d0
    rom[0x10E..0x110].copy_from_slice(&0x7000u16.to_be_bytes());
    // muls.w (2,a0), d0
    rom[0x110..0x112].copy_from_slice(&0xC1E8u16.to_be_bytes());
    rom[0x112..0x114].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..5 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0);
    assert_ne!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn divu_word_with_register_source_produces_quotient_and_remainder() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$0001000A, d0
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0001_000Au32.to_be_bytes());
    // moveq #5, d1
    rom[0x106..0x108].copy_from_slice(&0x7205u16.to_be_bytes());
    // divu.w d1, d0
    rom[0x108..0x10A].copy_from_slice(&0x80C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let cycles = cpu.step(&mut memory);

    assert_eq!(cpu.d_regs[0], 0x0001_3335);
    assert_eq!(cycles, 122);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn divu_word_cycles_cover_overflow_and_min_max_paths() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$00010000, d0 ; moveq #1, d1 ; divu.w d1,d0 (overflow => 10 cycles)
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0x7201u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x80C1u16.to_be_bytes());

    // moveq #0, d0 ; moveq #1, d1 ; divu.w d1,d0 (worst-case => 136 cycles)
    rom[0x10A..0x10C].copy_from_slice(&0x7000u16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x7201u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x80C1u16.to_be_bytes());

    // move.l #$FF0000FF, d0 ; move.w #$FF01, d1 ; divu.w d1,d0 (best-case => 76 cycles)
    rom[0x110..0x112].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0xFF00_00FFu32.to_be_bytes());
    rom[0x116..0x118].copy_from_slice(&0x323Cu16.to_be_bytes());
    rom[0x118..0x11A].copy_from_slice(&0xFF01u16.to_be_bytes());
    rom[0x11A..0x11C].copy_from_slice(&0x80C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c_overflow = cpu.step(&mut memory);
    assert_eq!(c_overflow, 10);
    assert_eq!(cpu.d_regs[0], 0x0001_0000);
    assert_ne!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c_worst = cpu.step(&mut memory);
    assert_eq!(c_worst, 136);
    assert_eq!(cpu.d_regs[0], 0);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c_best = cpu.step(&mut memory);
    assert_eq!(c_best, 76);
    assert_eq!(cpu.d_regs[0], 0x0000_FFFF);
    assert_eq!(cpu.sr() & CCR_V, 0);
}

#[test]
fn divu_word_overflow_with_memory_source_adds_ea_cycles() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.l #$00010000, d0
    rom[0x106..0x108].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    // divu.w (2,a0), d0
    rom[0x10C..0x10E].copy_from_slice(&0x80E8u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u16(0x00FF_0042, 0x0001);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let cycles = cpu.step(&mut memory);

    assert_eq!(cycles, 18);
    assert_eq!(cpu.d_regs[0], 0x0001_0000);
    assert_ne!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn divu_by_zero_vectors_to_exception_5() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Divide by zero vector #5
    rom[0x14..0x18].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x7007u16.to_be_bytes()); // moveq #7, d0
    rom[0x102..0x104].copy_from_slice(&0x7200u16.to_be_bytes()); // moveq #0, d1
    rom[0x104..0x106].copy_from_slice(&0x80C1u16.to_be_bytes()); // divu.w d1, d0

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let cycles = cpu.step(&mut memory);

    assert_eq!(cycles, 38);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.a_regs[7], 0x00FF_0FFA);
    assert_eq!(cpu.d_regs[0], 7);
    assert_eq!(memory.read_u32(0x00FF_0FFC), 0x0000_0106);
}

#[test]
fn divu_by_zero_with_memory_source_uses_memory_cycles() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Divide by zero vector #5
    rom[0x14..0x18].copy_from_slice(&0x0000_0200u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.l #$00001234, d0
    rom[0x106..0x108].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x0000_1234u32.to_be_bytes());
    // divu.w (2,a0), d0
    rom[0x10C..0x10E].copy_from_slice(&0x80E8u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u16(0x00FF_0042, 0x0000);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // movea.l
    cpu.step(&mut memory); // move.l
    let cycles = cpu.step(&mut memory); // divu.w (2,a0),d0

    assert_eq!(cycles, 46);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.d_regs[0], 0x0000_1234);
    assert_eq!(memory.read_u32(0x00FF_0FFC), 0x0000_0110);
}

#[test]
fn divs_word_with_register_source_handles_negative_result() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$FFFFFFD8 (-40), d0
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0xFFFF_FFD8u32.to_be_bytes());
    // moveq #6, d1
    rom[0x106..0x108].copy_from_slice(&0x7206u16.to_be_bytes());
    // divs.w d1, d0
    rom[0x108..0x10A].copy_from_slice(&0x81C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let cycles = cpu.step(&mut memory);

    assert_eq!(cpu.d_regs[0], 0xFFFC_FFFA);
    assert_eq!(cycles, 152);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn divs_by_zero_with_memory_source_uses_memory_cycles() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Divide by zero vector #5
    rom[0x14..0x18].copy_from_slice(&0x0000_0200u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // move.l #$FFFFFED4 (-300), d0
    rom[0x106..0x108].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0xFFFF_FED4u32.to_be_bytes());
    // divs.w (2,a0), d0
    rom[0x10C..0x10E].copy_from_slice(&0x81E8u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0002u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u16(0x00FF_0042, 0x0000);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // movea.l
    cpu.step(&mut memory); // move.l
    let cycles = cpu.step(&mut memory); // divs.w (2,a0),d0

    assert_eq!(cycles, 46);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.d_regs[0], 0xFFFF_FED4);
    assert_eq!(memory.read_u32(0x00FF_0FFC), 0x0000_0110);
}

#[test]
fn divs_word_overflow_sets_v_and_keeps_destination() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$00010000, d0
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    // moveq #1, d1
    rom[0x106..0x108].copy_from_slice(&0x7201u16.to_be_bytes());
    // divs.w d1, d0 (overflow: quotient 65536)
    rom[0x108..0x10A].copy_from_slice(&0x81C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let cycles = cpu.step(&mut memory);

    assert_eq!(cpu.d_regs[0], 0x0001_0000);
    assert_eq!(cycles, 16);
    assert_ne!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn divs_word_cycles_cover_long_and_short_paths() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #-1, d0 ; moveq #2, d1 ; divs.w d1,d0 (worst-case long path => 156 cycles)
    rom[0x100..0x102].copy_from_slice(&0x70FFu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x7202u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x81C1u16.to_be_bytes());

    // move.l #$0000FFFF, d0 ; moveq #1, d1 ; divs.w d1,d0 (best-case long path => 120 cycles)
    rom[0x106..0x108].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x0000_FFFFu32.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x7201u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x81C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c_worst = cpu.step(&mut memory);
    assert_eq!(c_worst, 156);
    assert_eq!(cpu.d_regs[0], 0xFFFF_0000);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let c_best = cpu.step(&mut memory);
    assert_eq!(c_best, 120);
    assert_eq!(cpu.d_regs[0], 0x0000_FFFF);
}

#[test]
fn divs_word_negative_absolute_overflow_uses_18_cycles() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.l #$80000000, d0 ; moveq #-1, d1 ; divs.w d1,d0
    rom[0x100..0x102].copy_from_slice(&0x203Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x8000_0000u32.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0x72FFu16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x81C1u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    cpu.step(&mut memory);
    let cycles = cpu.step(&mut memory);

    assert_eq!(cycles, 18);
    assert_eq!(cpu.d_regs[0], 0x8000_0000);
    assert_ne!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}
