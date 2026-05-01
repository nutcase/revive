use super::*;

#[test]
fn parse_save_state_gsu_reg_eq_accepts_register_constraints() {
    let parsed = parse_save_state_gsu_reg_eq("r2=004B,7:236E").expect("expected constraints");
    assert_eq!(
        parsed,
        vec![
            SaveStateGsuRegEq {
                reg: 2,
                value: 0x004B,
            },
            SaveStateGsuRegEq {
                reg: 7,
                value: 0x236E,
            },
        ]
    );

    let parsed_upper =
        parse_save_state_gsu_reg_eq("R2=004B,R7=236E").expect("expected uppercase constraints");
    assert_eq!(
        parsed_upper,
        vec![
            SaveStateGsuRegEq {
                reg: 2,
                value: 0x004B,
            },
            SaveStateGsuRegEq {
                reg: 7,
                value: 0x236E,
            },
        ]
    );
}

#[test]
fn parse_save_state_superfx_ram_byte_eq_accepts_addr_value_pairs() {
    let parsed = super::parse_save_state_superfx_ram_byte_eq("0x0136=4B,0x0137:00")
        .expect("expected filters");
    assert_eq!(
        parsed,
        vec![
            super::SaveStateSuperfxRamByteEq {
                addr: 0x0136,
                value: 0x4B,
            },
            super::SaveStateSuperfxRamByteEq {
                addr: 0x0137,
                value: 0x00,
            },
        ]
    );

    assert!(super::parse_save_state_superfx_ram_byte_eq("0136=123").is_none());
}

#[test]
fn parse_save_state_gsu_recent_exec_tail_accepts_pc_pairs() {
    let parsed = super::parse_save_state_gsu_recent_exec_tail("01:AF77,01:ACA6")
        .expect("expected recent exec tail");
    assert_eq!(parsed, vec![(0x01, 0xAF77), (0x01, 0xACA6)]);
}

#[test]
fn recent_exec_trace_ends_with_matches_tail_sequence() {
    let trace = vec![
        super::SuperFxExecTrace {
            opcode: 0x11,
            pbr: 0x01,
            pc: 0xAF71,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0,
            r0: 0,
            r1: 0,
            r2: 0,
            r3: 0,
            r4: 0,
            r5: 0,
            r6: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        },
        super::SuperFxExecTrace {
            opcode: 0xFF,
            pbr: 0x01,
            pc: 0xAF77,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0,
            r0: 0,
            r1: 0,
            r2: 0,
            r3: 0,
            r4: 0,
            r5: 0,
            r6: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        },
        super::SuperFxExecTrace {
            opcode: 0x60,
            pbr: 0x01,
            pc: 0xACA6,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0,
            r0: 0,
            r1: 0,
            r2: 0,
            r3: 0,
            r4: 0,
            r5: 0,
            r6: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        },
    ];

    assert!(super::recent_exec_trace_ends_with(
        &trace,
        &[(0x01, 0xAF77), (0x01, 0xACA6)]
    ));
    assert!(!super::recent_exec_trace_ends_with(
        &trace,
        &[(0x01, 0xAF71), (0x01, 0xACA6)]
    ));
}

#[test]
fn save_state_gsu_reg_eq_matches_current_register_values() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.write_reg(2, 0x004B);
    gsu.write_reg(7, 0x236E);

    let items = vec![
        SaveStateGsuRegEq {
            reg: 2,
            value: 0x004B,
        },
        SaveStateGsuRegEq {
            reg: 7,
            value: 0x236E,
        },
    ];
    assert!(items
        .iter()
        .all(|item| gsu.debug_reg(item.reg as usize) == item.value));

    gsu.write_reg(7, 0x004B);
    assert_ne!(gsu.debug_reg(7), 0x236E);
}

#[test]
fn getb_preserves_sign_and_zero_flags() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x5A;

    gsu.dst_reg = 3;
    gsu.sfr |= super::SFR_S_BIT | super::SFR_Z_BIT;
    gsu.rombr = 0x00;
    gsu.write_reg(14, 0x8000);

    assert!(gsu.execute_opcode(0xEF, &rom, 0x8000));
    assert_eq!(gsu.regs[3], 0x005A);
    assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
    assert_ne!(gsu.sfr & super::SFR_Z_BIT, 0);
}

#[test]
fn getbh_preserves_sign_and_zero_flags() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x5A;

    gsu.src_reg = 4;
    gsu.dst_reg = 5;
    gsu.regs[4] = 0x0034;
    gsu.sfr |= super::SFR_ALT1_BIT | super::SFR_S_BIT;
    gsu.rombr = 0x00;
    gsu.write_reg(14, 0x8000);

    assert!(gsu.execute_opcode(0xEF, &rom, 0x8000));
    assert_eq!(gsu.regs[5], 0x5A34);
    assert_ne!(gsu.sfr & super::SFR_S_BIT, 0);
    assert_eq!(gsu.sfr & super::SFR_Z_BIT, 0);
}

#[test]
fn getb_write_to_r14_marks_rom_buffer_dirty_until_next_read() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x34;
    rom[0x0034] = 0x77;

    gsu.dst_reg = 14;
    gsu.rombr = 0x00;
    gsu.write_reg(14, 0x8000);

    assert!(gsu.execute_opcode(0xEF, &rom, 0x8000));
    assert_eq!(gsu.regs[14], 0x0034);
    assert!(gsu.rom_buffer_valid);
    assert!(!gsu.rom_buffer_pending);
    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x77));
}

#[test]
fn last_reg_write_excluding_skips_trivial_opcode() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.recent_reg_writes.push(super::SuperFxRegWrite {
        opcode: 0xAC,
        pbr: 0x01,
        pc: 0xB380,
        reg: 12,
        old_value: 0,
        new_value: 8,
        src_reg: 0,
        dst_reg: 12,
        sfr: 0,
        repeats: 1,
    });
    gsu.recent_reg_writes.push(super::SuperFxRegWrite {
        opcode: 0x3C,
        pbr: 0x01,
        pc: 0xB391,
        reg: 12,
        old_value: 8,
        new_value: 7,
        src_reg: 0,
        dst_reg: 0,
        sfr: 0,
        repeats: 1,
    });

    let last = gsu
        .debug_last_reg_write_excluding(12, &[0x3C])
        .expect("expected non-trivial writer");
    assert_eq!(last.opcode, 0xAC);
    assert_eq!(last.pc, 0xB380);
    assert_eq!(last.new_value, 8);
}

#[test]
fn last_reg_write_excluding_returns_none_when_only_excluded_opcodes_exist() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.recent_reg_writes.push(super::SuperFxRegWrite {
        opcode: 0xE4,
        pbr: 0x01,
        pc: 0xB397,
        reg: 4,
        old_value: 2,
        new_value: 1,
        src_reg: 0,
        dst_reg: 0,
        sfr: 0,
        repeats: 1,
    });

    assert!(gsu.debug_last_reg_write_excluding(4, &[0xE4]).is_none());
}

#[test]
fn record_low_ram_write_tracks_last_writer_for_address() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0x8619;
    gsu.current_exec_opcode = 0xA0;
    gsu.src_reg = 0;
    gsu.dst_reg = 0;
    gsu.sfr = 0x0166;

    gsu.record_low_ram_write(0x0032, 0x00, 0x8E);
    gsu.record_low_ram_write(0x0032, 0x00, 0x8E);
    let write = gsu
        .debug_last_low_ram_write(0x0032)
        .expect("expected low RAM writer");
    assert_eq!(write.pc, 0x8619);
    assert_eq!(write.new_value, 0x8E);
    assert_eq!(write.r10, 0);
    assert_eq!(write.r12, 0);
    assert_eq!(write.r14, 0);
    assert_eq!(write.r15, 0);
    assert_eq!(write.repeats, 2);
    assert!(gsu.debug_last_low_ram_write(0x0100).is_none());
}

#[test]
fn record_low_ram_write_tracks_upper_short_ram_addresses() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xD1A8;
    gsu.current_exec_opcode = 0xA2;
    gsu.src_reg = 0;
    gsu.dst_reg = 0;
    gsu.sfr = 0x0064;
    gsu.regs[10] = 0x04C8;
    gsu.regs[12] = 0x012C;
    gsu.regs[14] = 0x083B;
    gsu.regs[15] = 0xD1AB;

    gsu.record_low_ram_write(0x01FE, 0x00, 0xA8);
    gsu.record_low_ram_write(0x01FF, 0x00, 0xBC);

    let lo = gsu
        .debug_last_low_ram_write(0x01FE)
        .expect("expected upper short RAM low byte write");
    assert_eq!(lo.pc, 0xD1A8);
    assert_eq!(lo.new_value, 0xA8);

    let hi = gsu
        .debug_last_low_ram_write(0x01FF)
        .expect("expected upper short RAM high byte write");
    assert_eq!(hi.pc, 0xD1A8);
    assert_eq!(hi.new_value, 0xBC);
}

#[test]
fn load_data_clears_transient_exec_state() {
    let source = SuperFx::new(0x20_0000);
    let state = source.save_data();

    let mut restored = SuperFx::new(0x20_0000);
    restored.current_exec_pbr = 0x01;
    restored.current_exec_pc = 0xD040;
    restored.current_exec_opcode = 0x19;
    restored.save_state_pc_hit = Some((0x01, 0xD040));
    restored.save_state_pc_hit_count = 3;
    restored.recent_exec_trace.push(super::SuperFxExecTrace {
        opcode: 0x19,
        pbr: 0x01,
        pc: 0xD040,
        src_reg: 4,
        dst_reg: 4,
        sfr: 0x1066,
        r0: 0,
        r1: 0,
        r2: 0,
        r3: 0,
        r4: 0,
        r5: 0,
        r6: 0,
        r11: 0,
        r12: 0,
        r13: 0,
        r14: 0,
        r15: 0,
    });

    restored.load_data(&state);

    assert_eq!(restored.debug_current_exec_pbr(), 0);
    assert_eq!(restored.debug_current_exec_pc(), 0);
    assert_eq!(restored.current_exec_opcode, 0);
    assert!(restored.debug_take_save_state_pc_hit().is_none());
    assert_eq!(restored.save_state_pc_hit_count, 0);
    assert!(restored.recent_exec_trace.is_empty());
}

#[test]
fn rewind_pipe_to_instruction_boundary_restores_current_opcode() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[15] = 0x8123;
    gsu.pbr = 0x05;
    gsu.pipe = 0x11;
    gsu.pipe_valid = true;
    gsu.pipe_pc = 0x8122;
    gsu.pipe_pbr = 0x05;

    gsu.rewind_pipe_to_instruction_boundary(0x01, 0x8000, 0x0D);

    assert!(gsu.pipe_valid);
    assert_eq!(gsu.pipe_pbr, 0x01);
    assert_eq!(gsu.pipe_pc, 0x8000);
    assert_eq!(gsu.pipe, 0x0D);
    assert_eq!(gsu.regs[15], 0x8001);
}

#[test]
fn record_reg_write_tracks_last_nontrivial_writer_per_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.last_nontrivial_reg_writes[12] = Some(super::SuperFxRegWrite {
        opcode: 0xAC,
        pbr: 0x01,
        pc: 0xB380,
        reg: 12,
        old_value: 0,
        new_value: 8,
        src_reg: 0,
        dst_reg: 12,
        sfr: 0,
        repeats: 1,
    });

    let last = gsu
        .debug_last_nontrivial_reg_write(12)
        .expect("expected tracked writer");
    assert_eq!(last.opcode, 0xAC);
    assert_eq!(last.pc, 0xB380);
    assert_eq!(last.new_value, 8);
}

#[test]
fn push_nontrivial_reg_write_history_coalesces_same_writer() {
    let mut history = Vec::new();
    let write = super::SuperFxRegWrite {
        opcode: 0x04,
        pbr: 0x01,
        pc: 0xB4BF,
        reg: 4,
        old_value: 2,
        new_value: 4,
        src_reg: 4,
        dst_reg: 4,
        sfr: 0,
        repeats: 1,
    };
    super::SuperFx::push_nontrivial_reg_write_history(&mut history, write.clone());
    let mut updated = write.clone();
    updated.new_value = 8;
    updated.repeats = 3;
    super::SuperFx::push_nontrivial_reg_write_history(&mut history, updated);

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].new_value, 8);
    assert_eq!(history[0].repeats, 3);
}
