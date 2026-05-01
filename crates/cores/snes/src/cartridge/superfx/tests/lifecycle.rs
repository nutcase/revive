use super::*;

#[test]
fn cpu_write_r15_finishes_and_raises_irq_when_unmasked() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.write_register(0x301E, 0x34);
    gsu.write_register(0x301F, 0x12);

    assert_eq!(gsu.read_register(0x301E, 0), 0x34);
    assert_eq!(gsu.read_register(0x301F, 0), 0x12);
    assert!(!gsu.running());
    assert!(gsu.scpu_irq_asserted());
    let _ = gsu.read_register(0x3031, 0);
    assert!(!gsu.scpu_irq_asserted());
}

#[test]
fn cpu_write_r15_low_byte_does_not_start_until_high_byte_arrives() {
    let mut gsu = SuperFx::new(0x20_0000);

    gsu.write_register(0x301E, 0x34);
    assert_eq!(gsu.read_register(0x301E, 0), 0x34);
    assert!(!gsu.running());
    assert!(!gsu.scpu_irq_asserted());

    gsu.write_register(0x301F, 0x12);
    assert!(gsu.scpu_irq_asserted());
    assert_eq!(gsu.read_register(0x301E, 0), 0x34);
    assert_eq!(gsu.read_register(0x301F, 0), 0x12);
}

#[test]
fn cpu_write_r15_high_byte_arms_execution_without_consuming_steps() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0x00u8; 0x20_0000];

    gsu.write_register_with_rom(0x301E, 0x00, &rom);
    gsu.write_register_with_rom(0x301F, 0x00, &rom);

    assert!(gsu.running());
    assert_ne!(gsu.read_register(0x3030, 0) & (SFR_GO_BIT as u8), 0);
    assert_eq!(gsu.debug_reg(15), 0x0000);
}

#[test]
fn starfox_like_cpu_write_r15_does_not_mutate_working_regs_immediately() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0x00u8; 0x20_0000];

    gsu.pbr = 0x01;
    gsu.rombr = 0x14;
    gsu.scmr = 0x39;
    gsu.write_reg(9, 0x2800);
    gsu.write_reg(13, 0xB3DE);
    gsu.write_reg(14, 0x6242);
    gsu.write_reg(15, 0xB3E6);

    gsu.write_register_with_rom(0x301E, 0x01, &rom);
    assert_eq!(gsu.debug_reg(15), 0xB301);
    assert_eq!(gsu.debug_reg(9), 0x2800);
    assert_eq!(gsu.debug_reg(13), 0xB3DE);
    assert_eq!(gsu.debug_reg(14), 0x6242);

    gsu.write_register_with_rom(0x301F, 0xB3, &rom);
    assert!(gsu.running());
    assert_eq!(gsu.debug_reg(15), 0xB301);
    assert_eq!(gsu.debug_reg(9), 0x2800);
    assert_eq!(gsu.debug_reg(13), 0xB3DE);
    assert_eq!(gsu.debug_reg(14), 0x6242);
}

#[test]
fn starting_execution_keeps_last_completed_tile_snapshot() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.tile_snapshot = vec![0xAA; 32];
    gsu.tile_snapshot_valid = true;

    gsu.start_execution(&rom);

    assert!(gsu.tile_snapshot_valid);
    assert_eq!(gsu.tile_snapshot[0], 0xAA);
}

#[test]
fn screen_buffer_snapshot_uses_captured_stop_metadata() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.latest_stop_snapshot = vec![0x11; 32];
    gsu.latest_stop_snapshot_valid = true;
    gsu.latest_stop_scbr = 0x0B;
    gsu.latest_stop_height = 192;
    gsu.latest_stop_bpp = 4;
    gsu.latest_stop_mode = 2;
    gsu.latest_stop_pc = 0xFBE4;
    gsu.latest_stop_pbr = 0x06;
    gsu.scbr = 0x00;
    gsu.scmr = 0x00;

    let (buffer, height, bpp, mode) = gsu.screen_buffer_snapshot().unwrap();
    assert_eq!(buffer.len(), 32);
    assert_eq!(height, 192);
    assert_eq!(bpp, 4);
    assert_eq!(mode, 2);
}

#[test]
fn game_ram_read_linear_uses_captured_snapshot_base_address() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.latest_stop_snapshot = vec![0x22; 32];
    gsu.latest_stop_snapshot_valid = true;
    gsu.latest_stop_scbr = 0x0B;
    gsu.latest_stop_height = 192;
    gsu.latest_stop_bpp = 4;
    gsu.latest_stop_mode = 2;
    gsu.latest_stop_pc = 0xFBE4;
    gsu.latest_stop_pbr = 0x06;
    gsu.scbr = 0x00;
    gsu.scmr = 0x00;

    assert_eq!(gsu.game_ram_read_linear((0x0Busize << 10) + 7), 0x22);
}

#[test]
fn screen_buffer_display_snapshot_requires_captured_stop_by_default() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.tile_snapshot = vec![0xAA; 32];
    gsu.tile_snapshot_valid = true;
    gsu.tile_snapshot_height = 192;
    gsu.tile_snapshot_bpp = 4;
    gsu.tile_snapshot_mode = 2;
    gsu.scbr = 0x0B;
    gsu.scmr = 0x21;
    let live_base = (gsu.scbr as usize) << 10;
    gsu.game_ram[live_base..live_base + 32].fill(0x11);

    assert!(gsu.screen_buffer_display_snapshot().is_none());
}

#[test]
fn display_snapshot_updates_on_dma_and_survives_later_stop_snapshots() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.latest_stop_snapshot = vec![0x22; 32];
    gsu.latest_stop_snapshot_valid = true;
    gsu.latest_stop_scbr = 0x0B;
    gsu.latest_stop_height = 192;
    gsu.latest_stop_bpp = 4;
    gsu.latest_stop_mode = 2;
    gsu.latest_stop_pc = 0xD6E6;
    gsu.latest_stop_pbr = 0x01;

    assert!(gsu.capture_display_snapshot_for_dma((0x0Busize << 10) + 4, 8));

    gsu.latest_stop_snapshot = vec![0x33; 32];
    gsu.latest_stop_pc = 0x829D;

    let (buffer, height, bpp, mode) = gsu.screen_buffer_display_snapshot().unwrap();
    let mut expected = vec![0; 32];
    expected[4..12].fill(0x22);
    assert_eq!(buffer, expected);
    assert_eq!(height, 192);
    assert_eq!(bpp, 4);
    assert_eq!(mode, 2);

    assert!(gsu.capture_display_snapshot_for_dma((0x0Busize << 10) + 16, 8));
    let (buffer, _height, _bpp, _mode) = gsu.screen_buffer_display_snapshot().unwrap();
    expected[16..24].fill(0x33);
    assert_eq!(buffer, expected);
}

#[test]
fn save_data_roundtrip_preserves_display_and_tile_snapshots() {
    let mut src = SuperFx::new(0x20_0000);
    src.tile_snapshot = vec![0xAA; 32];
    src.tile_snapshot_valid = true;
    src.tile_snapshot_scbr = 0x0B;
    src.tile_snapshot_height = 192;
    src.tile_snapshot_bpp = 4;
    src.tile_snapshot_mode = 2;
    src.tile_snapshot_pc = 0xB301;
    src.tile_snapshot_pbr = 0x01;
    src.latest_stop_snapshot = vec![0x55; 64];
    src.latest_stop_snapshot_valid = true;
    src.latest_stop_scbr = 0x0C;
    src.latest_stop_height = 160;
    src.latest_stop_bpp = 4;
    src.latest_stop_mode = 1;
    src.latest_stop_pc = 0xFBE4;
    src.latest_stop_pbr = 0x06;
    src.display_snapshot = vec![0x77; 16];
    src.display_snapshot_valid = true;
    src.display_snapshot_scbr = 0x0D;
    src.display_snapshot_height = 128;
    src.display_snapshot_bpp = 2;
    src.display_snapshot_mode = 0;
    src.recent_stop_snapshots.push(StopSnapshot {
        data: vec![0x11; 48],
        scbr: 0x0B,
        height: 192,
        bpp: 4,
        mode: 2,
        pc: 0xB3E4,
        pbr: 0x01,
    });
    src.recent_tile_snapshots.push(StopSnapshot {
        data: vec![0x22; 48],
        scbr: 0x0B,
        height: 192,
        bpp: 4,
        mode: 2,
        pc: 0xB3E4,
        pbr: 0x01,
    });

    let state = src.save_data();
    let mut dst = SuperFx::new(0x20_0000);
    dst.load_data(&state);

    assert_eq!(dst.tile_snapshot, vec![0xAA; 32]);
    assert!(dst.tile_snapshot_valid);
    assert_eq!(dst.tile_snapshot_scbr, 0x0B);
    assert_eq!(dst.tile_snapshot_height, 192);
    assert_eq!(dst.tile_snapshot_bpp, 4);
    assert_eq!(dst.tile_snapshot_mode, 2);
    assert_eq!(dst.tile_snapshot_pc, 0xB301);
    assert_eq!(dst.tile_snapshot_pbr, 0x01);
    assert_eq!(dst.latest_stop_snapshot, vec![0x55; 64]);
    assert!(dst.latest_stop_snapshot_valid);
    assert_eq!(dst.latest_stop_scbr, 0x0C);
    assert_eq!(dst.latest_stop_height, 160);
    assert_eq!(dst.latest_stop_bpp, 4);
    assert_eq!(dst.latest_stop_mode, 1);
    assert_eq!(dst.latest_stop_pc, 0xFBE4);
    assert_eq!(dst.latest_stop_pbr, 0x06);
    assert_eq!(dst.display_snapshot, vec![0x77; 16]);
    assert!(dst.display_snapshot_valid);
    assert_eq!(dst.display_snapshot_scbr, 0x0D);
    assert_eq!(dst.display_snapshot_height, 128);
    assert_eq!(dst.display_snapshot_bpp, 2);
    assert_eq!(dst.display_snapshot_mode, 0);
    assert_eq!(dst.recent_stop_snapshots.len(), 1);
    assert_eq!(dst.recent_stop_snapshots[0].pc, 0xB3E4);
    assert_eq!(dst.recent_stop_snapshots[0].data, vec![0x11; 48]);
    assert_eq!(dst.recent_tile_snapshots.len(), 1);
    assert_eq!(dst.recent_tile_snapshots[0].pc, 0xB3E4);
    assert_eq!(dst.recent_tile_snapshots[0].data, vec![0x22; 48]);
}

#[test]
fn save_data_roundtrip_preserves_last_nontrivial_reg_writes() {
    let mut src = SuperFx::new(0x20_0000);
    src.last_nontrivial_reg_writes[4] = Some(SuperFxRegWrite {
        opcode: 0xA4,
        pbr: 0x01,
        pc: 0xD04D,
        reg: 4,
        old_value: 0x1234,
        new_value: 0x0000,
        src_reg: 0,
        dst_reg: 0,
        sfr: 0x0068,
        repeats: 1,
    });
    src.last_nontrivial_reg_writes[9] = Some(SuperFxRegWrite {
        opcode: 0x19,
        pbr: 0x01,
        pc: 0xD055,
        reg: 9,
        old_value: 0x0E22,
        new_value: 0x0000,
        src_reg: 4,
        dst_reg: 4,
        sfr: 0x1060,
        repeats: 1,
    });

    let state = src.save_data();
    let mut dst = SuperFx::new(0x20_0000);
    dst.load_data(&state);

    let r4 = dst.debug_last_nontrivial_reg_write(4).expect("r4 write");
    assert_eq!(r4.pc, 0xD04D);
    assert_eq!(r4.new_value, 0x0000);
    let r9 = dst.debug_last_nontrivial_reg_write(9).expect("r9 write");
    assert_eq!(r9.pc, 0xD055);
    assert_eq!(r9.old_value, 0x0E22);
    assert_eq!(r9.new_value, 0x0000);
}

#[test]
fn save_data_roundtrip_preserves_recent_nontrivial_reg_writes() {
    let mut src = SuperFx::new(0x20_0000);
    src.recent_nontrivial_reg_writes[0] = vec![
        SuperFxRegWrite {
            opcode: 0xF0,
            pbr: 0x01,
            pc: 0xD069,
            reg: 0,
            old_value: 0x0100,
            new_value: 0x00BF,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0x1060,
            repeats: 1,
        },
        SuperFxRegWrite {
            opcode: 0xA0,
            pbr: 0x01,
            pc: 0x8191,
            reg: 0,
            old_value: 0x0080,
            new_value: 0x0100,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0x0068,
            repeats: 1,
        },
    ];
    src.recent_nontrivial_reg_writes[9] = vec![SuperFxRegWrite {
        opcode: 0x19,
        pbr: 0x01,
        pc: 0xD055,
        reg: 9,
        old_value: 0x0E22,
        new_value: 0x0000,
        src_reg: 4,
        dst_reg: 4,
        sfr: 0x1060,
        repeats: 1,
    }];

    let state = src.save_data();
    let mut dst = SuperFx::new(0x20_0000);
    dst.load_data(&state);

    let r0 = dst.debug_recent_nontrivial_reg_writes(0);
    assert_eq!(r0.len(), 2);
    assert_eq!(r0[0].pc, 0xD069);
    assert_eq!(r0[0].new_value, 0x00BF);
    assert_eq!(r0[1].pc, 0x8191);
    assert_eq!(r0[1].old_value, 0x0080);
    assert_eq!(r0[1].new_value, 0x0100);

    let r9 = dst.debug_recent_nontrivial_reg_writes(9);
    assert_eq!(r9.len(), 1);
    assert_eq!(r9[0].pc, 0xD055);
    assert_eq!(r9[0].new_value, 0x0000);
}

#[test]
fn save_data_roundtrip_preserves_recent_reg_writes() {
    let mut src = SuperFx::new(0x20_0000);
    src.last_reg_writes[14] = Some(SuperFxRegWrite {
        opcode: 0xAE,
        pbr: 0x01,
        pc: 0xB30A,
        reg: 14,
        old_value: 0xF144,
        new_value: 0x34B6,
        src_reg: 0,
        dst_reg: 0,
        sfr: 0x0062,
        repeats: 1,
    });
    src.recent_reg_writes_by_reg[14] = vec![src.last_reg_writes[14].clone().unwrap()];
    src.recent_reg_writes = vec![
        SuperFxRegWrite {
            opcode: 0xAE,
            pbr: 0x01,
            pc: 0xB30A,
            reg: 14,
            old_value: 0xF144,
            new_value: 0x34B6,
            src_reg: 0,
            dst_reg: 0,
            sfr: 0x0062,
            repeats: 1,
        },
        SuperFxRegWrite {
            opcode: 0x04,
            pbr: 0x01,
            pc: 0xB4BF,
            reg: 4,
            old_value: 0x0003,
            new_value: 0x0001,
            src_reg: 4,
            dst_reg: 4,
            sfr: 0x1068,
            repeats: 1,
        },
    ];

    let state = src.save_data();
    let mut dst = SuperFx::new(0x20_0000);
    dst.load_data(&state);

    let writes = dst.debug_recent_reg_writes();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].pc, 0xB30A);
    assert_eq!(writes[0].reg, 14);
    assert_eq!(writes[0].new_value, 0x34B6);
    assert_eq!(writes[1].pc, 0xB4BF);
    assert_eq!(writes[1].reg, 4);
    assert_eq!(writes[1].new_value, 0x0001);
    assert_eq!(dst.debug_last_reg_write(14).unwrap().new_value, 0x34B6);
    let r14 = dst.debug_recent_reg_writes_for_reg(14, 4);
    assert_eq!(r14.len(), 1);
    assert_eq!(r14[0].pc, 0xB30A);
}

#[test]
fn debug_recent_reg_writes_for_reg_filters_and_limits() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.recent_reg_writes = vec![
        SuperFxRegWrite {
            opcode: 0x10,
            pbr: 0x01,
            pc: 0x8000,
            reg: 1,
            old_value: 0x0000,
            new_value: 0x0001,
            src_reg: 0,
            dst_reg: 1,
            sfr: 0,
            repeats: 1,
        },
        SuperFxRegWrite {
            opcode: 0x11,
            pbr: 0x01,
            pc: 0x8001,
            reg: 2,
            old_value: 0x0000,
            new_value: 0x0002,
            src_reg: 0,
            dst_reg: 2,
            sfr: 0,
            repeats: 1,
        },
        SuperFxRegWrite {
            opcode: 0x12,
            pbr: 0x01,
            pc: 0x8002,
            reg: 1,
            old_value: 0x0001,
            new_value: 0x0003,
            src_reg: 0,
            dst_reg: 1,
            sfr: 0,
            repeats: 1,
        },
        SuperFxRegWrite {
            opcode: 0x13,
            pbr: 0x01,
            pc: 0x8003,
            reg: 1,
            old_value: 0x0003,
            new_value: 0x0004,
            src_reg: 0,
            dst_reg: 1,
            sfr: 0,
            repeats: 1,
        },
    ];

    let writes = gsu.debug_recent_reg_writes_for_reg(1, 2);

    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].pc, 0x8002);
    assert_eq!(writes[1].pc, 0x8003);
    assert!(writes.iter().all(|write| write.reg == 1));
}

#[test]
fn save_data_roundtrip_preserves_last_low_ram_writes() {
    let mut src = SuperFx::new(0x20_0000);
    src.last_low_ram_writes[0x28] = Some(SuperFxRamWrite {
        opcode: 0x90,
        pbr: 0x01,
        pc: 0xCF70,
        addr: 0x0028,
        old_value: 0x78,
        new_value: 0x74,
        src_reg: 0,
        dst_reg: 0,
        sfr: 0x0064,
        r10: 0x04CA,
        r12: 0x0000,
        r14: 0xC8EC,
        r15: 0xCF72,
        repeats: 1,
    });
    src.last_low_ram_writes[0x29] = Some(SuperFxRamWrite {
        opcode: 0x90,
        pbr: 0x01,
        pc: 0xCF70,
        addr: 0x0029,
        old_value: 0x7A,
        new_value: 0x7A,
        src_reg: 0,
        dst_reg: 0,
        sfr: 0x0064,
        r10: 0x04CA,
        r12: 0x0000,
        r14: 0xC8EC,
        r15: 0xCF72,
        repeats: 1,
    });

    let state = src.save_data();
    let mut dst = SuperFx::new(0x20_0000);
    dst.load_data(&state);

    let lo = dst
        .debug_last_low_ram_write(0x0028)
        .expect("low byte write");
    assert_eq!(lo.pc, 0xCF70);
    assert_eq!(lo.new_value, 0x74);
    assert_eq!(lo.r10, 0x04CA);

    let hi = dst
        .debug_last_low_ram_write(0x0029)
        .expect("high byte write");
    assert_eq!(hi.pc, 0xCF70);
    assert_eq!(hi.new_value, 0x7A);
    assert_eq!(hi.r15, 0xCF72);
}

#[test]
fn game_ram_read_linear_does_not_fall_back_to_tile_snapshot_by_default() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.tile_snapshot = vec![0xAA; 32];
    gsu.tile_snapshot_valid = true;
    gsu.tile_snapshot_scbr = 0x0B;
    gsu.scbr = 0x0B;
    gsu.scmr = 0x21;
    let live_addr = (0x0Busize << 10) + 7;
    gsu.game_ram[live_addr] = 0x11;

    assert_eq!(gsu.game_ram_read_linear(live_addr), 0x11);
}
