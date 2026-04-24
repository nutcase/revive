use super::*;

#[test]
fn executes_jsr_and_rts() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // jsr $00000120
    rom[0x100..0x102].copy_from_slice(&0x4EB9u16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_0120u32.to_be_bytes());
    // nop
    rom[0x106..0x108].copy_from_slice(&0x4E71u16.to_be_bytes());

    // subroutine: move.w #$BEEF, $00FF0008 ; rts
    rom[0x120..0x122].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x122..0x124].copy_from_slice(&0xBEEFu16.to_be_bytes());
    rom[0x124..0x128].copy_from_slice(&0x00FF_0008u32.to_be_bytes());
    rom[0x128..0x12A].copy_from_slice(&0x4E75u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // jsr
    assert_eq!(cpu.pc(), 0x0000_0120);

    cpu.step(&mut memory); // move.w
    assert_eq!(memory.read_u16(0xFF0008), 0xBEEF);

    cpu.step(&mut memory); // rts
    assert_eq!(cpu.pc(), 0x0000_0106);
}

#[test]
fn executes_jsr_pc_relative_and_rts() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // jsr (18,pc) -> 0x00000114
    rom[0x100..0x102].copy_from_slice(&0x4EBAu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0012u16.to_be_bytes());
    // move.w #$1111, $00FF0000
    rom[0x104..0x106].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0000u32.to_be_bytes());

    // subroutine: move.w #$2222, $00FF0002 ; rts
    rom[0x114..0x116].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x116..0x118].copy_from_slice(&0x2222u16.to_be_bytes());
    rom[0x118..0x11C].copy_from_slice(&0x00FF_0002u32.to_be_bytes());
    rom[0x11C..0x11E].copy_from_slice(&0x4E75u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // jsr (d16,pc)
    assert_eq!(cpu.pc(), 0x0000_0114);

    cpu.step(&mut memory); // subroutine move.w
    assert_eq!(memory.read_u16(0x00FF_0002), 0x2222);

    cpu.step(&mut memory); // rts
    assert_eq!(cpu.pc(), 0x0000_0104);

    cpu.step(&mut memory); // mainline move.w
    assert_eq!(memory.read_u16(0x00FF_0000), 0x1111);
}

#[test]
fn executes_jmp_an_and_pc_relative_modes() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00000120, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x0000_0120u32.to_be_bytes());
    // jmp (a0)
    rom[0x106..0x108].copy_from_slice(&0x4ED0u16.to_be_bytes());
    // move.w #$1111, $00FF0000 (skipped)
    rom[0x108..0x10A].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00FF_0000u32.to_be_bytes());

    // move.w #$2222, $00FF0002
    rom[0x120..0x122].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x122..0x124].copy_from_slice(&0x2222u16.to_be_bytes());
    rom[0x124..0x128].copy_from_slice(&0x00FF_0002u32.to_be_bytes());
    // jmp (10,pc) -> 0x00000134
    rom[0x128..0x12A].copy_from_slice(&0x4EFAu16.to_be_bytes());
    rom[0x12A..0x12C].copy_from_slice(&0x000Au16.to_be_bytes());
    // move.w #$3333, $00FF0004 (skipped)
    rom[0x12C..0x12E].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x12E..0x130].copy_from_slice(&0x3333u16.to_be_bytes());
    rom[0x130..0x134].copy_from_slice(&0x00FF_0004u32.to_be_bytes());
    // move.w #$4444, $00FF0006
    rom[0x134..0x136].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x136..0x138].copy_from_slice(&0x4444u16.to_be_bytes());
    rom[0x138..0x13C].copy_from_slice(&0x00FF_0006u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..5 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u16(0x00FF_0000), 0x0000);
    assert_eq!(memory.read_u16(0x00FF_0002), 0x2222);
    assert_eq!(memory.read_u16(0x00FF_0004), 0x0000);
    assert_eq!(memory.read_u16(0x00FF_0006), 0x4444);
}

#[test]
fn updates_flags_for_cmpi_tst_and_branches_with_bne_beq() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #1, d0
    rom[0x100..0x102].copy_from_slice(&0x7001u16.to_be_bytes());
    // cmpi.w #1, d0   (Z=1)
    rom[0x102..0x104].copy_from_slice(&0x0C40u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x0001u16.to_be_bytes());
    // bne.s +8 (not taken)
    rom[0x106..0x108].copy_from_slice(&0x6608u16.to_be_bytes());
    // tst.w d0 (Z=0)
    rom[0x108..0x10A].copy_from_slice(&0x4A40u16.to_be_bytes());
    // beq.s +8 (not taken)
    rom[0x10A..0x10C].copy_from_slice(&0x6708u16.to_be_bytes());
    // move.w #$1111, $00FF0000
    rom[0x10C..0x10E].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x110..0x114].copy_from_slice(&0x00FF_0000u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..7 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u16(0xFF0000), 0x1111);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
}

#[test]
fn executes_bra_short() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // bra.s -2 (branch to self)
    rom[0x100..0x102].copy_from_slice(&0x60FEu16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.pc(), 0x0000_0100);
}

#[test]
fn executes_bra_word_using_extension_word_base_pc() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // bra.w +0x0A -> 0x0000010C (base PC = 0x00000102)
    rom[0x100..0x102].copy_from_slice(&0x6000u16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x000Au16.to_be_bytes());
    // move.w #$1111, $00FF0000 (skipped)
    rom[0x104..0x106].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0000u32.to_be_bytes());
    // move.w #$2222, $00FF0002
    rom[0x10C..0x10E].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x2222u16.to_be_bytes());
    rom[0x110..0x114].copy_from_slice(&0x00FF_0002u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // bra.w
    assert_eq!(cpu.pc(), 0x0000_010C);

    cpu.step(&mut memory); // move.w #$2222
    assert_eq!(memory.read_u16(0x00FF_0000), 0x0000);
    assert_eq!(memory.read_u16(0x00FF_0002), 0x2222);
}

#[test]
fn executes_bsr_short_and_returns_with_rts() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // bsr.s +0x10 -> 0x00000112
    rom[0x100..0x102].copy_from_slice(&0x6110u16.to_be_bytes());
    // move.w #$1111, $00FF0000
    rom[0x102..0x104].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x106..0x10A].copy_from_slice(&0x00FF_0000u32.to_be_bytes());

    // subroutine: move.w #$2222, $00FF0002 ; rts
    rom[0x112..0x114].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x114..0x116].copy_from_slice(&0x2222u16.to_be_bytes());
    rom[0x116..0x11A].copy_from_slice(&0x00FF_0002u32.to_be_bytes());
    rom[0x11A..0x11C].copy_from_slice(&0x4E75u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let bsr_cycles = cpu.step(&mut memory);
    assert_eq!(bsr_cycles, 18);
    assert_eq!(cpu.pc(), 0x0000_0112);

    cpu.step(&mut memory); // subroutine move.w
    assert_eq!(memory.read_u16(0x00FF_0002), 0x2222);

    cpu.step(&mut memory); // rts
    assert_eq!(cpu.pc(), 0x0000_0102);

    cpu.step(&mut memory); // mainline move.w
    assert_eq!(memory.read_u16(0x00FF_0000), 0x1111);
}

#[test]
fn executes_bsr_word_and_returns_to_post_extension_address() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // bsr.w +0x10 -> 0x00000112 (base PC = 0x00000102)
    rom[0x100..0x102].copy_from_slice(&0x6100u16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0010u16.to_be_bytes());
    // move.w #$1111, $00FF0000
    rom[0x104..0x106].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x106..0x108].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0000u32.to_be_bytes());

    // subroutine: move.w #$2222, $00FF0002 ; rts
    rom[0x112..0x114].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x114..0x116].copy_from_slice(&0x2222u16.to_be_bytes());
    rom[0x116..0x11A].copy_from_slice(&0x00FF_0002u32.to_be_bytes());
    rom[0x11A..0x11C].copy_from_slice(&0x4E75u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // bsr.w
    assert_eq!(cpu.pc(), 0x0000_0112);

    cpu.step(&mut memory); // subroutine move.w
    assert_eq!(memory.read_u16(0x00FF_0002), 0x2222);

    cpu.step(&mut memory); // rts
    assert_eq!(cpu.pc(), 0x0000_0104);

    cpu.step(&mut memory); // mainline move.w
    assert_eq!(memory.read_u16(0x00FF_0000), 0x1111);
}

#[test]
fn executes_bcc_and_bcs_for_taken_and_not_taken_paths() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // cmpi.w #1, d0 (C=1)
    rom[0x102..0x104].copy_from_slice(&0x0C40u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x0001u16.to_be_bytes());
    // bcs.s +8 (taken)
    rom[0x106..0x108].copy_from_slice(&0x6508u16.to_be_bytes());
    // move.w #$1111, $00FF0000 (skipped)
    rom[0x108..0x10A].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00FF_0000u32.to_be_bytes());
    // bcc.s +8 (not taken)
    rom[0x110..0x112].copy_from_slice(&0x6408u16.to_be_bytes());
    // move.w #$2222, $00FF0002
    rom[0x112..0x114].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x114..0x116].copy_from_slice(&0x2222u16.to_be_bytes());
    rom[0x116..0x11A].copy_from_slice(&0x00FF_0002u32.to_be_bytes());
    // moveq #1, d1
    rom[0x11A..0x11C].copy_from_slice(&0x7201u16.to_be_bytes());
    // cmpi.w #1, d1 (C=0)
    rom[0x11C..0x11E].copy_from_slice(&0x0C41u16.to_be_bytes());
    rom[0x11E..0x120].copy_from_slice(&0x0001u16.to_be_bytes());
    // bcc.s +8 (taken)
    rom[0x120..0x122].copy_from_slice(&0x6408u16.to_be_bytes());
    // move.w #$3333, $00FF0004 (skipped)
    rom[0x122..0x124].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x124..0x126].copy_from_slice(&0x3333u16.to_be_bytes());
    rom[0x126..0x12A].copy_from_slice(&0x00FF_0004u32.to_be_bytes());
    // bcs.s +8 (not taken)
    rom[0x12A..0x12C].copy_from_slice(&0x6508u16.to_be_bytes());
    // move.w #$4444, $00FF0006
    rom[0x12C..0x12E].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x12E..0x130].copy_from_slice(&0x4444u16.to_be_bytes());
    rom[0x130..0x134].copy_from_slice(&0x00FF_0006u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..10 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u16(0x00FF_0000), 0x0000);
    assert_eq!(memory.read_u16(0x00FF_0002), 0x2222);
    assert_eq!(memory.read_u16(0x00FF_0004), 0x0000);
    assert_eq!(memory.read_u16(0x00FF_0006), 0x4444);
}

#[test]
fn bcc_cycle_counts_differ_for_short_and_word_not_taken() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // cmpi.w #1, d0 (C=1)
    rom[0x102..0x104].copy_from_slice(&0x0C40u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x0001u16.to_be_bytes());
    // bcc.s +2 (not taken)
    rom[0x106..0x108].copy_from_slice(&0x6402u16.to_be_bytes());
    // bcc.w +2 (not taken)
    rom[0x108..0x10A].copy_from_slice(&0x6400u16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x0002u16.to_be_bytes());
    // bcs.s +2 (taken)
    rom[0x10C..0x10E].copy_from_slice(&0x6502u16.to_be_bytes());
    // nop (skipped by bcs)
    rom[0x10E..0x110].copy_from_slice(&0x4E71u16.to_be_bytes());
    // nop
    rom[0x110..0x112].copy_from_slice(&0x4E71u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // moveq
    cpu.step(&mut memory); // cmpi
    let c1 = cpu.step(&mut memory); // bcc.s not taken
    let c2 = cpu.step(&mut memory); // bcc.w not taken
    let c3 = cpu.step(&mut memory); // bcs.s taken

    assert_eq!(c1, 8);
    assert_eq!(c2, 12);
    assert_eq!(c3, 10);
    assert_eq!(cpu.pc(), 0x0000_0110);
}

#[test]
fn executes_bmi_and_bpl_for_taken_and_not_taken_paths() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // cmpi.w #1, d0 (N=1)
    rom[0x102..0x104].copy_from_slice(&0x0C40u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x0001u16.to_be_bytes());
    // bmi.s +8 (taken)
    rom[0x106..0x108].copy_from_slice(&0x6B08u16.to_be_bytes());
    // move.w #$1111, $00FF0010 (skipped)
    rom[0x108..0x10A].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x10C..0x110].copy_from_slice(&0x00FF_0010u32.to_be_bytes());
    // bpl.s +8 (not taken)
    rom[0x110..0x112].copy_from_slice(&0x6A08u16.to_be_bytes());
    // move.w #$2222, $00FF0012
    rom[0x112..0x114].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x114..0x116].copy_from_slice(&0x2222u16.to_be_bytes());
    rom[0x116..0x11A].copy_from_slice(&0x00FF_0012u32.to_be_bytes());
    // moveq #1, d0
    rom[0x11A..0x11C].copy_from_slice(&0x7001u16.to_be_bytes());
    // tst.w d0 (N=0)
    rom[0x11C..0x11E].copy_from_slice(&0x4A40u16.to_be_bytes());
    // bpl.s +8 (taken)
    rom[0x11E..0x120].copy_from_slice(&0x6A08u16.to_be_bytes());
    // move.w #$3333, $00FF0014 (skipped)
    rom[0x120..0x122].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x122..0x124].copy_from_slice(&0x3333u16.to_be_bytes());
    rom[0x124..0x128].copy_from_slice(&0x00FF_0014u32.to_be_bytes());
    // bmi.s +8 (not taken)
    rom[0x128..0x12A].copy_from_slice(&0x6B08u16.to_be_bytes());
    // move.w #$4444, $00FF0016
    rom[0x12A..0x12C].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x12C..0x12E].copy_from_slice(&0x4444u16.to_be_bytes());
    rom[0x12E..0x132].copy_from_slice(&0x00FF_0016u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..10 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u16(0x00FF_0010), 0x0000);
    assert_eq!(memory.read_u16(0x00FF_0012), 0x2222);
    assert_eq!(memory.read_u16(0x00FF_0014), 0x0000);
    assert_eq!(memory.read_u16(0x00FF_0016), 0x4444);
}

#[test]
fn scc_writes_condition_result_without_modifying_ccr() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0 (Z=1)
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // seq d1
    rom[0x102..0x104].copy_from_slice(&0x57C1u16.to_be_bytes());
    // movea.l #$00FF0040, a0
    rom[0x104..0x106].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x106..0x10A].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // sne (a0) ; Z=1 so writes 0
    rom[0x10A..0x10C].copy_from_slice(&0x56D0u16.to_be_bytes());
    // moveq #1, d2 (Z=0)
    rom[0x10C..0x10E].copy_from_slice(&0x7401u16.to_be_bytes());
    // sne (1,a0) ; Z=0 so writes 0xFF
    rom[0x10E..0x110].copy_from_slice(&0x56E8u16.to_be_bytes());
    rom[0x110..0x112].copy_from_slice(&0x0001u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // moveq #0, d0
    assert_ne!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory); // seq d1
    assert_eq!(cpu.d_regs[1] & 0xFF, 0xFF);
    assert_ne!(cpu.sr() & CCR_Z, 0, "Scc must not change CCR");

    cpu.step(&mut memory); // movea.l
    cpu.step(&mut memory); // sne (a0)
    assert_eq!(memory.read_u8(0x00FF_0040), 0x00);
    assert_ne!(cpu.sr() & CCR_Z, 0, "Scc must not change CCR");

    cpu.step(&mut memory); // moveq #1, d2
    assert_eq!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory); // sne (1,a0)
    assert_eq!(memory.read_u8(0x00FF_0041), 0xFF);
    assert_eq!(cpu.sr() & CCR_Z, 0, "Scc must not change CCR");
}

#[test]
fn dbcc_loops_until_counter_expires_and_skips_decrement_when_condition_true() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #2, d0
    rom[0x100..0x102].copy_from_slice(&0x7002u16.to_be_bytes());
    // moveq #0, d1
    rom[0x102..0x104].copy_from_slice(&0x7200u16.to_be_bytes());
    // addq.b #1, d1
    rom[0x104..0x106].copy_from_slice(&0x5201u16.to_be_bytes());
    // dbf d0, -4 (to addq.b)
    rom[0x106..0x108].copy_from_slice(&0x51C8u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0xFFFCu16.to_be_bytes());
    // moveq #1, d2 (Z=0)
    rom[0x10A..0x10C].copy_from_slice(&0x7401u16.to_be_bytes());
    // dbne d2, +0 (condition true, must not decrement d2)
    rom[0x10C..0x10E].copy_from_slice(&0x56CAu16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0000u16.to_be_bytes());
    // move.w d1, $00FF0000
    rom[0x110..0x112].copy_from_slice(&0x33C1u16.to_be_bytes());
    rom[0x112..0x116].copy_from_slice(&0x00FF_0000u32.to_be_bytes());
    // move.w d2, $00FF0002
    rom[0x116..0x118].copy_from_slice(&0x33C2u16.to_be_bytes());
    rom[0x118..0x11C].copy_from_slice(&0x00FF_0002u32.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..12 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[1] & 0xFF, 0x03);
    assert_eq!(cpu.d_regs[0] & 0xFFFF, 0xFFFF);
    assert_eq!(cpu.d_regs[2] & 0xFFFF, 0x0001);
    assert_eq!(memory.read_u16(0x00FF_0000), 0x0003);
    assert_eq!(memory.read_u16(0x00FF_0002), 0x0001);
}
