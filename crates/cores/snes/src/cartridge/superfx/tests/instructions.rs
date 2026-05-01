use super::*;

#[test]
fn and_opcode_uses_plain_and_without_alt1() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 8;
    gsu.dst_reg = 8;
    gsu.regs[8] = 0x00F3;
    gsu.regs[7] = 0x00CC;

    assert!(gsu.execute_opcode_internal(0x77, &rom, 0x8000, false));
    assert_eq!(gsu.regs[8], 0x00C0);
}

#[test]
fn bic_opcode_is_selected_by_alt1() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 8;
    gsu.dst_reg = 8;
    gsu.regs[8] = 0x00F3;
    gsu.regs[7] = 0x00CC;
    gsu.sfr |= SFR_ALT1_BIT;

    assert!(gsu.execute_opcode_internal(0x77, &rom, 0x8000, false));
    assert_eq!(gsu.regs[8], 0x0033);
}

#[test]
fn fmult_writes_upper_product_word_and_sets_carry_from_lower_word() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 5;
    gsu.dst_reg = 2;
    gsu.regs[5] = 0x4AAA;
    gsu.regs[6] = 0xDAAB;

    assert!(gsu.execute_opcode_internal(0x9F, &rom, 0x8000, false));
    // FMULT: (product << 1) >> 16. product = 0xF51CA38E
    // (0xF51CA38E << 1) = 0xEA39471C >> 16 = 0xEA39
    // FMULT: product >> 16 (per snes9x/ares)
    assert_eq!(gsu.regs[2], 0xF51C);
    assert_ne!(gsu.sfr & SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
    assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
}

#[test]
fn lmult_writes_upper_product_word_and_keeps_low_word_in_r4() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 9;
    gsu.dst_reg = 8;
    gsu.regs[9] = 0xB556;
    gsu.regs[6] = 0xDAAB;
    gsu.sfr |= SFR_ALT1_BIT;

    assert!(gsu.execute_opcode_internal(0x9F, &rom, 0x8000, false));
    assert_eq!(gsu.regs[8], 0x0AE3);
    assert_eq!(gsu.regs[4], 0x5C72);
    assert_eq!(gsu.sfr & SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
    assert_eq!(gsu.sfr & SFR_CY_BIT, 0);
}

#[test]
fn iwt_r15_works_for_ff_opcode() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0xFF;
    rom[0x0001] = 0x34;
    rom[0x0002] = 0x92;
    rom[0x0003] = 0x01;
    rom[0x1236] = 0x00;

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;
    gsu.run_steps(&rom, 9);

    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x9236);
}

#[test]
fn branch_run_steps_executes_delay_slot_using_target_stream_for_immediate_fetch() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x05; // BRA
    rom[0x0001] = 0x03; // target = 0x8005 after delay-slot prefetch
    rom[0x0002] = 0xAC; // delay slot: IBT R12, #imm
    rom[0x0003] = 0x11; // should be ignored
    rom[0x0005] = 0x22; // target-stream immediate
    rom[0x0006] = 0x00; // STOP at target

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 4);

    assert_eq!(gsu.regs[12], 0x0022);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x8008);
}

#[test]
fn jmp_r11_run_steps_executes_delay_slot_using_target_stream_for_immediate_fetch() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x9B; // JMP R11
    rom[0x0001] = 0xAC; // delay slot: IBT R12, #imm
    rom[0x0002] = 0x11; // should be ignored
    rom[0x1234] = 0x22; // target-stream immediate
    rom[0x1235] = 0x00; // STOP at target fallthrough

    gsu.pbr = 0x00;
    gsu.regs[11] = 0x9234;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 4);

    assert_eq!(gsu.regs[12], 0x0022);
    assert!(!gsu.running());
    assert_eq!(
        gsu.debug_recent_pc_transfers()
            .last()
            .map(|t| (t.opcode, t.from_pc, t.to_pc)),
        Some((0x9B, 0x8000, 0x9234))
    );
}

#[test]
fn iwt_r15_run_steps_executes_delay_slot_before_transfer() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0xFF; // IWT R15, $9234
    rom[0x0001] = 0x34;
    rom[0x0002] = 0x92;
    rom[0x0003] = 0xD0; // delay slot: INC R0
    rom[0x1234] = 0x00; // STOP at target

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 4);

    assert_eq!(gsu.regs[0], 0x0001);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x9236);
}

#[test]
fn with_r15_move_to_r8_uses_logical_execution_time_r15() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x2F; // WITH R15
    rom[0x0001] = 0x18; // MOVE R8, R15
    rom[0x0002] = 0x00; // STOP

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 4);

    // snes9x FETCHPIPE executes MOVE with R15 pointing at the next byte
    // already in the pipe, not one byte past it.
    assert_eq!(gsu.regs[8], 0x8002);
    assert!(!gsu.running());
}

#[test]
fn to_b_form_resets_selectors_before_following_opcode() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x27; // WITH R7
    rom[0x0001] = 0x1D; // TO R13 (B-form copy from R7)
    rom[0x0002] = 0x69; // SUB R9 -> must fall back to default R0 after CLRFLAGS
    rom[0x0003] = 0x00; // STOP

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.regs[0] = 0x1234;
    gsu.regs[7] = 0x0003;
    gsu.regs[9] = 0x0001;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 4);

    assert_eq!(gsu.regs[13], 0x0003);
    assert_eq!(gsu.regs[7], 0x0003);
    assert_eq!(gsu.regs[0], 0x1233);
    assert!(!gsu.running());
}

#[test]
fn to_r15_getb_run_steps_switches_immediately_to_target_stream() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x1F; // TO R15
    rom[0x0001] = 0xEF; // GETB -> R15
    rom[0x0002] = 0xD0; // one stale old-stream byte still executes
    rom[0x0003] = 0xD1; // but execution must not skip ahead to this byte
    rom[0x0004] = 0x05; // ROM data byte read by GETB
    rom[0x0005] = 0x00; // target stream STOP
    rom[0x0006] = 0x01; // target+1: NOP

    gsu.pbr = 0x00;
    gsu.rombr = 0x00;
    gsu.regs[14] = 0x8004;
    gsu.rom_buffer_pending = true;
    gsu.rom_buffer_valid = false;
    gsu.rom_buffer_pending_bank = 0x00;
    gsu.rom_buffer_pending_addr = 0x8004;
    gsu.cbr = 0x8000;
    gsu.cache_enabled = true;
    gsu.cache_valid_mask = u32::MAX;
    gsu.cache_ram[0x0000] = 0x1F; // TO R15
    gsu.cache_ram[0x0001] = 0xEF; // GETB -> R15
    gsu.cache_ram[0x0002] = 0xD0; // stale old-stream byte still executes
    gsu.cache_ram[0x0003] = 0xD1; // but execution must not advance to this byte
    gsu.cache_ram[0x0005] = 0x00; // STOP at target stream
    gsu.cache_ram[0x0006] = 0x01; // NOP after STOP target
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 4);

    // FX_STEP fetches one sequential byte before GETB writes R15, so the
    // immediate next old-stream byte executes once. Execution must then
    // resume from the target stream, not from the second old-stream byte.
    assert_eq!(gsu.regs[0], 0x0001);
    assert_eq!(gsu.regs[1], 0x0000);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x0007);
}

#[test]
fn run_steps_keeps_gsu_running_when_slice_budget_is_exhausted() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0xD0;
    rom[0x0001] = 0xD0;
    rom[0x0002] = 0x00;

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 2);
    assert!(gsu.running());
    assert_ne!(gsu.read_register(0x3030, 0) & (SFR_GO_BIT as u8), 0);
    assert_eq!(gsu.regs[15], 0x8003);

    gsu.run_steps(&rom, 2);
    assert!(!gsu.running());
    assert_eq!(gsu.read_register(0x3030, 0) & (SFR_GO_BIT as u8), 0);
}

#[test]
fn loop_decrements_full_r12_and_branches_when_nonzero() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01;
    gsu.regs[12] = 0xFF02;
    gsu.regs[13] = 0x1234;
    gsu.regs[15] = 0x8001;

    assert!(gsu.execute_opcode_internal(0x3C, &rom, 0x0000, false));
    assert_eq!(gsu.regs[12], 0xFF01);
    assert_eq!(gsu.regs[15], 0x1234);
}

#[test]
fn loop_stops_when_r12_reaches_zero() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01; // delay slot NOP
    gsu.regs[12] = 0x0001;
    gsu.regs[13] = 0x1234;
    gsu.regs[15] = 0x8001;

    assert!(gsu.execute_opcode_internal(0x3C, &rom, 0x0000, false));
    assert_eq!(gsu.regs[12], 0x0000);
    assert_eq!(gsu.regs[15], 0x8001);
    assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
}

#[test]
fn loop_clears_prefix_flags() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.regs[12] = 0x0002;
    gsu.regs[13] = 0x1234;
    gsu.regs[15] = 0x8001;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_B_BIT;

    assert!(gsu.execute_opcode_internal(0x3C, &rom, 0x0000, false));
    assert_eq!(gsu.regs[15], 0x1234);
    assert_eq!(gsu.sfr & super::SFR_ALT1_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
}

#[test]
fn loop_run_steps_executes_prefetched_delay_slot_when_branching() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x3C; // LOOP
    rom[0x0001] = 0xD0; // delay slot: INC R0
    rom[0x0002] = 0x00; // STOP

    gsu.pbr = 0x00;
    gsu.regs[12] = 0x0002;
    gsu.regs[13] = 0x8000;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 2);

    assert_eq!(gsu.regs[0], 0x0001);
    assert_eq!(gsu.regs[12], 0x0001);
    assert!(gsu.running());
    assert_eq!(gsu.regs[15], 0x8001);
}

#[test]
fn branch_taken_uses_opcode_pc_plus_one_as_base() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01;
    gsu.regs[15] = 0x8001;

    assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
    assert_eq!(gsu.regs[15], 0x8003);
}

#[test]
fn branch_preserves_with_and_alt_prefix_state() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01;
    gsu.regs[15] = 0x8001;
    gsu.src_reg = 6;
    gsu.dst_reg = 7;
    gsu.with_reg = 5;
    gsu.sfr |= SFR_B_BIT | SFR_ALT1_BIT | super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
    assert_eq!(gsu.regs[15], 0x8003);
    assert_eq!(gsu.src_reg, 6);
    assert_eq!(gsu.dst_reg, 7);
    assert_eq!(gsu.with_reg, 5);
    assert_ne!(gsu.sfr & SFR_B_BIT, 0);
    assert_ne!(gsu.sfr & SFR_ALT1_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_ALT2_BIT, 0);
}

#[test]
fn blt_takes_branch_when_sign_and_overflow_match() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01;
    gsu.regs[15] = 0x8001;
    gsu.sfr = SFR_S_BIT | SFR_OV_BIT;
    gsu.sync_condition_flags_from_sfr();

    assert!(gsu.execute_opcode_internal(0x06, &rom, 0x8000, false));
    assert_eq!(gsu.regs[15], 0x8003);
}

#[test]
fn bge_takes_branch_when_sign_and_overflow_differ() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01;
    gsu.regs[15] = 0x8001;
    gsu.sfr = SFR_S_BIT;
    gsu.sync_condition_flags_from_sfr();

    assert!(gsu.execute_opcode_internal(0x07, &rom, 0x8000, false));
    assert_eq!(gsu.regs[15], 0x8003);
}

#[test]
fn branch_does_not_execute_delay_slot() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01;
    rom[0x0002] = 0xD0;
    gsu.regs[0] = 0x1234;
    gsu.regs[15] = 0x8001;

    assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
    assert_eq!(gsu.regs[0], 0x1234);
    assert_eq!(gsu.regs[15], 0x8003);
}

#[test]
fn branch_taken_preserves_with_prefix_for_target_instruction() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    gsu.regs[0] = 0x1234;
    gsu.regs[2] = 0xABCD;
    gsu.src_reg = 0;
    gsu.dst_reg = 0;
    gsu.sfr |= SFR_B_BIT;
    gsu.regs[15] = 0x8001;
    rom[0x0001] = 0x01; // BRA +1

    assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
    assert_ne!(gsu.sfr & SFR_B_BIT, 0);

    assert!(gsu.execute_opcode_internal(0x12, &rom, 0x8002, false));

    assert_eq!(gsu.regs[2], 0x1234);
    assert_eq!(gsu.sfr & SFR_B_BIT, 0);
}

#[test]
fn branch_not_taken_preserves_with_prefix_for_fallthrough_instruction() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    gsu.regs[0] = 0x1234;
    gsu.regs[2] = 0xABCD;
    gsu.src_reg = 0;
    gsu.dst_reg = 0;
    gsu.sfr |= SFR_B_BIT;
    gsu.regs[15] = 0x8001;
    rom[0x0001] = 0x00; // BEQ +0 (not taken because Z is clear)

    assert!(gsu.execute_opcode_internal(0x09, &rom, 0x8000, false));
    assert_ne!(gsu.sfr & SFR_B_BIT, 0);

    assert!(gsu.execute_opcode_internal(0x12, &rom, 0x8002, false));

    assert_eq!(gsu.regs[2], 0x1234);
    assert_eq!(gsu.sfr & SFR_B_BIT, 0);
}

#[test]
fn branch_taken_preserves_alt_prefix_for_target_instruction() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    gsu.sfr |= SFR_ALT1_BIT;
    gsu.regs[15] = 0x8001;
    rom[0x0001] = 0x01; // BRA +1
    rom[0x0003] = 0x12; // immediate for IBT
    gsu.write_ram_word_short(0x24, 0xBEEF);

    assert!(gsu.execute_opcode_internal(0x05, &rom, 0x8000, false));
    assert_eq!(gsu.alt_mode(), 1);

    gsu.regs[15] = 0x8003;
    assert!(gsu.execute_opcode_internal(0xA0, &rom, 0x8002, false));

    assert_eq!(gsu.regs[0], 0xBEEF);
}

#[test]
fn rol_uses_carry_in() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 4;
    gsu.dst_reg = 4;
    gsu.regs[4] = 0x0160;
    gsu.sfr |= SFR_CY_BIT;
    gsu.sync_condition_flags_from_sfr();

    assert!(gsu.execute_opcode_internal(0x04, &rom, 0x0000, false));
    assert_eq!(gsu.regs[4], 0x02C1);
}

#[test]
fn div2_alt1_turns_negative_one_into_zero() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 4;
    gsu.dst_reg = 4;
    gsu.regs[4] = 0xFFFF;
    gsu.sfr |= SFR_ALT1_BIT;

    assert!(gsu.execute_opcode_internal(0x96, &rom, 0x0000, false));
    assert_eq!(gsu.regs[4], 0x0000);
    assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
}

#[test]
fn sub_carry_uses_unsigned_16bit_diff_rule() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 4;
    gsu.dst_reg = 4;
    gsu.regs[4] = 0x8000;
    gsu.regs[7] = 0x0001;

    assert!(gsu.execute_opcode_internal(0x67, &rom, 0x0000, false));
    assert_eq!(gsu.regs[4], 0x7FFF);
    assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
}

#[test]
fn link_four_sets_r11_to_return_after_delayed_jump_sequence() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[15] = 0xB33B;

    assert!(gsu.execute_opcode(0x94, &[], 0xB33A));
    assert_eq!(gsu.regs[11], 0xB33F);
}

#[test]
fn jmp_9b_targets_r11() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01;
    gsu.regs[11] = 0x3456;
    gsu.regs[9] = 0x1234;
    gsu.regs[15] = 0x8001;

    assert!(gsu.execute_opcode_internal(0x9B, &rom, 0x8000, false));
    assert_eq!(gsu.regs[15], 0x3456);
}

#[test]
fn iwt_r15_records_pc_transfer_history() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x34;
    rom[0x0001] = 0x92;
    gsu.regs[15] = 0x8000;

    assert!(gsu.execute_opcode_internal(0xFF, &rom, 0x8000, false));
    let transfer = gsu.debug_recent_pc_transfers().last().unwrap();
    assert_eq!(transfer.opcode, 0xFF);
    assert_eq!(transfer.from_pc, 0x8000);
    assert_eq!(transfer.to_pc, 0x9234);
}

#[test]
fn jmp_9b_records_pc_transfer_history() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x01;
    gsu.regs[11] = 0x3456;
    gsu.regs[15] = 0x8001;

    assert!(gsu.execute_opcode_internal(0x9B, &rom, 0x8000, false));
    let transfer = gsu.debug_recent_pc_transfers().last().unwrap();
    assert_eq!(transfer.opcode, 0x9B);
    assert_eq!(transfer.from_pc, 0x8000);
    assert_eq!(transfer.to_pc, 0x3456);
}

#[test]
fn reg15_read_uses_written_value_when_modified_under_pipe() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pipe_valid = true;
    gsu.regs[15] = 0x8002;
    gsu.r15_modified = false;
    assert_eq!(gsu.reg(15), 0x8002);

    gsu.write_reg(15, 0x9234);
    assert_eq!(gsu.reg(15), 0x9234);
}

#[test]
fn sm_uses_opcode_register_not_source_selector() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0002] = 0x12;

    gsu.src_reg = 1;
    gsu.regs[1] = 0x1111;
    gsu.regs[6] = 0xBEEF;
    gsu.sfr |= super::SFR_ALT2_BIT;
    gsu.pipe = 0x34;
    gsu.pipe_valid = true;
    gsu.running = true;
    gsu.regs[15] = 0x8001;
    gsu.cache_enabled = false;

    assert!(gsu.execute_opcode_internal(0xF6, &rom, 0x8000, false));
    assert_eq!(gsu.read_ram_word(0x1234), 0xBEEF);
}

#[test]
fn alt3_f6_uses_lm_not_iwt() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0002] = 0x12;
    gsu.write_ram_word(0x1234, 0xCAFE);
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;
    gsu.pipe = 0x34;
    gsu.pipe_valid = true;
    gsu.running = true;
    gsu.regs[15] = 0x8001;
    gsu.cache_enabled = false;

    assert!(gsu.execute_opcode_internal(0xF6, &rom, 0x8000, false));
    assert_eq!(gsu.regs[6], 0xCAFE);
}

#[test]
fn alt3_96_uses_div2() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 1;
    gsu.dst_reg = 2;
    gsu.regs[1] = 0xFFFF;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode_internal(0x96, &rom, 0x8000, false));
    assert_eq!(gsu.regs[2], 0x0000);
}

#[test]
fn alt3_9b_uses_ljmp() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 1;
    gsu.regs[1] = 0x9234;
    gsu.regs[11] = 0x0005;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode_internal(0x9B, &rom, 0x8000, false));
    assert_eq!(gsu.pbr, 0x05);
    assert_eq!(gsu.regs[15], 0x9234);
}

#[test]
fn alt3_9f_uses_lmult() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 9;
    gsu.dst_reg = 8;
    gsu.regs[9] = 0xB556;
    gsu.regs[6] = 0xDAAB;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode_internal(0x9F, &rom, 0x8000, false));
    assert_eq!(gsu.regs[8], 0x0AE3);
    assert_eq!(gsu.regs[4], 0x5C72);
}

#[test]
fn stw_uses_current_source_register_value() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.src_reg = 13;
    gsu.regs[13] = 0x1234;
    gsu.regs[1] = 0x0010;

    assert!(gsu.execute_opcode(0x31, &[], 0x8000));
    assert_eq!(gsu.game_ram[0x0010], 0x34);
    assert_eq!(gsu.game_ram[0x0011], 0x12);
}

#[test]
fn stb_stores_low_byte_of_source_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.src_reg = 5;
    gsu.regs[5] = 0xABCD;
    gsu.regs[1] = 0x0020;
    gsu.sfr |= super::SFR_ALT1_BIT;

    assert!(gsu.execute_opcode(0x31, &[], 0x8000));
    assert_eq!(gsu.game_ram[0x0020], 0xCD);
    assert_eq!(gsu.game_ram[0x0021], 0x00);
}

#[test]
fn ldw_loads_word_into_destination_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.dst_reg = 7;
    gsu.regs[1] = 0x0010;
    gsu.game_ram[0x0010] = 0x78;
    gsu.game_ram[0x0011] = 0x56;

    assert!(gsu.execute_opcode(0x41, &[], 0x8000));
    assert_eq!(gsu.regs[7], 0x5678);
}

#[test]
fn ldb_zero_extends_byte_into_destination_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.dst_reg = 4;
    gsu.regs[1] = 0x0030;
    gsu.game_ram[0x0030] = 0x9A;
    gsu.sfr |= super::SFR_ALT1_BIT;

    assert!(gsu.execute_opcode(0x41, &[], 0x8000));
    assert_eq!(gsu.regs[4], 0x009A);
}

#[test]
fn cmp_updates_sign_and_zero_flags_without_writing_destination() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.src_reg = 9;
    gsu.dst_reg = 9;
    gsu.regs[9] = 0x0016;
    gsu.regs[1] = 0x0029;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode(0x61, &[], 0x8000));
    assert_eq!(gsu.regs[9], 0x0016);
    assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_CY_BIT, 0);
}

#[test]
fn with_then_to_performs_move() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[3] = 0x1357;
    gsu.src_reg = 9;
    gsu.dst_reg = 1;

    assert!(gsu.execute_opcode(0x23, &[], 0x8000));
    assert_eq!(gsu.debug_src_reg(), 3);
    assert_eq!(gsu.debug_dst_reg(), 3);
    assert!(gsu.execute_opcode(0x11, &[], 0x8001));
    assert_eq!(gsu.regs[3], 0x1357);
    assert_eq!(gsu.regs[1], 0x1357);
    assert_eq!(gsu.debug_src_reg(), 0);
    assert_eq!(gsu.debug_dst_reg(), 0);
}

#[test]
fn with_then_from_performs_move_without_touching_flags() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[2] = 0x0000;
    gsu.dst_reg = 1;
    gsu.sfr |= super::SFR_S_BIT;

    assert!(gsu.execute_opcode(0x21, &[], 0x8000));
    assert!(gsu.execute_opcode(0xB2, &[], 0x8001));
    assert_eq!(gsu.regs[1], 0x0000);
    assert_eq!(gsu.debug_src_reg(), 0);
    assert_eq!(gsu.debug_dst_reg(), 0);
    // MOVES updates sign/zero flags based on the copied value (0 → Z set, S clear)
    assert_ne!(gsu.sfr & super::SFR_Z_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_S_BIT, 0);
}

#[test]
fn moves_sets_overflow_from_low_byte_bit7() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.dst_reg = 1;
    gsu.regs[2] = 0x0080;
    gsu.sfr |= super::SFR_B_BIT;

    assert!(gsu.execute_opcode(0xB2, &[], 0x8000));
    assert_eq!(gsu.regs[1], 0x0080);
    assert_ne!(gsu.sfr & super::SFR_OV_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
}

#[test]
fn moves_zero_flag_tracks_full_word_value() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.dst_reg = 1;
    gsu.regs[2] = 0x0200;
    gsu.sfr |= super::SFR_B_BIT;

    assert!(gsu.execute_opcode(0xB2, &[], 0x8000));
    assert_eq!(gsu.regs[1], 0x0200);
    assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_S_BIT, 0);
}

#[test]
fn with_immediately_updates_source_and_destination_registers() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.src_reg = 5;
    gsu.dst_reg = 6;
    gsu.regs[3] = 0x0001;
    gsu.sfr |= super::SFR_CY_BIT;
    gsu.sync_condition_flags_from_sfr();

    assert!(gsu.execute_opcode(0x23, &[], 0x8000));
    assert_eq!(gsu.debug_src_reg(), 3);
    assert_eq!(gsu.debug_dst_reg(), 3);
    assert!(gsu.execute_opcode(0x04, &[], 0x8001));

    assert_eq!(gsu.regs[3], 0x0003);
    assert_eq!(gsu.regs[6], 0x0000);
    assert_eq!(gsu.debug_src_reg(), 0);
    assert_eq!(gsu.debug_dst_reg(), 0);
    assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
}

#[test]
fn hib_uses_high_byte_for_sign_and_zero_flags() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.src_reg = 4;
    gsu.dst_reg = 5;
    gsu.regs[4] = 0x8001;

    assert!(gsu.execute_opcode(0xC0, &[], 0x8000));
    assert_eq!(gsu.regs[5], 0x0080);
    assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
}

#[test]
fn lob_uses_low_byte_for_sign_and_zero_flags() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.src_reg = 4;
    gsu.dst_reg = 5;
    gsu.regs[4] = 0x0080;

    assert!(gsu.execute_opcode(0x9E, &[], 0x8000));
    assert_eq!(gsu.regs[5], 0x0080);
    assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
}

#[test]
fn with_mode_consumes_selectors_then_resets_them_to_r0() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[3] = 0x0012;
    gsu.regs[0] = 0x0004;

    assert!(gsu.execute_opcode(0x23, &[], 0x8000));
    assert!(gsu.execute_opcode(0xD3, &[], 0x8001));
    assert!(gsu.execute_opcode(0x12, &[], 0x8002));
    assert!(gsu.execute_opcode(0x03, &[], 0x8003));

    assert_eq!(gsu.regs[3], 0x0013);
    assert_eq!(gsu.regs[2], 0x0002);
    assert_eq!(gsu.regs[0], 0x0004);
    assert_eq!(gsu.debug_src_reg(), 0);
    assert_eq!(gsu.debug_dst_reg(), 0);
    assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
}

#[test]
fn to_and_from_prefixes_apply_to_next_opcode_then_reset_to_r0() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[0] = 0x0002;
    gsu.regs[1] = 0x0002;

    assert!(gsu.execute_opcode(0x11, &[], 0x8000));
    assert!(gsu.execute_opcode(0x03, &[], 0x8001));
    assert_eq!(gsu.regs[1], 0x0001);
    assert_eq!(gsu.debug_dst_reg(), 0);

    gsu.regs[1] = 0x0006;
    gsu.regs[0] = 0x0008;
    assert!(gsu.execute_opcode(0xB1, &[], 0x8002));
    assert!(gsu.execute_opcode(0x03, &[], 0x8003));
    assert_eq!(gsu.regs[0], 0x0003);
    assert_eq!(gsu.regs[1], 0x0006);
    assert_eq!(gsu.debug_src_reg(), 0);
    assert_eq!(gsu.debug_dst_reg(), 0);
}

#[test]
fn to_r15_prefix_does_not_skip_past_the_next_prefetched_byte() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[15] = 0x8001;

    assert!(gsu.execute_opcode(0x1F, &[], 0x8000));
    assert_eq!(gsu.debug_dst_reg(), 15);
    assert_eq!(gsu.regs[15], 0x8001);
}

#[test]
fn alt_prefixes_clear_b_but_keep_register_selectors() {
    let mut gsu = SuperFx::new(0x20_0000);

    assert!(gsu.execute_opcode(0x26, &[], 0x8000));
    assert_ne!(gsu.sfr & super::SFR_B_BIT, 0);
    assert_eq!(gsu.debug_src_reg(), 6);
    assert_eq!(gsu.debug_dst_reg(), 6);

    assert!(gsu.execute_opcode(0x3D, &[], 0x8001));
    assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_ALT1_BIT, 0);
    assert_eq!(gsu.debug_src_reg(), 6);
    assert_eq!(gsu.debug_dst_reg(), 6);
}

#[test]
fn alt1_preserves_existing_alt2_bit() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.sfr |= super::SFR_ALT2_BIT | super::SFR_B_BIT;

    assert!(gsu.execute_opcode(0x3D, &[], 0x8000));
    assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_ALT1_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_ALT2_BIT, 0);
    assert_eq!(gsu.alt_mode(), 3);
}

#[test]
fn alt2_preserves_existing_alt1_bit() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_B_BIT;

    assert!(gsu.execute_opcode(0x3E, &[], 0x8000));
    assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_ALT1_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_ALT2_BIT, 0);
    assert_eq!(gsu.alt_mode(), 3);
}

#[test]
fn loop_clears_prefix_flags_and_resets_selectors() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[12] = 0x0002;
    gsu.regs[13] = 0x9000;

    assert!(gsu.execute_opcode(0x24, &[], 0x8000));
    assert_eq!(gsu.debug_src_reg(), 4);
    assert_eq!(gsu.debug_dst_reg(), 4);

    assert!(gsu.execute_opcode(0x3C, &[], 0x8001));
    assert_eq!(gsu.regs[12], 0x0001);
    assert_eq!(gsu.regs[15], 0x9000);
    assert_eq!(gsu.debug_src_reg(), 0);
    assert_eq!(gsu.debug_dst_reg(), 0);
    assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_ALT1_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_ALT2_BIT, 0);
}

#[test]
fn ibt_sign_extends_immediate_when_alt0() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x80;
    gsu.regs[15] = 0x8000;

    assert!(gsu.execute_opcode_internal(0xA3, &rom, 0x0000, false));
    assert_eq!(gsu.regs[3], 0xFF80);
}

#[test]
fn lms_loads_word_from_ram_word_address_immediate() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x12;
    gsu.sfr |= super::SFR_ALT1_BIT;
    gsu.regs[15] = 0x8000;
    gsu.write_ram_word(0x0024, 0xBEEF);

    assert!(gsu.execute_opcode_internal(0xA1, &rom, 0x0000, false));
    assert_eq!(gsu.regs[1], 0xBEEF);
}

#[test]
fn alt3_a0_uses_lms_not_ibt() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x12;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;
    gsu.regs[15] = 0x8000;
    gsu.write_ram_word(0x0024, 0xBEEF);

    assert!(gsu.execute_opcode_internal(0xA1, &rom, 0x0000, false));
    assert_eq!(gsu.regs[1], 0xBEEF);
}

#[test]
fn sms_stores_word_to_ram_word_address_immediate() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x12;
    gsu.sfr |= super::SFR_ALT2_BIT;
    gsu.regs[15] = 0x8000;
    gsu.regs[1] = 0xBEEF;

    assert!(gsu.execute_opcode_internal(0xA1, &rom, 0x0000, false));
    assert_eq!(gsu.read_ram_word(0x0024), 0xBEEF);
}

#[test]
fn ramb_uses_low_two_bits_of_source_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 5;
    gsu.regs[5] = 0x0003;
    gsu.sfr |= super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode_internal(0xDF, &rom, 0x0000, false));
    assert_eq!(gsu.debug_rambr(), 0x03);
}

#[test]
fn plot_writes_snes_tile_bitplanes_into_game_ram() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x00;
    gsu.scbr = 0x00;
    gsu.colr = 0x03;
    gsu.regs[1] = 0;
    gsu.regs[2] = 0;

    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    // Flush pixel caches to write to RAM
    gsu.flush_all_pixel_caches();
    assert_eq!(gsu.game_ram[0], 0x80);
    assert_eq!(gsu.game_ram[1], 0x80);
}

#[test]
fn plot_uses_low_8_bits_of_x_coordinate() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x00;
    gsu.scbr = 0x00;
    gsu.colr = 0x03;
    gsu.regs[1] = 0x0100;
    gsu.regs[2] = 0;

    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    gsu.flush_all_pixel_caches();

    assert_eq!(gsu.game_ram[0], 0x80);
    assert_eq!(gsu.game_ram[1], 0x80);
}

#[test]
fn pixel_cache_flush_writes_leftmost_pixel_to_msb() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x00;
    gsu.scbr = 0x00;
    gsu.pixelcache[0].offset = 0;
    gsu.pixelcache[0].bitpend = 0x80;
    gsu.pixelcache[0].data[7] = 0x01;

    gsu.flush_pixel_cache(0);

    assert_eq!(gsu.game_ram[0], 0x80);
    assert_eq!(gsu.pixelcache[0].bitpend, 0);
}

#[test]
fn mode2_screen_mode_uses_4bpp_layout_like_bsnes() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x02;
    gsu.scbr = 0x00;
    gsu.colr = 0x0F;
    gsu.regs[1] = 0;
    gsu.regs[2] = 0;

    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    gsu.flush_all_pixel_caches();
    assert_eq!(gsu.game_ram[0x00], 0x80);
    assert_eq!(gsu.game_ram[0x01], 0x80);
    assert_eq!(gsu.game_ram[0x10], 0x80);
    assert_eq!(gsu.game_ram[0x11], 0x80);
    assert_eq!(gsu.bits_per_pixel(), Some(4));
}

#[test]
fn height_mode_3_uses_obj_layout_storage() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x24;

    assert_eq!(gsu.screen_height(), Some(256));
    assert_eq!(gsu.screen_buffer_len(), Some(32 * 32 * 16));
    assert_eq!(gsu.tile_pixel_addr(0, 128), Some((0x2000, 0, 7)));
}

#[test]
fn por_obj_mode_does_not_override_screen_height() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x01; // 4bpp, 128-line base
    gsu.por = 0x10; // OBJ flag affects plot/rpix layout, not nominal SCMR geometry

    assert_eq!(gsu.screen_height(), Some(128));
    assert_eq!(gsu.screen_buffer_len(), Some(32 * 32 * 32));
    assert_eq!(gsu.tile_pixel_addr(0, 127), Some((0x1E00, 7, 7)));
    assert_eq!(gsu.tile_pixel_addr(0, 255), Some((0x5E00, 7, 7)));
}

#[test]
fn por_obj_mode_matches_obj_layout_tile_addressing() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x00; // 2bpp, 128-line nominal
    gsu.por = 0x10; // OBJ plot layout

    assert_eq!(gsu.screen_height(), Some(128));
    assert_eq!(gsu.screen_buffer_len(), Some(32 * 32 * 16));
    assert_eq!(gsu.tile_pixel_addr(0, 0), Some((0x0000, 0, 7)));
    assert_eq!(gsu.tile_pixel_addr(128, 0), Some((0x1000, 0, 7)));
    assert_eq!(gsu.tile_pixel_addr(0, 128), Some((0x2000, 0, 7)));
    assert_eq!(gsu.tile_pixel_addr(128, 128), Some((0x3000, 0, 7)));
}

#[test]
fn rpix_preserves_colr_and_returns_pixel_in_destination_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x01;
    gsu.scbr = 0x00;
    gsu.regs[1] = 0;
    gsu.regs[2] = 0;
    gsu.dst_reg = 4;
    gsu.regs[4] = 0x8EBC;
    gsu.colr = 0xC3;
    gsu.sfr |= super::SFR_ALT1_BIT;
    gsu.plot_pixel(0, 0, 0x0A);

    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    assert_eq!(gsu.colr, 0xC3);
    assert_eq!(gsu.regs[4], 0x000A);
}

#[test]
fn cpu_access_is_not_blocked_by_scmr_when_gsu_is_idle() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = super::SCMR_RON_BIT | super::SCMR_RAN_BIT;
    gsu.running = false;

    assert!(gsu.cpu_has_rom_access());
    assert!(gsu.cpu_has_ram_access());
}

#[test]
fn cpu_access_is_blocked_by_scmr_bits_while_gsu_is_running() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = super::SCMR_RON_BIT | super::SCMR_RAN_BIT;
    gsu.running = true;

    assert!(!gsu.cpu_has_rom_access());
    assert!(!gsu.cpu_has_ram_access());
}

#[test]
fn merge_combines_high_bytes_of_r7_and_r8() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 0;
    gsu.dst_reg = 3;
    gsu.regs[7] = 0xAB12;
    gsu.regs[8] = 0xCD34;

    assert!(gsu.execute_opcode_internal(0x70, &rom, 0x8000, false));
    // Result = R7 high (0xAB) << 8 | R8 high (0xCD)
    assert_eq!(gsu.regs[3], 0xABCD);
}

#[test]
fn merge_sets_flags_from_result_bit_masks() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.dst_reg = 0;
    gsu.regs[7] = 0x8000;
    gsu.regs[8] = 0x8000;

    assert!(gsu.execute_opcode_internal(0x70, &rom, 0x8000, false));
    assert_eq!(gsu.regs[0], 0x8080);
    // bsnes: S=(0x8080&0x8080)!=0 → true, Z=(0x8080&0xF0F0)==0 → false
    // CY=(0x8080&0xE0E0)!=0 → true, OV=(0x8080&0xC0C0)!=0 → true
    assert_ne!(gsu.sfr & SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
    assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_OV_BIT, 0);
}

#[test]
fn add_carry_matches_unsigned_reference_for_wraparound_sum() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 1;
    gsu.dst_reg = 2;
    gsu.regs[1] = 0xFFFF;
    gsu.regs[3] = 0x0001;

    assert!(gsu.execute_opcode_internal(0x53, &rom, 0x8000, false));
    assert_eq!(gsu.regs[2], 0x0000);
    assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
    assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
}

#[test]
fn adc_carry_matches_unsigned_reference_for_wraparound_sum() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 4;
    gsu.dst_reg = 5;
    gsu.regs[4] = 0xFFFF;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT | super::SFR_CY_BIT;
    gsu.sync_condition_flags_from_sfr();

    assert!(gsu.execute_opcode_internal(0x50, &rom, 0x8000, false));
    assert_eq!(gsu.regs[5], 0x0000);
    assert_ne!(gsu.sfr & super::SFR_CY_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_Z_BIT, 0);
}

#[test]
fn add_sets_carry_for_later_starfox_feedback_values() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 2;
    gsu.dst_reg = 1;
    gsu.regs[2] = 0x9528;
    gsu.regs[3] = 0xB2A0;

    assert!(gsu.execute_opcode_internal(0x53, &rom, 0x8000, false));
    assert_eq!(gsu.regs[1], 0x47C8);
    assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
}

#[test]
fn sub_carry_matches_unsigned_reference_for_wraparound_diff() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 0;
    gsu.dst_reg = 1;
    gsu.regs[0] = 0xFFFE;
    gsu.regs[4] = 0x7FFF;

    assert!(gsu.execute_opcode_internal(0x64, &rom, 0x8000, false));
    assert_eq!(gsu.regs[1], 0x7FFF);
    assert_ne!(gsu.sfr & super::SFR_CY_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_S_BIT, 0);
}

#[test]
fn b414_helper_builds_r4_and_copies_it_to_r6_on_non_equal_path() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0013].copy_from_slice(&[
        0xA0, 0x03, // IBT R0,#03
        0x3F, // ALT3
        0x64, // CMP R4
        0x09, 0x1F, // BEQ +0x1F (not taken)
        0x01, // NOP
        0xA0, 0x02, // IBT R0,#02
        0x3F, // ALT3
        0x64, // CMP R4
        0x09, 0x09, // BEQ +0x09 (not taken)
        0x01, // NOP
        0x24, // WITH R4
        0x3E, // ALT2
        0x55, // ADD #5 => R4 = 6
        0x24, // WITH R4
        0x16, // TO R6 (B-form) => R6 = 6
    ]);
    gsu.regs[4] = 0x0001;
    gsu.regs[6] = 0x0003;
    gsu.regs[15] = 0x8000;
    gsu.running = true;

    gsu.run_steps(&rom, 15);

    assert_eq!(gsu.regs[4], 0x0006);
    assert_eq!(gsu.regs[6], 0x0006);
    assert_eq!(gsu.debug_src_reg(), 0);
    assert_eq!(gsu.debug_dst_reg(), 0);
    assert_eq!(gsu.sfr & super::SFR_B_BIT, 0);
}

#[test]
fn b37d_helper_builds_r4_r12_and_r13_for_b384_loop() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0008].copy_from_slice(&[
        0x24, // WITH R4
        0x3E, // ALT2
        0x57, // ADD #7 => R4 = 7
        0xAC, 0x08, // IBT R12,#08
        0x2F, // WITH R15
        0x1D, // TO R13 => R13 = R15
        0x22, // WITH R2
    ]);
    gsu.regs[4] = 0x0000;
    gsu.regs[15] = 0x8000;
    gsu.running = true;

    gsu.run_steps(&rom, 8);

    assert_eq!(gsu.regs[4], 0x0007);
    assert_eq!(gsu.regs[12], 0x0008);
    assert_eq!(gsu.regs[13], 0x8007);
}

#[test]
fn merge_flags_when_upper_bits_clear() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.dst_reg = 0;
    gsu.regs[7] = 0x12FF;
    gsu.regs[8] = 0x34FF;

    assert!(gsu.execute_opcode_internal(0x70, &rom, 0x8000, false));
    assert_eq!(gsu.regs[0], 0x1234);
    // bsnes: S=(0x1234&0x8080)!=0 → false, Z=(0x1234&0xF0F0)==0 → false
    // CY=(0x1234&0xE0E0)!=0 → true(0x0020), OV=(0x1234&0xC0C0)!=0 → false
    assert_eq!(gsu.sfr & SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
    assert_ne!(gsu.sfr & SFR_CY_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_OV_BIT, 0);
}

#[test]
fn stw_alt2_falls_back_to_word_store() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.src_reg = 5;
    gsu.regs[5] = 0xBEEF;
    gsu.regs[1] = 0x0040;
    gsu.sfr |= super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode(0x31, &[], 0x8000));
    assert_eq!(gsu.read_ram_word(0x0040), 0xBEEF);
}

#[test]
fn loop_taken_keeps_prefetched_delay_slot_visible() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x31; // STW [R1], R0
    rom[0x0001] = 0xD1; // INC R1
    rom[0x0002] = 0x3C; // LOOP
    rom[0x0003] = 0x00; // STOP

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.regs[13] = 0x8000;
    gsu.regs[12] = 10;
    gsu.regs[1] = 0x0010;
    gsu.regs[0] = 0xA55A;
    gsu.src_reg = 0;
    gsu.running = true;

    gsu.run_steps(&rom, 30);

    assert!(!gsu.running());
    assert_eq!(gsu.regs[12], 9);
    assert_eq!(gsu.regs[1], 0x0011);
    assert_eq!(gsu.game_ram[0x0010], 0x5A);
    assert_eq!(gsu.game_ram[0x0011], 0xA5);
}

#[test]
fn simple_store_inc_loop_fast_path_collapses_taken_iterations() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x31; // STW [R1], R0
    rom[0x0001] = 0xD1; // INC R1
    rom[0x0002] = 0x3C; // LOOP
    rom[0x0003] = 0xD1; // delay slot: INC R1
    rom[0x0004] = 0x00; // STOP

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.regs[13] = 0x8000;
    gsu.regs[12] = 0x0004;
    gsu.regs[1] = 0x0010;
    gsu.regs[0] = 0xA55A;
    gsu.src_reg = 0;
    gsu.running = true;

    assert_eq!(gsu.prime_pipe(&rom), Some(()));
    assert_eq!(gsu.fast_forward_simple_store_inc_loop(&rom, 64), Some(12));

    assert_eq!(gsu.regs[12], 0x0001);
    assert_eq!(gsu.regs[1], 0x0016);
    assert_eq!(gsu.regs[15], 0x8000);
    assert!(!gsu.pipe_valid);
    assert_eq!(gsu.read_ram_word(0x0010), 0xA55A);
    assert_eq!(gsu.read_ram_word(0x0012), 0xA55A);
    assert_eq!(gsu.read_ram_word(0x0014), 0xA55A);
    assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
    assert_eq!(gsu.sfr & SFR_S_BIT, 0);
}

#[test]
fn simple_store_inc_loop_fast_path_rejoins_generic_execution() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x31; // STW [R1], R0
    rom[0x0001] = 0xD1; // INC R1
    rom[0x0002] = 0x3C; // LOOP
    rom[0x0003] = 0xD1; // delay slot: INC R1
    rom[0x0004] = 0x00; // STOP

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.regs[13] = 0x8000;
    gsu.regs[12] = 0x0004;
    gsu.regs[1] = 0x0010;
    gsu.regs[0] = 0xA55A;
    gsu.src_reg = 0;
    gsu.running = true;

    assert_eq!(gsu.prime_pipe(&rom), Some(()));
    assert_eq!(gsu.fast_forward_simple_store_inc_loop(&rom, 64), Some(12));

    gsu.run_steps(&rom, 16);

    assert!(!gsu.running());
    assert_eq!(gsu.regs[12], 0x0000);
    assert_eq!(gsu.regs[1], 0x0018);
    assert_eq!(gsu.read_ram_word(0x0010), 0xA55A);
    assert_eq!(gsu.read_ram_word(0x0012), 0xA55A);
    assert_eq!(gsu.read_ram_word(0x0014), 0xA55A);
    assert_eq!(gsu.read_ram_word(0x0016), 0xA55A);
}

#[test]
fn run_steps_auto_uses_simple_store_inc_loop_fast_path() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x31; // STW [R1], R0
    rom[0x0001] = 0xD1; // INC R1
    rom[0x0002] = 0x3C; // LOOP
    rom[0x0003] = 0xD1; // delay slot: INC R1
    rom[0x0004] = 0x00; // STOP

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.regs[13] = 0x8000;
    gsu.regs[12] = 0x0100;
    gsu.regs[1] = 0x0010;
    gsu.regs[0] = 0xA55A;
    gsu.src_reg = 0;
    gsu.running = true;

    gsu.run_steps(&rom, 64);

    assert!(gsu.running());
    assert_eq!(gsu.regs[12], 0x00F0);
    assert_eq!(gsu.regs[1], 0x0030);
    assert!(gsu.debug_recent_pc_transfers().is_empty());
    assert_eq!(gsu.read_ram_word(0x0010), 0xA55A);
    assert_eq!(gsu.read_ram_word(0x0012), 0xA55A);
    assert_eq!(gsu.read_ram_word(0x002E), 0xA55A);
}

#[test]
fn run_steps_b48b_helper_copies_bytes_and_returns_via_r8() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];

    rom[0x0000] = 0xE6; // DEC R6
    rom[0x0001] = 0xB1; // FROM R1
    rom[0x0002] = 0x54; // ADD R4
    rom[0x0003] = 0xE0; // DEC R0
    rom[0x0004] = 0x3D; // ALT1
    rom[0x0005] = 0x40; // LDB (R0)
    rom[0x0006] = 0xE1; // DEC R1
    rom[0x0007] = 0x3D; // ALT1
    rom[0x0008] = 0x31; // STB (R1)
    rom[0x0009] = 0xE6; // DEC R6
    rom[0x000A] = 0x0A; // BPL
    rom[0x000B] = 0xF5; // -> B1
    rom[0x000C] = 0x01; // NOP
    rom[0x000D] = 0x98; // JMP R8
    rom[0x000E] = 0x01; // NOP
    rom[0x1000] = 0x00; // STOP

    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.regs[13] = 0x8000;
    gsu.regs[1] = 0x0012;
    gsu.regs[4] = 0x0002;
    gsu.regs[6] = 0x0001;
    gsu.regs[8] = 0x9000;
    gsu.game_ram[0x0013] = 0xAB;
    gsu.running = true;

    gsu.run_steps(&rom, 20);

    assert_eq!(gsu.game_ram[0x0011], 0xAB);
    assert_eq!(gsu.regs[1], 0x0011);
    assert_eq!(gsu.regs[6], 0xFFFF);
    assert_eq!(
        gsu.debug_recent_pc_transfers()
            .last()
            .map(|t| (t.from_pc, t.to_pc)),
        Some((0x800D, 0x9000))
    );
}

#[test]
fn stb_alt3_falls_back_to_byte_store() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.src_reg = 5;
    gsu.regs[5] = 0xABCD;
    gsu.regs[1] = 0x0050;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode(0x31, &[], 0x8000));
    assert_eq!(gsu.game_ram[0x0050], 0xCD);
    assert_eq!(gsu.game_ram[0x0051], 0x00);
}

#[test]
fn lms_r14_loads_little_endian_word_from_short_ram() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x31;
    gsu.game_ram[0x0062] = 0x96;
    gsu.game_ram[0x0063] = 0xD8;
    gsu.regs[15] = 0x8001;
    gsu.sfr |= super::SFR_ALT1_BIT;

    assert!(gsu.execute_opcode(0xAE, &rom, 0x8000));
    assert_eq!(gsu.regs[14], 0xD896);
}

#[test]
fn lms_r9_loads_little_endian_word_from_short_ram() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0001] = 0x16;
    gsu.game_ram[0x002C] = 0x00;
    gsu.game_ram[0x002D] = 0x40;
    gsu.regs[15] = 0x8001;
    gsu.sfr |= super::SFR_ALT1_BIT;

    assert!(gsu.execute_opcode(0xA9, &rom, 0x8000));
    assert_eq!(gsu.regs[9], 0x4000);
}
