use super::super::*;

#[test]
fn bit_ops_immediate_and_dynamic_support_register_and_memory_targets() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #0, d0
    rom[0x100..0x102].copy_from_slice(&0x7000u16.to_be_bytes());
    // bset #1, d0
    rom[0x102..0x104].copy_from_slice(&0x08C0u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x0001u16.to_be_bytes());
    // bchg #1, d0
    rom[0x106..0x108].copy_from_slice(&0x0840u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0001u16.to_be_bytes());
    // bclr #2, d0
    rom[0x10A..0x10C].copy_from_slice(&0x0880u16.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0x0002u16.to_be_bytes());
    // moveq #3, d1
    rom[0x10E..0x110].copy_from_slice(&0x7203u16.to_be_bytes());
    // bset d1, d0
    rom[0x110..0x112].copy_from_slice(&0x03C0u16.to_be_bytes());
    // btst d1, d0
    rom[0x112..0x114].copy_from_slice(&0x0300u16.to_be_bytes());
    // movea.l #$00FF0040, a0
    rom[0x114..0x116].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x116..0x11A].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // bset #2, (a0)
    rom[0x11A..0x11C].copy_from_slice(&0x08D0u16.to_be_bytes());
    rom[0x11C..0x11E].copy_from_slice(&0x0002u16.to_be_bytes());
    // bchg d1, (a0)
    rom[0x11E..0x120].copy_from_slice(&0x0350u16.to_be_bytes());
    // btst #2, (a0)
    rom[0x120..0x122].copy_from_slice(&0x0810u16.to_be_bytes());
    rom[0x122..0x124].copy_from_slice(&0x0002u16.to_be_bytes());
    // bclr #3, (a0)
    rom[0x124..0x126].copy_from_slice(&0x0890u16.to_be_bytes());
    rom[0x126..0x128].copy_from_slice(&0x0003u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u8(0x00FF_0040, 0x00);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..12 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0], 0x0000_0008);
    assert_eq!(memory.read_u8(0x00FF_0040), 0x04);
    assert_eq!(cpu.sr() & CCR_Z, 0);
    assert_eq!(cpu.sr() & (CCR_N | CCR_V | CCR_C), 0);
}

#[test]
fn executes_shift_and_rotate_register_forms_used_by_roms() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // moveq #1, d1
    rom[0x100..0x102].copy_from_slice(&0x7201u16.to_be_bytes());
    // ror.b #1, d1  (E219)
    rom[0x102..0x104].copy_from_slice(&0xE219u16.to_be_bytes());
    // moveq #1, d2
    rom[0x104..0x106].copy_from_slice(&0x7401u16.to_be_bytes());
    // rol.l #4, d2  (E99A)
    rom[0x106..0x108].copy_from_slice(&0xE99Au16.to_be_bytes());
    // move.b #$C0, d0
    rom[0x108..0x10A].copy_from_slice(&0x103Cu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x00C0u16.to_be_bytes());
    // lsr.b #6, d0  (EC08)
    rom[0x10C..0x10E].copy_from_slice(&0xEC08u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..6 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[1] & 0xFF, 0x80);
    assert_eq!(cpu.d_regs[2], 0x0000_0010);
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x03);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn executes_roxl_and_roxr_register_forms() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$0010, d7 (set X via move to CCR)
    rom[0x100..0x102].copy_from_slice(&0x3E3Cu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0010u16.to_be_bytes());
    // move.w d7, ccr
    rom[0x104..0x106].copy_from_slice(&0x44C7u16.to_be_bytes());
    // moveq #-128, d0
    rom[0x106..0x108].copy_from_slice(&0x7080u16.to_be_bytes());
    // roxl.b #1, d0
    rom[0x108..0x10A].copy_from_slice(&0xE310u16.to_be_bytes());
    // roxr.b #1, d0
    rom[0x10A..0x10C].copy_from_slice(&0xE210u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..5 {
        cpu.step(&mut memory);
    }

    assert_eq!(cpu.d_regs[0] & 0xFF, 0x80);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn shift_rotate_register_count_zero_uses_68000_flag_rules() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$0010, d7 ; move.w d7, ccr (set X=1)
    rom[0x100..0x102].copy_from_slice(&0x3E3Cu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0010u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x44C7u16.to_be_bytes());
    // moveq #0, d1 (shift count = 0)
    rom[0x106..0x108].copy_from_slice(&0x7200u16.to_be_bytes());
    // move.b #$81, d0
    rom[0x108..0x10A].copy_from_slice(&0x103Cu16.to_be_bytes());
    rom[0x10A..0x10C].copy_from_slice(&0x0081u16.to_be_bytes());
    // asr.b d1,d0 ; roxr.b d1,d0 ; ror.b d1,d0
    rom[0x10C..0x10E].copy_from_slice(&0xE220u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0xE230u16.to_be_bytes());
    rom[0x110..0x112].copy_from_slice(&0xE238u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    cpu.step(&mut memory); // asr.b d1,d0 (count 0)
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x81);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
    assert_eq!(cpu.sr() & CCR_V, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);

    cpu.step(&mut memory); // roxr.b d1,d0 (count 0)
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x81);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);

    cpu.step(&mut memory); // ror.b d1,d0 (count 0)
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x81);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn asl_sets_overflow_when_sign_changes() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.b #$40,d0 ; asl.b #1,d0
    rom[0x100..0x102].copy_from_slice(&0x103Cu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x0040u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0xE300u16.to_be_bytes());

    // movea.l #$00FF0040,a0 ; asl.w (16,a0)
    rom[0x106..0x108].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    rom[0x10C..0x10E].copy_from_slice(&0xE2E8u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0x0010u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    memory.write_u16(0x00FF_0050, 0x4000);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // move.b
    cpu.step(&mut memory); // asl.b #1,d0
    assert_eq!(cpu.d_regs[0] & 0xFF, 0x80);
    assert_ne!(cpu.sr() & CCR_V, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
    assert_eq!(cpu.sr() & CCR_X, 0);

    cpu.step(&mut memory); // movea.l
    cpu.step(&mut memory); // asl.w (16,a0)
    assert_eq!(memory.read_u16(0x00FF_0050), 0x8000);
    assert_ne!(cpu.sr() & CCR_V, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_C, 0);
    assert_eq!(cpu.sr() & CCR_X, 0);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn executes_memory_shift_form_with_displacement_extension_word() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0040, a0
    rom[0x100..0x102].copy_from_slice(&0x207Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0040u32.to_be_bytes());
    // asr.w (16,a0)  (E0E8 0010)
    rom[0x106..0x108].copy_from_slice(&0xE0E8u16.to_be_bytes());
    rom[0x108..0x10A].copy_from_slice(&0x0010u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    memory.write_u16(0x00FF_0050, 0x8001);

    cpu.step(&mut memory); // movea.l
    cpu.step(&mut memory); // asr.w (16,a0)

    assert_eq!(memory.read_u16(0x00FF_0050), 0xC000);
    assert_eq!(cpu.pc(), 0x0000_010A);
    assert_eq!(cpu.unknown_opcode_total(), 0);
    assert_ne!(cpu.sr() & CCR_X, 0);
    assert_ne!(cpu.sr() & CCR_C, 0);
    assert_ne!(cpu.sr() & CCR_N, 0);
    assert_eq!(cpu.sr() & CCR_Z, 0);
}

#[test]
fn write_sr_masks_reserved_bits_on_68000() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // move.w #$FFFF, sr
    rom[0x100..0x102].copy_from_slice(&0x46FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0xFFFFu16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory);
    // 68000 exposes only T,S,IPL and CCR bits in SR.
    assert_eq!(cpu.sr(), 0xA71F);
}

#[test]
fn trace_bit_raises_trace_exception_before_next_instruction() {
    let mut rom = vec![0u8; 0x700];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Trace vector #9 -> 0x0200.
    rom[0x24..0x28].copy_from_slice(&0x0000_0200u32.to_be_bytes());

    // move.w #$A700, sr (set T bit)
    rom[0x100..0x102].copy_from_slice(&0x46FCu16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0xA700u16.to_be_bytes());
    // moveq #1, d0 (must not execute before trace exception)
    rom[0x104..0x106].copy_from_slice(&0x7001u16.to_be_bytes());
    // handler: nop
    rom[0x200..0x202].copy_from_slice(&0x4E71u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    cpu.step(&mut memory); // move.w to sr
    assert_eq!(cpu.pc(), 0x0000_0104);
    assert_eq!(cpu.d_regs[0], 0);

    let trace_cycles = cpu.step(&mut memory); // pending trace exception
    assert_eq!(trace_cycles, 34);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!(cpu.d_regs[0], 0);
    assert_eq!(cpu.exception_histogram.get(&9).copied(), Some(1));
}
