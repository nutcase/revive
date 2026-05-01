use super::*;

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
