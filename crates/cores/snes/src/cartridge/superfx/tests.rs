use super::{
    parse_save_state_gsu_reg_eq, SaveStateGsuRegEq, StopSnapshot, SuperFx, SuperFxRamWrite,
    SuperFxRegWrite, SFR_ALT1_BIT, SFR_ALT2_BIT, SFR_B_BIT, SFR_CY_BIT, SFR_GO_BIT, SFR_IRQ_BIT,
    SFR_OV_BIT, SFR_S_BIT, SFR_Z_BIT,
};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

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

#[test]
fn illegal_rom_pattern_matches_documented_dummy_values() {
    assert_eq!(SuperFx::illegal_rom_read_value(0x8000), 0x00);
    assert_eq!(SuperFx::illegal_rom_read_value(0x8004), 0x04);
    assert_eq!(SuperFx::illegal_rom_read_value(0x800A), 0x08);
    assert_eq!(SuperFx::illegal_rom_read_value(0x800E), 0x0C);
    assert_eq!(SuperFx::illegal_rom_read_value(0x8001), 0x01);
}

#[test]
fn cpu_rom_addr_maps_e0_ff_banks_like_c0_df_mirrors() {
    assert_eq!(SuperFx::cpu_rom_addr(0xC2, 0x8515), Some(0x28515));
    assert_eq!(SuperFx::cpu_rom_addr(0xE2, 0x8515), Some(0x28515));
    assert_eq!(SuperFx::cpu_rom_addr(0xFF, 0xFFFF), Some(0x1FFFFF));
}

#[test]
fn writing_sfr_go_directly_triggers_noop_completion() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.write_register(0x3031, 0x00);
    gsu.write_register(0x3030, 0x20);

    assert!(!gsu.running());
    assert!((gsu.sfr & SFR_IRQ_BIT) != 0);
}

#[test]
fn sfr_low_reflects_natural_go_clear_after_run_has_completed() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.write_register(0x301E, 0x30);
    gsu.write_register(0x301F, 0xB3);

    assert!(!gsu.running());
    assert_eq!(gsu.read_register(0x3030, 0xFF) & (SFR_GO_BIT as u8), 0);
}

#[test]
fn sfr_low_reports_raw_sfr_bits_even_when_execution_has_stopped() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.sfr = 0x0030;
    gsu.running = false;

    assert_eq!(gsu.read_register(0x3030, 0xFF), 0x30);
}

#[test]
fn read_data_rom_byte_reads_from_buffer_without_modifying_r14() {
    // GETB reads from the ROM buffer without auto-incrementing R14.
    // R14 must be managed explicitly by the program.
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x1_0000] = 0x34;

    gsu.rombr = 0x02;
    gsu.write_reg(14, 0x0000);

    // First read refreshes the buffer from current ROMB/R14.
    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x34));
    // Second read returns same data (R14 unchanged, buffer unchanged)
    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x34));
    assert_eq!(gsu.rombr, 0x02);
    assert_eq!(gsu.regs[14], 0x0000); // R14 not modified
}

#[test]
fn write_reg_r14_triggers_rom_buffer_reload() {
    // R14 writes mark the ROM buffer dirty; the next GETB/GETC read
    // refreshes from the new address.
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0A_7141] = 0x20;
    rom[0x0A_7142] = 0x6F;

    gsu.rombr = 0x14;
    gsu.write_reg(14, 0xF141);
    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x20));

    // DEC R14 triggers reload from new address
    gsu.write_reg(14, 0xF142);
    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x6F));
    assert_eq!(gsu.regs[14], 0xF142); // R14 not modified by read
}

#[test]
fn write_reg_r14_uses_current_rombr_when_buffer_refreshes() {
    // Match bsnes more closely: the ROM buffer is refreshed after the
    // instruction using the current ROMB/R14, not a bank captured at write time.
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0A_7141] = 0x20;
    rom[0x0B_F141] = 0x33;

    gsu.rombr = 0x14;
    gsu.write_reg(14, 0xF141);
    gsu.rombr = 0x17; // change rombr AFTER write_reg

    // Read uses the current ROMB at refresh time.
    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x33));
}

#[test]
fn cpu_write_r14_preserves_pending_rom_reload_into_start_execution() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0A_7141] = 0x20;

    gsu.rombr = 0x14;
    gsu.write_register_with_rom(0x301C, 0x41, &rom);
    gsu.write_register_with_rom(0x301D, 0xF1, &rom);

    assert!(gsu.rom_buffer_pending);
    assert!(!gsu.rom_buffer_valid);
    assert_eq!(gsu.rom_buffer_pending_bank, 0x14);
    assert_eq!(gsu.rom_buffer_pending_addr, 0xF141);

    gsu.debug_prepare_cpu_start(&rom);

    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x20));
}

#[test]
fn cpu_write_pbr_invalidates_cache_lines() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.cache_enabled = true;
    gsu.cache_valid_mask = u32::MAX;

    gsu.write_register_with_rom(0x3034, 0x21, &[]);

    assert_eq!(gsu.pbr, 0x21);
    assert_eq!(gsu.cache_valid_mask, 0);
}

#[test]
fn read_data_rom_byte_uses_bsnes_lorom_mapping_for_low_banks() {
    let mut gsu = SuperFx::new(0x10_0000);
    let mut rom = vec![0u8; 0x10_0000];
    rom[0x0A_56C1] = 0x1F;
    rom[0x0A_56C0] = 0xD5;

    gsu.rombr = 0x14;
    gsu.write_reg(14, 0x56C1);
    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0x1F));

    gsu.write_reg(14, 0x56C0);
    assert_eq!(gsu.read_data_rom_byte(&rom), Some(0xD5));
}

#[test]
fn rombr_write_clears_alt3_before_following_iwt_table_setup() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x000A].copy_from_slice(&[
        0xA0, 0x14, // IBT R0,#14
        0x3F, // ALT3
        0xDF, // ROMBR = R0, then clear prefix flags
        0xFB, 0xB8, 0x1A, // IWT R11,#1AB8
        0xFC, 0x2C, 0x01, // IWT R12,#012C
    ]);

    // If ALT3 leaked past DF, FB/FC would behave as LM and read these words instead.
    gsu.write_ram_word(0x1AB8, 0x9C09);
    gsu.write_ram_word(0x012C, 0x004B);
    gsu.regs[15] = 0x8000;
    gsu.running = true;

    gsu.run_steps(&rom, 16);

    assert_eq!(gsu.debug_rombr(), 0x14);
    assert_eq!(gsu.regs[11], 0x1AB8);
    assert_eq!(gsu.regs[12], 0x012C);
}

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

#[test]
fn cpu_writes_do_not_override_read_only_rombr_and_rambr() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.rombr = 0x12;
    gsu.rambr = 0x01;

    gsu.write_register(0x3036, 0xB2);
    gsu.write_register(0x303C, 0x03);

    assert_eq!(gsu.debug_rombr(), 0x12);
    assert_eq!(gsu.debug_rambr(), 0x01);
}

#[test]
fn read_only_rambr_register_exposes_low_two_bits() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.rambr = 0x03;

    assert_eq!(gsu.read_register(0x303C, 0x00), 0x03);
}

#[test]
fn cpu_writes_do_not_override_read_only_cbr() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.cbr = 0x34B0;
    gsu.cache_valid_mask = u32::MAX;

    gsu.write_register(0x303E, 0xBE);
    gsu.write_register(0x303F, 0x34);

    assert_eq!(gsu.debug_cbr(), 0x34B0);
    assert_eq!(gsu.cache_valid_mask, u32::MAX);
}

#[test]
fn cpu_cache_window_marks_written_line_valid() {
    let mut gsu = SuperFx::new(0x20_0000);

    gsu.write_register(0x3112, 0x5A);

    assert_eq!(gsu.cache_read(0x3112), 0x5A);
    assert_ne!(gsu.cache_valid_mask & (1 << 1), 0);
}

#[test]
fn ram_word_access_keeps_last_ram_addr_at_word_base() {
    let mut gsu = SuperFx::new(0x20_0000);

    gsu.write_ram_word(0x1234, 0xBEEF);
    assert_eq!(gsu.last_ram_addr, 0x1234);

    let value = gsu.read_ram_word(0x1234);
    assert_eq!(value, 0xBEEF);
    assert_eq!(gsu.last_ram_addr, 0x1234);
}

#[test]
fn direct_ram_word_access_uses_xor_one_for_high_byte() {
    let mut gsu = SuperFx::new(0x20_0000);

    gsu.write_ram_word(0x1235, 0xBEEF);

    assert_eq!(gsu.game_ram[0x1235], 0xEF);
    assert_eq!(gsu.game_ram[0x1234], 0xBE);
    assert_eq!(gsu.read_ram_word(0x1235), 0xBEEF);
}

#[test]
fn short_ram_word_access_uses_plus_one_for_high_byte() {
    let mut gsu = SuperFx::new(0x20_0000);

    gsu.write_ram_word_short(0x1235, 0xBEEF);

    assert_eq!(gsu.game_ram[0x1235], 0xEF);
    assert_eq!(gsu.game_ram[0x1236], 0xBE);
    assert_eq!(gsu.read_ram_word_short(0x1235), 0xBEEF);
}

#[test]
fn buffered_ram_word_write_defers_final_byte_until_sync() {
    let mut gsu = SuperFx::new(0x20_0000);

    gsu.write_ram_buffer_word(0x1234, 0xBEEF);

    assert_eq!(gsu.game_ram[0x1234], 0xEF);
    assert_eq!(gsu.game_ram[0x1235], 0x00);
    assert!(gsu.ram_buffer_pending);
    assert_eq!(gsu.ram_buffer_pending_addr, 0x1235);
    assert_eq!(gsu.ram_buffer_pending_data, 0xBE);

    gsu.sync_ram_buffer();

    assert_eq!(gsu.game_ram[0x1235], 0xBE);
    assert!(!gsu.ram_buffer_pending);
}

#[test]
fn ramb_flushes_pending_buffer_before_bank_switch() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];

    gsu.write_ram_buffer_byte(0x0010, 0xAA);
    gsu.src_reg = 0;
    gsu.regs[0] = 0x0001;
    gsu.sfr = SFR_ALT2_BIT;

    assert!(gsu.execute_opcode_internal(0xDF, &rom, 0x8000, false));

    assert_eq!(gsu.game_ram[0x0010], 0xAA);
    assert_eq!(gsu.rambr, 0x01);
    assert!(!gsu.ram_buffer_pending);
}

#[test]
fn sbk_stores_back_to_base_of_last_word_access() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.write_ram_word(0x2000, 0x1122);
    gsu.write_ram_word(0x2002, 0x3344);
    gsu.src_reg = 1;
    gsu.regs[1] = 0xA1B2;

    let _ = gsu.read_ram_word(0x2000);
    assert!(gsu.execute_opcode_internal(0x90, &rom, 0x8000, false));

    assert_eq!(gsu.read_ram_word(0x2000), 0xA1B2);
    assert_eq!(gsu.read_ram_word(0x2002), 0x3344);
}

#[test]
fn read_program_rom_byte_uses_high_32k_rom_windows() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x8001] = 0xA5;

    assert_eq!(gsu.read_program_rom_byte(&rom, 0x01, 0x8001), Some(0xA5));
    assert_eq!(gsu.read_program_rom_byte(&rom, 0x01, 0x0001), Some(0xA5));
}

#[test]
fn read_program_rom_byte_prefers_cache_window_over_rom() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0034] = 0x11;
    gsu.cache_enabled = true;
    gsu.cache_ram[0x34] = 0xA5;
    gsu.cache_valid_mask = u32::MAX;

    assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x0034), Some(0xA5));
}

#[test]
fn cache_opcode_invalidates_lines_and_refills_on_demand() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0230] = 0xA5;
    rom[0x0235] = 0x5A;
    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8235;

    assert!(gsu.execute_opcode_internal(0x02, &rom, 0x8234, false));
    assert!(gsu.cache_enabled);
    assert_eq!(gsu.cbr, 0x8230);
    assert_eq!(gsu.cache_valid_mask, 0);
    assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x8235), Some(0x5A));
    assert_ne!(gsu.cache_valid_mask & 1, 0);
    assert_eq!(gsu.cache_ram[0x00], 0xA5);
    assert_eq!(gsu.cache_ram[0x05], 0x5A);
}

#[test]
fn cache_opcode_keeps_valid_lines_when_base_is_unchanged() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8235;
    gsu.cbr = 0x8230;
    gsu.cache_enabled = true;
    gsu.cache_valid_mask = 0x1234_5678;
    gsu.cache_ram[0x35] = 0xA5;

    assert!(gsu.execute_opcode_internal(0x02, &rom, 0x8234, false));
    assert!(gsu.cache_enabled);
    assert_eq!(gsu.cbr, 0x8230);
    assert_eq!(gsu.cache_valid_mask, 0x1234_5678);
    assert_eq!(gsu.cache_ram[0x35], 0xA5);
}

#[test]
fn cache_opcode_uses_prefetched_r15_window_at_16byte_boundary() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.pbr = 0x01;
    // snes9x's fx_cache uses R15, and under the pipelined core that points at the
    // prefetched stream. Star Fox later executes 01:84FB from cache page 000B,
    // which requires CACHE at 01:84EE to land on 0x84F0.
    gsu.regs[15] = 0x84F0;

    assert!(gsu.execute_opcode_internal(0x02, &rom, 0x84EE, false));
    assert!(gsu.cache_enabled);
    assert_eq!(gsu.cbr, 0x84F0);
}

#[test]
fn cache_fetch_uses_cbr_relative_window() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x1230] = 0x9A;
    rom[0x1231] = 0xBC;
    gsu.cache_enabled = true;
    gsu.cbr = 0x9230;

    assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x9230), Some(0x9A));
    assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x9231), Some(0xBC));
    assert_eq!(gsu.cache_ram[0x00], 0x9A);
    assert_eq!(gsu.cache_ram[0x01], 0xBC);
}

#[test]
fn read_program_rom_byte_uses_rom_outside_cache_window() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x2234] = 0x5A;
    gsu.cbr = 0x1200;
    gsu.cache_enabled = true;
    gsu.cache_ram[0x34] = 0xA5;
    gsu.cache_valid_mask = u32::MAX;

    assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0xA234), Some(0x5A));
}

#[test]
fn read_program_rom_byte_reads_program_ram_banks() {
    let rom = vec![0u8; 0x20_0000];
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.game_ram[0x1234] = 0xA5;
    gsu.game_ram[0x1_1234] = 0x5A;

    assert_eq!(gsu.read_program_rom_byte(&rom, 0x70, 0x1234), Some(0xA5));
    assert_eq!(gsu.read_program_rom_byte(&rom, 0x71, 0x1234), Some(0x5A));
}

#[test]
fn read_program_rom_byte_wraps_32k_rom_banks_through_rom_size() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x10_000];
    rom[0x0000] = 0x5A;
    rom[0x8000] = 0xA5;

    assert_eq!(gsu.read_program_rom_byte(&rom, 0x00, 0x8000), Some(0x5A));
    assert_eq!(gsu.read_program_rom_byte(&rom, 0x01, 0x8000), Some(0xA5));
    assert_eq!(gsu.read_program_rom_byte(&rom, 0x02, 0x8000), Some(0x5A));
}

#[test]
fn default_steps_per_cpu_cycle_tracks_clsr_speed_mode() {
    // CLSR bit 0: 0 = standard 10.738 MHz (SLOW), 1 = turbo 21.477 MHz (FAST)
    let standard = SuperFx::new(0x20_0000); // clsr=0 → standard speed
    let mut turbo = SuperFx::new(0x20_0000);
    turbo.clsr = 0x01; // clsr=1 → turbo speed

    assert_eq!(
        standard.steps_per_cpu_cycle(),
        super::DEFAULT_SUPERFX_RATIO_SLOW
    );
    assert_eq!(
        turbo.steps_per_cpu_cycle(),
        super::DEFAULT_SUPERFX_RATIO_FAST
    );
}

#[test]
fn superfx_cpu_ratio_env_overrides_default_speed() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var("SUPERFX_CPU_RATIO").ok();
    std::env::set_var("SUPERFX_CPU_RATIO", "7");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.clsr = 0x01;
    assert_eq!(gsu.steps_per_cpu_cycle(), 7);

    if let Some(value) = prev {
        std::env::set_var("SUPERFX_CPU_RATIO", value);
    } else {
        std::env::remove_var("SUPERFX_CPU_RATIO");
    }
}

#[test]
fn superfx_status_poll_boost_env_overrides_default_value() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var("SUPERFX_STATUS_POLL_BOOST").ok();
    std::env::set_var("SUPERFX_STATUS_POLL_BOOST", "96");

    assert_eq!(SuperFx::status_poll_step_budget(), 96);

    if let Some(value) = prev {
        std::env::set_var("SUPERFX_STATUS_POLL_BOOST", value);
    } else {
        std::env::remove_var("SUPERFX_STATUS_POLL_BOOST");
    }
}

#[test]
fn debug_in_starfox_cached_delay_loop_matches_expected_signature() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x8EBC;
    gsu.regs[11] = 0x8615;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000C;

    assert!(gsu.debug_in_starfox_cached_delay_loop());

    gsu.regs[11] = 0x8609;
    assert!(gsu.debug_in_starfox_cached_delay_loop());

    gsu.regs[11] = 0x8614;
    assert!(!gsu.debug_in_starfox_cached_delay_loop());

    gsu.regs[11] = 0x8608;
    assert!(!gsu.debug_in_starfox_cached_delay_loop());
}

#[test]
fn debug_in_starfox_cached_delay_loop_ignores_r0_data_value() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x0000;
    gsu.regs[11] = 0x8615;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000B;
    assert!(gsu.debug_in_starfox_cached_delay_loop());

    gsu.regs[0] = 0x8EBC;
    assert!(gsu.debug_in_starfox_cached_delay_loop());
}

#[test]
fn fast_forward_starfox_cached_delay_loop_collapses_r12_to_zero() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x0000;
    gsu.regs[11] = 0x8615;
    gsu.regs[12] = 0xBC8E;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000B;
    gsu.sfr = SFR_S_BIT;

    assert!(gsu.fast_forward_starfox_cached_delay_loop());
    assert_eq!(gsu.regs[12], 0x0000);
    assert_eq!(gsu.regs[15], 0x000C);
    assert!(!gsu.pipe_valid);
    assert!(gsu.sfr & SFR_Z_BIT != 0);
    assert!(gsu.sfr & SFR_S_BIT == 0);
}

#[test]
fn status_poll_late_wait_assist_can_exit_after_cached_delay_loop() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.cache_valid_mask = u32::MAX;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x0000;
    gsu.regs[11] = 0x8615;
    gsu.regs[12] = 0xBC8E;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000B;
    gsu.cache_ram[0x000C] = 0x00;

    gsu.run_status_poll_until_stop_with_starfox_late_wait_assist(&rom, 4);

    assert!(!gsu.running);
    assert_eq!(gsu.regs[12], 0x0000);
    assert_eq!(gsu.regs[15], 0x000E);
}

#[test]
fn status_poll_until_sfr_low_mask_changes_stops_after_go_bit_clears() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.running = true;
    gsu.cache_enabled = true;
    gsu.cache_valid_mask = u32::MAX;
    gsu.pbr = 0x01;
    gsu.cbr = 0x84F0;
    gsu.regs[0] = 0x0000;
    gsu.regs[11] = 0x8615;
    gsu.regs[12] = 0xBC8E;
    gsu.regs[13] = 0x000B;
    gsu.regs[15] = 0x000B;
    gsu.sfr = SFR_GO_BIT;
    gsu.cache_ram[0x000C] = 0x00;

    let initial_low = gsu.observed_sfr_low();
    assert_ne!(initial_low & (SFR_GO_BIT as u8), 0);

    gsu.run_status_poll_until_sfr_low_mask_changes(&rom, initial_low, SFR_GO_BIT as u8, 4);

    assert!(!gsu.running);
    assert_eq!(gsu.observed_sfr_low() & (SFR_GO_BIT as u8), 0);
    assert_eq!(gsu.regs[15], 0x000E);
}

#[test]
fn starfox_live_producer_wait_assist_can_run_until_stop() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x01_B384] = 0x00;
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.rambr = 0x00;
    gsu.regs[13] = 0xB384;
    gsu.regs[15] = 0xB384;
    gsu.sfr = SFR_GO_BIT;

    gsu.run_status_poll_until_go_clears_in_starfox_live_producer_loop(&rom, 4);

    assert!(!gsu.running);
    assert_eq!(gsu.observed_sfr_low() & (SFR_GO_BIT as u8), 0);
}

#[test]
fn starfox_live_producer_wait_assist_stops_after_leaving_producer_band() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.rambr = 0x00;
    gsu.regs[13] = 0xD1B4;
    gsu.regs[15] = 0xD1B4;
    gsu.sfr = SFR_GO_BIT;

    gsu.run_status_poll_until_go_clears_in_starfox_live_producer_loop(&rom, 8);

    assert!(gsu.running);
    assert_eq!(gsu.regs[13], 0xD1B4);
}

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

fn write_test_data_rom_byte(rom: &mut [u8], bank: u8, addr: u16, value: u8) {
    let bank = bank & 0x7F;
    let full_addr = ((bank as usize) << 16) | addr as usize;
    let offset = if (full_addr & 0xE0_0000) == 0x40_0000 {
        full_addr
    } else {
        ((full_addr & 0x3F_0000) >> 1) | (full_addr & 0x7FFF)
    };
    rom[offset % rom.len()] = value;
}

fn make_b301_packet_transform_state(r14_packet: u16) -> (SuperFx, Vec<u8>) {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];

    let program = [
        0x3D, 0xA0, 0x32, 0x3F, 0xDF, 0x60, 0x3E, 0xDF, 0x3D, 0xAE, 0x31, 0xEE, 0x11, 0xEF, 0xEE,
        0x21, 0x3D, 0xEF, 0xEE, 0xEE, 0xEE, 0x13, 0xEF, 0xEE, 0x23, 0x3D, 0xEF, 0xEE, 0x12, 0xEF,
        0xEE, 0x22, 0x3D, 0xEF, 0x3D, 0xA9, 0x16,
    ];
    let program_base = 0x01usize * 0x8000 + (0xB301usize & 0x7FFF);
    rom[program_base..program_base + program.len()].copy_from_slice(&program);

    write_test_data_rom_byte(&mut rom, 0x14, 0xD895, 0x00);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD894, 0x20);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD891, 0x06);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD890, 0x02);

    write_test_data_rom_byte(&mut rom, 0x14, 0xD5A9, 0x00);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD5A8, 0x0A);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD5A5, 0x12);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD5A4, 0x00);

    gsu.pbr = 0x01;
    gsu.rombr = 0x00;
    gsu.regs[0] = 0x0824;
    gsu.regs[1] = 0x5191;
    gsu.regs[2] = 0x5190;
    gsu.regs[3] = 0x0700;
    gsu.regs[4] = 0x0000;
    gsu.regs[5] = 0xC7F8;
    gsu.regs[6] = 0x0153;
    gsu.regs[7] = 0xB4B6;
    gsu.regs[8] = 0xB337;
    gsu.regs[9] = 0x2800;
    gsu.regs[10] = 0x0000;
    gsu.regs[11] = 0xB33E;
    gsu.regs[12] = 0x0000;
    gsu.regs[13] = 0xB3DE;
    gsu.write_reg(14, 0x6242);
    gsu.regs[15] = 0xB301;
    gsu.game_ram[0x0062] = (r14_packet & 0x00FF) as u8;
    gsu.game_ram[0x0063] = (r14_packet >> 8) as u8;
    gsu.game_ram[0x0064] = 0x14;
    gsu.game_ram[0x002C] = 0x00;
    gsu.game_ram[0x002D] = 0x40;
    gsu.debug_prepare_cpu_start(&rom);

    (gsu, rom)
}

#[test]
fn b301_packet_transform_consumes_d896_stream() {
    let (mut gsu, rom) = make_b301_packet_transform_state(0xD896);

    gsu.run_steps(&rom, 34);

    assert_eq!(gsu.regs[1], 0x2000);
    assert_eq!(gsu.regs[2], 0x0000);
    assert_eq!(gsu.regs[3], 0x0206);
    assert_eq!(gsu.regs[9], 0x4000);
    assert_eq!(gsu.regs[14], 0xD88E);
    assert_eq!(gsu.regs[15], 0xB327);
}

#[test]
fn b301_packet_transform_consumes_d5aa_stream() {
    let (mut gsu, rom) = make_b301_packet_transform_state(0xD5AA);

    gsu.run_steps(&rom, 34);

    assert_eq!(gsu.regs[1], 0x0A00);
    assert_eq!(gsu.regs[2], 0x0000);
    assert_eq!(gsu.regs[3], 0x0012);
    assert_eq!(gsu.regs[9], 0x4000);
    assert_eq!(gsu.regs[14], 0xD5A2);
    assert_eq!(gsu.regs[15], 0xB327);
}

#[test]
fn d4b4_success_tail_loads_record_fields_into_r9_r13_and_r6() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    let base = 0x01usize * 0x8000 + (0xD4B4usize & 0x7FFF);
    rom[base..base + 44].copy_from_slice(&[
        0xA2, 0x08, 0x22, 0x5B, 0x12, 0x42, 0x22, 0x19, 0x52, 0x90, 0x3D, 0xA4, 0x14, 0x12, 0x54,
        0xA0, 0x05, 0x5B, 0x3D, 0x40, 0x95, 0xA6, 0x0A, 0x26, 0x5B, 0x16, 0x46, 0x26, 0x1D, 0x56,
        0x90, 0x3D, 0xA4, 0x15, 0x16, 0x54, 0xA5, 0x01, 0x25, 0x5B, 0x15, 0x3D, 0x45, 0x00,
    ]);

    gsu.pbr = 0x01;
    gsu.rombr = 0x14;
    gsu.regs[0] = 0x0031;
    gsu.regs[1] = 0xE2DA;
    gsu.regs[2] = 0x004B;
    gsu.regs[3] = 0x0000;
    gsu.regs[4] = 0xE2D3;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[9] = 0x004A;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AD6;
    gsu.regs[12] = 0x0129;
    gsu.regs[13] = 0xD1B4;
    gsu.write_reg(14, 0x8A9D);
    gsu.regs[15] = 0xD4B4;
    gsu.game_ram[0x0028] = 0x2B;
    gsu.game_ram[0x0029] = 0x01;
    gsu.game_ram[0x002A] = 0x0A;
    gsu.game_ram[0x002B] = 0x61;
    gsu.game_ram[0x1ADB] = 0xF9;
    gsu.game_ram[0x1ADE] = 0x32;
    gsu.game_ram[0x1ADF] = 0x00;
    gsu.game_ram[0x1AE0] = 0xF9;
    gsu.game_ram[0x1AE1] = 0xFF;
    gsu.running = true;

    gsu.run_steps(&rom, 64);

    assert_eq!(gsu.regs[2], 0x018E);
    assert_eq!(gsu.regs[6], 0x60FC);
    assert_eq!(gsu.regs[9], 0x0032);
    assert_eq!(gsu.regs[13], 0xFFF9);
    assert_eq!(gsu.regs[15], 0xD4E1);
}

#[test]
fn d4b4_success_tail_rearms_match_word_after_loading_zero_cursor() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    let base = 0x01usize * 0x8000 + (0xD4B4usize & 0x7FFF);
    rom[base..base + 44].copy_from_slice(&[
        0xA2, 0x08, 0x22, 0x5B, 0x12, 0x42, 0x22, 0x19, 0x52, 0x90, 0x3D, 0xA4, 0x14, 0x12, 0x54,
        0xA0, 0x05, 0x5B, 0x3D, 0x40, 0x95, 0xA6, 0x0A, 0x26, 0x5B, 0x16, 0x46, 0x26, 0x1D, 0x56,
        0x90, 0x3D, 0xA4, 0x15, 0x16, 0x54, 0xA5, 0x01, 0x25, 0x5B, 0x15, 0x3D, 0x45, 0x00,
    ]);

    gsu.pbr = 0x01;
    gsu.rombr = 0x14;
    gsu.regs[0] = 0x0031;
    gsu.regs[1] = 0xE2DA;
    gsu.regs[2] = 0x004B;
    gsu.regs[3] = 0x0000;
    gsu.regs[4] = 0xE2D3;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[9] = 0x004A;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AD6;
    gsu.regs[12] = 0x0129;
    gsu.regs[13] = 0xD1B4;
    gsu.write_reg(14, 0x8A7B);
    gsu.regs[15] = 0xD4B4;
    gsu.game_ram[0x0028] = 0x2B;
    gsu.game_ram[0x0029] = 0x01;
    gsu.game_ram[0x002A] = 0x0A;
    gsu.game_ram[0x002B] = 0x61;
    gsu.game_ram[0x1ADB] = 0xF9;
    gsu.game_ram[0x1ADE] = 0x32;
    gsu.game_ram[0x1ADF] = 0x00;
    gsu.game_ram[0x1AE0] = 0x00;
    gsu.game_ram[0x1AE1] = 0x00;
    gsu.running = true;

    gsu.run_steps(&rom, 64);

    assert_eq!(gsu.regs[13], 0x0000);
    assert_eq!(gsu.read_ram_word(0x1AE0), 0xFFF9);
    assert_eq!(gsu.regs[15], 0xD4E1);
}

#[test]
fn d496_success_prelude_builds_continuation_fields_before_tail() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    let base = 0x01usize * 0x8000 + (0xD496usize & 0x7FFF);
    rom[base..base + 33].copy_from_slice(&[
        0x2B, 0x3E, 0x6C, 0xA0, 0x03, 0x5B, 0x3D, 0x40, 0x95, 0xA1, 0x06, 0x21, 0x5B, 0x11, 0x41,
        0x21, 0x13, 0x51, 0x90, 0x3D, 0xA4, 0x13, 0x11, 0x54, 0xA0, 0x04, 0x5B, 0x3D, 0x40, 0x95,
        0xA2, 0x08, 0x00,
    ]);

    gsu.pbr = 0x01;
    gsu.rombr = 0x14;
    gsu.regs[0] = 0x0000;
    gsu.regs[1] = 0x00FC;
    gsu.regs[2] = 0x004B;
    gsu.regs[3] = 0x2B14;
    gsu.regs[4] = 0x0006;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[9] = 0x004A;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AE2;
    gsu.regs[12] = 0x0129;
    gsu.regs[13] = 0xD1B4;
    gsu.write_reg(14, 0x8A7B);
    gsu.regs[15] = 0xD496;
    gsu.game_ram[0x0026] = 0xD3;
    gsu.game_ram[0x0027] = 0xE2;
    gsu.game_ram[0x1AD9] = 0x07;
    gsu.game_ram[0x1ADC] = 0x00;
    gsu.game_ram[0x1ADD] = 0x00;
    gsu.game_ram[0x1ADA] = 0x32;
    gsu.running = true;

    gsu.run_steps(&rom, 27);

    assert_eq!(gsu.regs[1], 0xE2DA);
    assert_eq!(gsu.regs[2], 0x0008);
    assert_eq!(gsu.regs[3], 0x0000);
    assert_eq!(gsu.regs[4], 0xE2D3);
    assert_eq!(gsu.regs[9], 0x004A);
    assert_eq!(gsu.regs[11], 0x1AD6);
    assert_eq!(gsu.regs[13], 0xD1B4);
    assert_eq!(gsu.regs[15], 0xD4B7);
}

#[test]
fn af70_continuation_stream_jumps_to_target_when_pointer_is_nonzero() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x000D].copy_from_slice(&[
        0x11, // TO R1
        0x4A, // LDW [R10] -> R1
        0x11, // TO R1
        0x41, // LDW [R1] -> R1
        0x21, // WITH R1
        0xB1, // MOVES R1 -> R1 (updates Z)
        0x09, 0x05, // BEQ +5 (not taken)
        0x01, // NOP
        0xFF, 0x00, 0x90, // IWT R15,#9000
        0x01, // stale old-stream byte executes once before the transfer
    ]);
    rom[0x1000] = 0x00; // STOP at $9000

    gsu.pbr = 0x00;
    gsu.regs[10] = 0x04C4;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;
    gsu.write_ram_word(0x04C4, 0x887F);
    gsu.write_ram_word(0x887F, 0x29E3);

    gsu.run_steps(&rom, 11);

    assert_eq!(gsu.regs[1], 0x29E3);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x9002);
    assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
}

#[test]
fn af70_continuation_stream_branches_locally_when_pointer_is_zero() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x000E].copy_from_slice(&[
        0x11, // TO R1
        0x4A, // LDW [R10] -> R1
        0x11, // TO R1
        0x41, // LDW [R1] -> R1
        0x21, // WITH R1
        0xB1, // MOVES R1 -> R1 (updates Z)
        0x09, 0x05, // BEQ +5 (taken)
        0x01, // skipped NOP
        0xFF, 0x00, 0x90, // skipped IWT R15,#9000
        0x00, // padding at $800C
        0x00, // STOP at branch target $800D
    ]);

    gsu.pbr = 0x00;
    gsu.regs[10] = 0x04C4;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;
    gsu.write_ram_word(0x04C4, 0x887F);
    gsu.write_ram_word(0x887F, 0x0000);

    gsu.run_steps(&rom, 7);

    assert_eq!(gsu.regs[1], 0x0000);
    assert!(gsu.running());
    assert_eq!(gsu.regs[15], 0x800D);
    assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
}

#[test]
fn d1d0_success_fragment_branches_after_writing_record_when_r8_is_not_one() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0012].copy_from_slice(&[
        0x53, // ADD R3 -> R7
        0xB5, // FROM R5
        0x37, // STW [R7], R5
        0xB6, // FROM R6
        0x3D, // ALT1
        0x33, // STB [R3], R6
        0xA0, 0x01, // IBT R0,#1
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5 (taken)
        0x01, // skipped NOP
        0xFF, 0xFD, 0xD3, // skipped IWT R15,#D3FD
        0x00, // padding at $8010
        0x00, // STOP at branch target $8011
    ]);

    gsu.pbr = 0x00;
    gsu.regs[3] = 0x1AD6;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x000C;
    gsu.regs[8] = 0x0003;
    gsu.regs[15] = 0x8000;
    gsu.src_reg = 7;
    gsu.dst_reg = 7;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 14);

    assert_eq!(gsu.regs[7], 0x1AE2);
    assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
    assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x8013);
}

#[test]
fn d1d0_success_fragment_falls_through_to_iwt_when_r8_is_one() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0011].copy_from_slice(&[
        0x53, // ADD R3 -> R7
        0xB5, // FROM R5
        0x37, // STW [R7], R5
        0xB6, // FROM R6
        0x3D, // ALT1
        0x33, // STB [R3], R6
        0xA0, 0x01, // IBT R0,#1
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5 (not taken)
        0x01, // NOP
        0xFF, 0xFD, 0xD3, // IWT R15,#D3FD
        0xD0, // delay slot: INC R0
    ]);
    rom[0x53FD] = 0x00; // STOP at $D3FD

    gsu.pbr = 0x00;
    gsu.regs[3] = 0x1AD6;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x000C;
    gsu.regs[8] = 0x0001;
    gsu.regs[15] = 0x8000;
    gsu.src_reg = 7;
    gsu.dst_reg = 7;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 14);

    assert_eq!(gsu.regs[7], 0x1AE2);
    assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
    assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
    assert_eq!(gsu.regs[0], 0x0002);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0xD3FF);
}

#[test]
fn d1d0_success_fragment_dispatches_to_d316_when_r8_is_three() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0027].copy_from_slice(&[
        0x53, // ADD R3 -> R7
        0xB5, // FROM R5
        0x37, // STW [R7], R5
        0xB6, // FROM R6
        0x3D, // ALT1
        0x33, // STB [R3], R6
        0xA0, 0x01, // IBT R0,#1
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5
        0x01, // NOP
        0xFF, 0xFD, 0xD3, // IWT R15,#D3FD
        0x01, // delay slot
        0xA0, 0x02, // IBT R0,#2
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5
        0x01, // NOP
        0xFF, 0x87, 0xD3, // IWT R15,#D387
        0x01, // delay slot
        0xA0, 0x03, // IBT R0,#3
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5 (not taken)
        0x01, // NOP
        0xFF, 0x16, 0xD3, // IWT R15,#D316
        0xD0, // delay slot: INC R0
    ]);
    rom[0x5316] = 0x00; // STOP at $D316

    gsu.pbr = 0x00;
    gsu.regs[3] = 0x1AD6;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x000C;
    gsu.regs[8] = 0x0003;
    gsu.regs[15] = 0x8000;
    gsu.src_reg = 7;
    gsu.dst_reg = 7;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 24);

    assert_eq!(gsu.regs[7], 0x1AE2);
    assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
    assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
    assert_eq!(gsu.regs[0], 0x0004);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0xD318);
}

fn run_d316_success_fragment_sample(
    r2_start: u16,
    r3_start: u16,
    r4_start: u16,
    r9_start: u16,
    r12_start: u16,
    expected_r0: u16,
    expected_r2: u16,
    expected_r4: u16,
) {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    let base = 0x01usize * 0x8000;
    rom[base..base + 43].copy_from_slice(&[
        0x60, // SUB R0 -> R0
        0xA7, 0x01, // IBT R7,#1
        0x27, // WITH R7
        0x53, // ADD R3 -> R7
        0xB0, // FROM R0
        0x3D, // ALT1
        0x37, // STB [R7], R0
        0xB2, // FROM R2
        0x4D, // SWAP
        0x97, // ROR
        0x52, // ADD R2
        0x12, // TO R2
        0x3D, // ALT1
        0x52, // ADC R2
        0xA0, 0x0F, // IBT R0,#0F
        0x72, // AND R2
        0xA7, 0x07, // IBT R7,#7
        0x67, // SUB R7
        0xA7, 0x03, // IBT R7,#3
        0x27, // WITH R7
        0x53, // ADD R3 -> R7
        0xB0, // FROM R0
        0x3D, // ALT1
        0x37, // STB [R7], R0
        0x20, // WITH R0
        0xB0, // MOVES R0
        0x0A, 0x03, // BPL +3
        0x01, // NOP
        0x4F, // NOT
        0xD0, // INC R0
        0x20, // WITH R0
        0x14, // TO R4
        0xB2, // FROM R2
        0x4D, // SWAP
        0x97, // ROR
        0x52, // ADD R2
        0x12, // TO R2
        0x00, // STOP before the next ALT1/ADC stage
    ]);

    gsu.pbr = 0x01;
    gsu.regs[0] = 0x0003;
    gsu.regs[1] = 0x00FC;
    gsu.regs[2] = r2_start;
    gsu.regs[3] = r3_start;
    gsu.regs[4] = r4_start;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = r3_start.wrapping_add(0x000C);
    gsu.regs[8] = 0x0003;
    gsu.regs[9] = r9_start;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0xAD88;
    gsu.regs[12] = r12_start;
    gsu.regs[13] = 0xD1B4;
    gsu.regs[14] = 0x8A7B;
    gsu.regs[15] = 0x8000;
    gsu.src_reg = 0;
    gsu.dst_reg = 0;
    gsu.running = true;
    gsu.sfr = 0x0066 | SFR_GO_BIT;
    gsu.game_ram[r3_start as usize + 1] = 0xA2;
    gsu.game_ram[r3_start as usize + 3] = 0xFF;

    gsu.run_steps(&rom, 64);

    assert_eq!(gsu.regs[0], expected_r0);
    assert_eq!(gsu.regs[2], expected_r2);
    assert_eq!(gsu.regs[4], expected_r4);
    assert_eq!(gsu.regs[7], r3_start.wrapping_add(3));
    assert_eq!(gsu.game_ram[r3_start as usize + 1], 0x00);
    assert_eq!(gsu.game_ram[r3_start as usize + 3], expected_r4 as u8);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x802C);
}

#[test]
fn d316_success_fragment_matches_live_trace_for_88ed_sample() {
    run_d316_success_fragment_sample(
        0x88ED, 0x1AD6, 0x00FB, 0x004E, 0x0129, 0xD7E2, 0x889E, 0x0007,
    );
}

#[test]
fn d316_success_fragment_matches_live_trace_for_1a60_sample() {
    run_d316_success_fragment_sample(
        0x1A60, 0x1D3E, 0x000E, 0x004D, 0x00FD, 0xCB7F, 0x64CD, 0x0006,
    );
}

#[test]
fn d316_success_fragment_matches_live_trace_for_65a3_sample() {
    run_d316_success_fragment_sample(
        0x65A3, 0x1F0C, 0x000D, 0x004C, 0x00DC, 0x9906, 0x1CF8, 0x0001,
    );
}

#[test]
fn late_search_key_override_uses_match_word_when_raw_key_is_missing() {
    let prev = std::env::var_os("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2");
    std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.regs[7] = 0x5A89;
    gsu.regs[15] = 0xD47A;
    gsu.write_ram_word(0x1AE0, 0xFFF9);
    gsu.write_ram_word(0x1AE2, 0x004B);
    gsu.write_ram_word(0x1AB8, 0x004B);

    gsu.maybe_force_starfox_late_search_key_from_match();
    assert_eq!(gsu.regs[7], 0x004B);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2");
    }
}

#[test]
fn parser_key_override_uses_match_word_when_ad46_writes_missing_head_key() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD");
    std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xAD46;
    gsu.current_exec_opcode = 0xA0;
    gsu.write_ram_word(0x1AE0, 0xFFF9);
    gsu.write_ram_word(0x1AE2, 0x004B);
    gsu.write_ram_word(0x1AB8, 0x004B);

    gsu.write_ram_word(0x0136, 0x5ECF);

    assert_eq!(gsu.debug_read_ram_word_short(0x0136), 0x004B);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD");
    }
}

#[test]
fn parser_key_override_can_promote_any_table_field_to_record_head() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD");
    std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xAD46;
    gsu.current_exec_opcode = 0xA0;
    gsu.write_ram_word_short(0x1AB8, 0x004B);
    gsu.write_ram_word_short(0x1AB8 + 12, 0x5A89);

    gsu.write_ram_word_short(0x0136, 0x5A89);
    assert_eq!(gsu.debug_read_ram_word_short(0x0136), 0x004B);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD");
    }
}

#[test]
fn late_search_key_override_can_promote_any_table_field_to_record_head() {
    let prev = std::env::var_os("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD");
    std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.regs[7] = 0x5A89;
    gsu.regs[15] = 0xD47A;
    gsu.write_ram_word_short(0x1AB8, 0x004B);
    gsu.write_ram_word_short(0x1AB8 + 12, 0x5A89);

    gsu.maybe_force_starfox_late_search_key_from_match();
    assert_eq!(gsu.regs[7], 0x004B);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD");
    }
}

#[test]
fn continuation_ptr_override_redirects_887f_write_to_match_fragment() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT");
    std::env::set_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xB396;
    gsu.current_exec_opcode = 0x31;
    gsu.write_ram_word_short(0x888C, 0x4BFC);
    gsu.game_ram[0x021F] = 0x88;

    gsu.write_ram_byte(0x021E, 0x7F);

    assert_eq!(gsu.debug_read_ram_word_short(0x021E), 0x888D);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT");
    }
}

#[test]
fn continuation_cursor_override_redirects_04c4_887f_to_match_fragment() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT");
    std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xACAD;
    gsu.write_ram_word_short(0x888C, 0x4BFC);

    gsu.write_ram_word(0x04C4, 0x887F);

    assert_eq!(gsu.debug_read_ram_word_short(0x04C4), 0x888D);

    if let Some(value) = prev {
        std::env::set_var(
            "STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT",
            value,
        );
    } else {
        std::env::remove_var("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT");
    }
}

#[test]
fn continuation_cursor_override_accepts_explicit_env_value() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE");
    std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE", "8890");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xACAD;

    gsu.write_ram_word(0x04C4, 0x887F);

    assert_eq!(gsu.debug_read_ram_word_short(0x04C4), 0x8890);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE");
    }
}

#[test]
fn continuation_cursor_override_can_null_stream_after_success_fragment() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS");
    std::env::set_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xACAD;
    gsu.write_ram_word_short(0x1AE0, 0xFFF9);
    gsu.write_ram_word_short(0x1AE2, 0x004B);
    gsu.write_ram_word_short(0x888C, 0x4BFC);

    assert_eq!(
        gsu.maybe_force_starfox_continuation_cursor_word(0x04C4, 0x29E3),
        0x0000
    );

    if let Some(value) = prev {
        std::env::set_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS", value);
    } else {
        std::env::remove_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS");
    }
}

#[test]
fn success_cursor_override_keeps_1ae0_armed_at_d1cc() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_CURSOR_ARMED");
    std::env::set_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xD1CC;

    gsu.write_ram_word(0x1AE0, 0x0000);

    assert_eq!(gsu.debug_read_ram_word_short(0x1AE0), 0xFFF9);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED", value);
    } else {
        std::env::remove_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED");
    }
}

#[test]
fn success_branch_target_override_keeps_r13_at_d1b4() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_BRANCH_TARGET");
    std::env::set_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.regs[13] = 0xD1B4;

    gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

    assert_eq!(gsu.regs[13], 0xD1B4);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET", value);
    } else {
        std::env::remove_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET");
    }
}

#[test]
fn success_branch_target_override_can_redirect_tail_to_b196() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196");
    std::env::set_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.regs[13] = 0xD1B4;

    gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

    assert_eq!(gsu.regs[13], 0xB196);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196");
    }
}

#[test]
fn b30a_r14_seed_override_applies_only_on_matching_frame_and_pc() {
    let _guard = env_lock().lock().unwrap();
    let prev_value = std::env::var_os("STARFOX_FORCE_B30A_R14_VALUE");
    let prev_frame = std::env::var_os("STARFOX_FORCE_B30A_R14_FRAME");
    std::env::set_var("STARFOX_FORCE_B30A_R14_VALUE", "0x0000");
    std::env::set_var("STARFOX_FORCE_B30A_R14_FRAME", "163");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;

    super::set_trace_superfx_exec_frame(163);
    assert_eq!(
        gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30A),
        0x0000
    );
    assert_eq!(
        gsu.maybe_force_starfox_b30a_r14_seed(13, 0xD896, 0xB30A),
        0xD896
    );
    assert_eq!(
        gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30B),
        0xD896
    );

    super::set_trace_superfx_exec_frame(164);
    assert_eq!(
        gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30A),
        0xD896
    );

    if let Some(value) = prev_value {
        std::env::set_var("STARFOX_FORCE_B30A_R14_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B30A_R14_VALUE");
    }
    if let Some(value) = prev_frame {
        std::env::set_var("STARFOX_FORCE_B30A_R14_FRAME", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B30A_R14_FRAME");
    }
}

#[test]
fn b380_r12_seed_override_applies_only_on_matching_frame_and_pc() {
    let _guard = env_lock().lock().unwrap();
    let prev_value = std::env::var_os("STARFOX_FORCE_B380_R12_VALUE");
    let prev_frame = std::env::var_os("STARFOX_FORCE_B380_R12_FRAME");
    std::env::set_var("STARFOX_FORCE_B380_R12_VALUE", "6");
    std::env::set_var("STARFOX_FORCE_B380_R12_FRAME", "163");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;

    super::set_trace_superfx_exec_frame(163);
    assert_eq!(
        gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB380),
        0x0006
    );
    assert_eq!(
        gsu.maybe_force_starfox_b380_r12_seed(11, 0x0008, 0xB380),
        0x0008
    );
    assert_eq!(
        gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB381),
        0x0008
    );

    super::set_trace_superfx_exec_frame(164);
    assert_eq!(
        gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB380),
        0x0008
    );

    if let Some(value) = prev_value {
        std::env::set_var("STARFOX_FORCE_B380_R12_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B380_R12_VALUE");
    }
    if let Some(value) = prev_frame {
        std::env::set_var("STARFOX_FORCE_B380_R12_FRAME", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B380_R12_FRAME");
    }
}

#[test]
fn b384_preexec_live_state_override_applies_only_on_matching_frame_and_pc() {
    let _guard = env_lock().lock().unwrap();
    let prev_r12 = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_R12_VALUE");
    let prev_r14 = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_R14_VALUE");
    let prev_frame = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_FRAME");
    std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE", "6");
    std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE", "0");
    std::env::set_var("STARFOX_FORCE_B384_PREEXEC_FRAME", "163");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.regs[12] = 0x0008;
    gsu.regs[14] = 0xCD92;

    super::set_trace_superfx_exec_frame(163);
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB384);
    assert_eq!(gsu.regs[12], 0x0006);
    assert_eq!(gsu.regs[14], 0x0000);

    gsu.regs[12] = 0x0008;
    gsu.regs[14] = 0xCD92;
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB388);
    assert_eq!(gsu.regs[12], 0x0006);
    assert_eq!(gsu.regs[14], 0x0000);

    gsu.regs[12] = 0x0008;
    gsu.regs[14] = 0xCD92;
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB396);
    assert_eq!(gsu.regs[12], 0x0006);
    assert_eq!(gsu.regs[14], 0x0000);

    gsu.regs[12] = 0x0008;
    gsu.regs[14] = 0xCD92;
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB397);
    assert_eq!(gsu.regs[12], 0x0008);
    assert_eq!(gsu.regs[14], 0xCD92);

    super::set_trace_superfx_exec_frame(164);
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB384);
    assert_eq!(gsu.regs[12], 0x0008);
    assert_eq!(gsu.regs[14], 0xCD92);

    if let Some(value) = prev_r12 {
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE");
    }
    if let Some(value) = prev_r14 {
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE");
    }
    if let Some(value) = prev_frame {
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_FRAME", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_FRAME");
    }
}

#[test]
fn success_context_override_keeps_r9_and_r13_when_success_tail_zeroes_them() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_CONTEXT");
    std::env::set_var("STARFOX_KEEP_SUCCESS_CONTEXT", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.regs[7] = 0x004B;
    gsu.regs[9] = 0x004A;
    gsu.regs[13] = 0xD1B4;

    gsu.write_reg_exec(9, 0x0000, 0x19, 0xD4BB);
    gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

    assert_eq!(gsu.regs[9], 0x004A);
    assert_eq!(gsu.regs[13], 0xD1B4);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_KEEP_SUCCESS_CONTEXT", value);
    } else {
        std::env::remove_var("STARFOX_KEEP_SUCCESS_CONTEXT");
    }
}

#[test]
fn ac98_override_can_null_success_continuation_word_before_bad_parser_handoff() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_NULL_AC98_AFTER_SUCCESS");
    std::env::set_var("STARFOX_NULL_AC98_AFTER_SUCCESS", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.write_ram_word_short(0x1AE2, 0x004B);
    gsu.write_ram_word_short(0x888C, 0x4BFC);

    gsu.write_reg_exec(1, 0x887F, 0xF1, 0xAC98);

    assert_eq!(gsu.regs[1], 0x0000);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_NULL_AC98_AFTER_SUCCESS", value);
    } else {
        std::env::remove_var("STARFOX_NULL_AC98_AFTER_SUCCESS");
    }
}

#[test]
fn d31a_success_fragment_clears_record_plus_one_flag_byte() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0008].copy_from_slice(&[
        0xA7, 0x01, // IBT R7,#1
        0x27, // WITH R7
        0x53, // ADD R3 -> R7
        0xB0, // FROM R0
        0x3D, // ALT1
        0x37, // STB [R7], R0
        0x00, // STOP
    ]);

    gsu.pbr = 0x00;
    gsu.regs[0] = 0x0000;
    gsu.regs[3] = 0x1AD6;
    gsu.regs[7] = 0x000C;
    gsu.regs[15] = 0x8000;
    gsu.game_ram[0x1AD7] = 0xA2;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    for _ in 0..16 {
        if !gsu.running() {
            break;
        }
        gsu.run_steps(&rom, 1);
    }

    assert_eq!(gsu.regs[7], 0x1AD7);
    assert_eq!(gsu.game_ram[0x1AD7], 0x00);
    assert!(!gsu.running());
}

#[test]
fn d4d8_success_tail_sets_low_bit_on_zero_flag_byte() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0015].copy_from_slice(&[
        0xA5, 0x01, // IBT R5,#1
        0x25, // WITH R5
        0x5B, // ADD R11 -> R5
        0x15, // TO R5
        0x3D, // ALT1
        0x45, // LDB [R5] -> R5
        0xB5, // FROM R5
        0x3E, // ALT2
        0xC1, // OR #1 -> R0
        0xA4, 0x01, // IBT R4,#1
        0x24, // WITH R4
        0x5B, // ADD R11 -> R4
        0xB0, // FROM R0
        0x3D, // ALT1
        0x34, // STB [R4], R0
        0xB5, // FROM R5
        0x3E, // ALT2
        0x72, // AND #2 -> R0
        0x00, // STOP
    ]);

    gsu.pbr = 0x00;
    gsu.regs[1] = 0xE2DA;
    gsu.regs[2] = 0x007D;
    gsu.regs[4] = 0xFC4B;
    gsu.regs[5] = 0x0001;
    gsu.regs[6] = 0xFC44;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AD6;
    gsu.regs[12] = 0x0129;
    gsu.regs[14] = 0x8A7B;
    gsu.regs[15] = 0x8000;
    gsu.game_ram[0x1AD7] = 0x00;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    for _ in 0..32 {
        if !gsu.running() {
            break;
        }
        gsu.run_steps(&rom, 1);
    }

    assert_eq!(gsu.game_ram[0x1AD7], 0x01);
    assert_eq!(gsu.regs[0], 0x0000);
    assert!(!gsu.running());
}

#[test]
fn d4d8_success_tail_preserves_existing_flag_bits_before_setting_low_bit() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0015].copy_from_slice(&[
        0xA5, 0x01, // IBT R5,#1
        0x25, // WITH R5
        0x5B, // ADD R11 -> R5
        0x15, // TO R5
        0x3D, // ALT1
        0x45, // LDB [R5] -> R5
        0xB5, // FROM R5
        0x3E, // ALT2
        0xC1, // OR #1 -> R0
        0xA4, 0x01, // IBT R4,#1
        0x24, // WITH R4
        0x5B, // ADD R11 -> R4
        0xB0, // FROM R0
        0x3D, // ALT1
        0x34, // STB [R4], R0
        0xB5, // FROM R5
        0x3E, // ALT2
        0x72, // AND #2 -> R0
        0x00, // STOP
    ]);

    gsu.pbr = 0x00;
    gsu.regs[1] = 0xE2DA;
    gsu.regs[2] = 0x007D;
    gsu.regs[4] = 0xFC4B;
    gsu.regs[5] = 0x0001;
    gsu.regs[6] = 0xFC44;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AD6;
    gsu.regs[12] = 0x0129;
    gsu.regs[14] = 0x8A7B;
    gsu.regs[15] = 0x8000;
    gsu.game_ram[0x1AD7] = 0xA2;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    for _ in 0..32 {
        if !gsu.running() {
            break;
        }
        gsu.run_steps(&rom, 1);
    }

    assert_eq!(gsu.game_ram[0x1AD7], 0xA3);
    assert_eq!(gsu.regs[0], 0x0002);
    assert!(!gsu.running());
}

#[test]
fn stop_updates_cbr_and_clears_r_bit() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0xD0; // INC R0
    rom[0x0001] = 0x00; // STOP
    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 4);
    assert!(!gsu.running());
    // CBR should be updated to R15 & 0xFFF0 at STOP
    assert_eq!(gsu.cbr, gsu.regs[15] & 0xFFF0);
    // R_BIT should be cleared
    assert_eq!(gsu.sfr & super::SFR_R_BIT, 0);
    // GO bit should be cleared
    assert_eq!(gsu.sfr & SFR_GO_BIT, 0);
}

#[test]
fn stop_clears_prefix_flags_and_plot_option_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0x00; // STOP
    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT | SFR_ALT1_BIT | super::SFR_ALT2_BIT | super::SFR_B_BIT;
    gsu.src_reg = 6;
    gsu.dst_reg = 7;
    gsu.with_reg = 8;
    gsu.por = 0x1F;

    gsu.run_steps(&rom, 1);

    assert_eq!(
        gsu.sfr & (SFR_ALT1_BIT | super::SFR_ALT2_BIT | super::SFR_B_BIT),
        0
    );
    assert_eq!(gsu.src_reg, 0);
    assert_eq!(gsu.dst_reg, 0);
    assert_eq!(gsu.with_reg, 0);
    assert_eq!(gsu.por, 0);
}

#[test]
fn sfr_r_bit_set_while_running() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0xD0; // INC R0
    rom[0x0001] = 0xD0; // INC R0
    rom[0x0002] = 0x00; // STOP
    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    // Run only 1 step - should still be running
    gsu.run_steps(&rom, 1);
    assert!(gsu.running());
    assert_ne!(gsu.sfr & super::SFR_R_BIT, 0);
}

#[test]
fn run_steps_stops_immediately_after_ram_save_hit() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000] = 0xD0; // INC R0
    rom[0x0001] = 0xD0; // INC R0
    rom[0x0002] = 0x00; // STOP
    gsu.pbr = 0x00;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;
    gsu.save_state_ram_addr_hit = Some((0x00, 0x8000, 0x0010));

    gsu.run_steps(&rom, 8);

    assert_eq!(gsu.regs[0], 1);
    assert!(gsu.running());
    assert_eq!(gsu.save_state_ram_addr_hit, Some((0x00, 0x8000, 0x0010)));
}

#[test]
fn ram_word_after_byte_write_uses_pending_xor_paired_byte() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.game_ram[0x021E] = 0x52;
    gsu.game_ram[0x021F] = 0x88;

    assert_eq!(gsu.read_ram_word(0x021E), 0x8852);
    assert_eq!(gsu.ram_word_after_byte_write(0x021E, 0x021E, 0x7F), 0x887F);
    assert_eq!(gsu.ram_word_after_byte_write(0x021E, 0x021F, 0x29), 0x2952);
}

#[test]
fn plot_always_increments_r1() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x00; // 2bpp, 128h
    gsu.scbr = 0x00;
    gsu.colr = 0x01;
    gsu.por = 0x08;
    gsu.regs[1] = 10;
    gsu.regs[2] = 0;

    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    assert_eq!(gsu.regs[1], 11);
}

#[test]
fn apply_color_matches_shift_and_merge_bits() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.colr = 0xA0;

    gsu.por = 0x04;
    assert_eq!(gsu.apply_color(0xBC), 0xAB);

    gsu.por = 0x08;
    assert_eq!(gsu.apply_color(0xBC), 0xAC);

    gsu.por = 0x0C;
    assert_eq!(gsu.apply_color(0xBC), 0xAB);
}

#[test]
fn plot_dither_mode_selects_color_nibble_by_position() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x00; // 2bpp, 128h
    gsu.scbr = 0x04; // offset screen to avoid overlap
    gsu.por = 0x0A; // dither (bit 1) + merge low nibble (bit 3)
    gsu.colr = 0x31; // high=3, low=1

    // Even position (x+y=0): use low nibble (1)
    gsu.regs[1] = 0;
    gsu.regs[2] = 0;
    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));

    // Odd position (x+y=1): use high nibble (3)
    gsu.regs[1] = 1;
    gsu.regs[2] = 0;
    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
}

#[test]
fn color_opcode_respects_por_shift_and_merge_bits() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[0] = 0x00BC;
    gsu.src_reg = 0;

    gsu.colr = 0xA0;
    gsu.por = 0x04;
    assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
    assert_eq!(gsu.colr, 0xAB);

    gsu.regs[0] = 0x00BC;
    gsu.colr = 0xA0;
    gsu.por = 0x08;
    assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
    assert_eq!(gsu.colr, 0xAC);

    gsu.regs[0] = 0x00BC;
    gsu.colr = 0xA0;
    gsu.por = 0x0C;
    assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
    assert_eq!(gsu.colr, 0xAB);
}

#[test]
fn plot_8bpp_uses_full_byte_for_transparency_when_freezehigh_is_clear() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x03; // 8bpp
    gsu.scbr = 0x00;

    gsu.plot_pixel(0, 0, 0x10);
    gsu.flush_all_pixel_caches();
    assert_ne!(gsu.read_plot_pixel(0, 0), 0);

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x03; // 8bpp
    gsu.scbr = 0x00;
    gsu.por = 0x08; // freezehigh

    gsu.plot_pixel(0, 0, 0x10);
    gsu.flush_all_pixel_caches();
    assert_eq!(gsu.read_plot_pixel(0, 0), 0);
}

#[test]
fn cmode_opcode_updates_plot_option_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[0] = 0x0010;
    gsu.src_reg = 0;
    gsu.sfr |= SFR_ALT1_BIT;

    assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
    assert_eq!(gsu.por, 0x10);
    assert_eq!(gsu.screen_height(), Some(128));
}

#[test]
fn alt3_cmode_opcode_updates_plot_option_register() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.regs[0] = 0x0010;
    gsu.src_reg = 0;
    gsu.sfr |= SFR_ALT1_BIT | super::SFR_ALT2_BIT;

    assert!(gsu.execute_opcode(0x4E, &[], 0x8000));
    assert_eq!(gsu.por, 0x10);
    assert_eq!(gsu.screen_height(), Some(128));
}

#[test]
fn alt3_rpix_reads_pixel() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.dst_reg = 3;
    gsu.regs[1] = 0;
    gsu.regs[2] = 0;
    gsu.colr = 0x5A;
    gsu.sfr |= SFR_ALT1_BIT | super::SFR_ALT2_BIT;
    gsu.plot_pixel(0, 0, 0x0A);

    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    assert_eq!(gsu.regs[3], 0x0002);
    assert_eq!(gsu.colr, 0x5A);
}

#[test]
fn rpix_4bit_preserves_existing_sign_zero_flags() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x01; // 4bpp
    gsu.scbr = 0x00;
    gsu.dst_reg = 2;
    gsu.regs[1] = 0;
    gsu.regs[2] = 0;
    gsu.sfr |= SFR_ALT1_BIT | SFR_S_BIT | SFR_Z_BIT;
    gsu.plot_pixel(0, 0, 0x0A);

    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    assert_eq!(gsu.regs[2], 0x000A);
    assert_eq!(gsu.sfr & SFR_S_BIT, SFR_S_BIT);
    assert_eq!(gsu.sfr & SFR_Z_BIT, SFR_Z_BIT);
}

#[test]
fn rpix_8bit_zero_case_updates_zero_only_and_preserves_sign() {
    let mut gsu = SuperFx::new(0x20_0000);
    gsu.scmr = 0x03; // 8bpp
    gsu.scbr = 0x00;
    gsu.dst_reg = 2;
    gsu.regs[1] = 0;
    gsu.regs[2] = 0;
    gsu.sfr |= SFR_ALT1_BIT | SFR_S_BIT;
    gsu.plot_pixel(0, 0, 0x00);

    assert!(gsu.execute_opcode(0x4C, &[], 0x8000));
    assert_eq!(gsu.regs[2], 0x0000);
    assert_eq!(gsu.sfr & SFR_S_BIT, SFR_S_BIT);
    assert_eq!(gsu.sfr & SFR_Z_BIT, SFR_Z_BIT);
}

#[test]
fn rom_bank_mask_adapts_to_rom_size() {
    // 1MB ROM = 32 banks of 32KB → mask = 31
    let gsu_1m = SuperFx::new(0x10_0000);
    assert_eq!(gsu_1m.rom_bank_mask, 31);

    // 2MB ROM = 64 banks → mask = 63
    let gsu_2m = SuperFx::new(0x20_0000);
    assert_eq!(gsu_2m.rom_bank_mask, 63);

    // 512KB ROM = 16 banks → mask = 15
    let gsu_512k = SuperFx::new(0x8_0000);
    assert_eq!(gsu_512k.rom_bank_mask, 15);
}

#[test]
fn default_instruction_cycle_cost_is_one() {
    let mut gsu = SuperFx::new(0x20_0000);
    let rom = vec![0u8; 0x20_0000];
    gsu.src_reg = 0;
    gsu.regs[0] = 3;
    gsu.regs[1] = 5;

    assert!(gsu.execute_opcode_internal(0x81, &rom, 0x8000, false));
    assert_eq!(gsu.last_opcode_cycles, 1);
}
