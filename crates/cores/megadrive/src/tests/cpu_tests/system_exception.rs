use super::*;

#[test]
fn services_vdp_level6_interrupt_when_unmasked() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Autovector level 6
    rom[0x78..0x7C].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x4E71u16.to_be_bytes());
    rom[0x200..0x202].copy_from_slice(&0x4E71u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);
    cpu.sr = SR_SUPERVISOR; // Interrupt mask = 0

    // Register 1 = 0x60 (display + V-INT enable)
    memory.write_u16(0xC00004, 0x8160);
    assert!(memory.step_vdp(127_800));

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 44);
    assert_eq!(cpu.pc(), 0x0000_0200);
    assert_eq!((cpu.sr & SR_INT_MASK) >> 8, 6);
    assert_eq!(cpu.a_regs[7], 0x00FF_0FFA);
}

#[test]
fn trap_and_rte_round_trip_to_handler_and_back() {
    let mut rom = vec![0u8; 0x1200];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // TRAP #1 vector (32 + 1 = 33)
    rom[0x84..0x88].copy_from_slice(&0x0000_0200u32.to_be_bytes());

    // trap #1
    rom[0x100..0x102].copy_from_slice(&0x4E41u16.to_be_bytes());
    // move.w #$1111, $00FF0000
    rom[0x102..0x104].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x1111u16.to_be_bytes());
    rom[0x106..0x10A].copy_from_slice(&0x00FF_0000u32.to_be_bytes());

    // handler: move.w #$2222, $00FF0002 ; rte
    rom[0x200..0x202].copy_from_slice(&0x33FCu16.to_be_bytes());
    rom[0x202..0x204].copy_from_slice(&0x2222u16.to_be_bytes());
    rom[0x204..0x208].copy_from_slice(&0x00FF_0002u32.to_be_bytes());
    rom[0x208..0x20A].copy_from_slice(&0x4E73u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let trap_cycles = cpu.step(&mut memory);
    assert_eq!(trap_cycles, 34);
    assert_eq!(cpu.pc(), 0x0000_0200);

    cpu.step(&mut memory); // handler move.w
    assert_eq!(memory.read_u16(0x00FF_0002), 0x2222);

    let rte_cycles = cpu.step(&mut memory);
    assert_eq!(rte_cycles, 20);
    assert_eq!(cpu.pc(), 0x0000_0102);
    assert_eq!(cpu.sr(), 0x2700);

    cpu.step(&mut memory); // post-trap mainline move.w
    assert_eq!(memory.read_u16(0x00FF_0000), 0x1111);
}

#[test]
fn link_and_unlk_manage_stack_frame() {
    let mut rom = vec![0u8; 0x800];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());

    // movea.l #$00FF0100, a7
    rom[0x100..0x102].copy_from_slice(&0x2E7Cu16.to_be_bytes());
    rom[0x102..0x106].copy_from_slice(&0x00FF_0100u32.to_be_bytes());
    // movea.l #$00FF0200, a6
    rom[0x106..0x108].copy_from_slice(&0x2C7Cu16.to_be_bytes());
    rom[0x108..0x10C].copy_from_slice(&0x00FF_0200u32.to_be_bytes());
    // link a6, #-8
    rom[0x10C..0x10E].copy_from_slice(&0x4E56u16.to_be_bytes());
    rom[0x10E..0x110].copy_from_slice(&0xFFF8u16.to_be_bytes());
    // unlk a6
    rom[0x110..0x112].copy_from_slice(&0x4E5Eu16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    for _ in 0..4 {
        cpu.step(&mut memory);
    }

    assert_eq!(memory.read_u32(0x00FF_00FC), 0x00FF_0200);
    assert_eq!(cpu.a_regs[6], 0x00FF_0200);
    assert_eq!(cpu.a_regs[7], 0x00FF_0100);
}

#[test]
fn illegal_opcode_vectors_to_exception_4() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Illegal instruction vector #4
    rom[0x10..0x14].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x4AFCu16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 34);
    assert_eq!(cpu.pc(), 0x0000_0180);
    assert_eq!(cpu.a_regs[7], 0x00FF_0FFA);
    assert_eq!(memory.read_u16(0x00FF_0FFA), 0x2700);
    assert_eq!(memory.read_u32(0x00FF_0FFC), 0x0000_0102);
}

#[test]
fn trapv_vectors_only_when_overflow_is_set() {
    let mut rom = vec![0u8; 0x400];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Trap #7 vector.
    rom[0x1C..0x20].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x4E76u16.to_be_bytes()); // trapv

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    // V clear: no trap.
    let cycles_no_trap = cpu.step(&mut memory);
    assert_eq!(cycles_no_trap, 4);
    assert_eq!(cpu.pc(), 0x0000_0102);

    cpu.pc = 0x0000_0100;
    cpu.sr |= CCR_V;
    let cycles_trap = cpu.step(&mut memory);
    assert_eq!(cycles_trap, 34);
    assert_eq!(cpu.pc(), 0x0000_0180);
}

#[test]
fn reset_requires_supervisor_mode() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Privilege violation vector #8
    rom[0x20..0x24].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x4E70u16.to_be_bytes()); // reset

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let sup_cycles = cpu.step(&mut memory);
    assert_eq!(sup_cycles, 132);
    assert_eq!(cpu.pc(), 0x0000_0102);

    cpu.pc = 0x0000_0100;
    cpu.sr &= !SR_SUPERVISOR;
    let user_cycles = cpu.step(&mut memory);
    assert_eq!(user_cycles, 34);
    assert_eq!(cpu.pc(), 0x0000_0180);
}

#[test]
fn reset_instruction_pulses_external_reset_line() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0x4E70u16.to_be_bytes()); // reset

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    // Run Z80 first so we can verify RESET drives it back to initial state.
    memory.write_u16(0xA11200, 0x0100); // release reset
    memory.write_u16(0xA11100, 0x0000); // bus owned by Z80
    memory.step_subsystems(64);
    assert!(memory.z80().pc() > 0);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 132);
    assert_eq!(memory.z80().read_reset_byte(), 0x01);
    assert_eq!(memory.z80().pc(), 0);
}

#[test]
fn line_a_and_line_f_vector_to_10_and_11() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // vector 10 @ 0x28
    rom[0x28..0x2C].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    // vector 11 @ 0x2C
    rom[0x2C..0x30].copy_from_slice(&0x0000_01A0u32.to_be_bytes());
    rom[0x100..0x102].copy_from_slice(&0xA000u16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0xF000u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let c1 = cpu.step(&mut memory);
    assert_eq!(c1, 34);
    assert_eq!(cpu.pc(), 0x0000_0180);

    cpu.pc = 0x0000_0102;
    cpu.a_regs[7] = cpu.ssp;
    let c2 = cpu.step(&mut memory);
    assert_eq!(c2, 34);
    assert_eq!(cpu.pc(), 0x0000_01A0);
}

#[test]
fn bkpt_on_68000_behaves_like_illegal() {
    let mut rom = vec![0u8; 0x500];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // illegal vector #4
    rom[0x10..0x14].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    // bkpt #0
    rom[0x100..0x102].copy_from_slice(&0x4848u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let cycles = cpu.step(&mut memory);
    assert_eq!(cycles, 34);
    assert_eq!(cpu.pc(), 0x0000_0180);
    assert_eq!(cpu.unknown_opcode_total(), 0);
}

#[test]
fn stop_halts_fetch_until_interrupt() {
    let mut rom = vec![0u8; 0x600];
    rom[0x0..0x4].copy_from_slice(&0x00FF_1000u32.to_be_bytes());
    rom[0x4..0x8].copy_from_slice(&0x0000_0100u32.to_be_bytes());
    // Level-6 autovector
    rom[0x78..0x7C].copy_from_slice(&0x0000_0180u32.to_be_bytes());
    // stop #$2000 ; moveq #1,d0
    rom[0x100..0x102].copy_from_slice(&0x4E72u16.to_be_bytes());
    rom[0x102..0x104].copy_from_slice(&0x2000u16.to_be_bytes());
    rom[0x104..0x106].copy_from_slice(&0x7001u16.to_be_bytes());

    let cart = Cartridge::from_bytes(rom).expect("valid rom");
    let mut memory = MemoryMap::new(cart);
    let mut cpu = M68k::new();
    cpu.reset(&mut memory);

    let stop_cycles = cpu.step(&mut memory);
    assert_eq!(stop_cycles, 4);
    assert_eq!(cpu.pc(), 0x0000_0104);

    // Still stopped: PC does not advance.
    let idle_cycles = cpu.step(&mut memory);
    assert_eq!(idle_cycles, 4);
    assert_eq!(cpu.pc(), 0x0000_0104);
    assert_eq!(cpu.d_regs[0], 0);

    // Raise VINT level 6 and ensure STOP is released by interrupt service.
    memory.write_u16(0xC00004, 0x8160); // display+vint enable
    memory.step_vdp(127_800);
    let int_cycles = cpu.step(&mut memory);
    assert_eq!(int_cycles, 44);
    assert_eq!(cpu.pc(), 0x0000_0180);
}
