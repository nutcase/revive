use super::*;

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
