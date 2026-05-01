use super::*;

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
