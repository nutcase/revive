use super::super::*;

#[test]
fn mapper_77_routes_two_nametables_to_chr_ram_and_two_to_internal_vram() {
    let mut cart = make_mapper77_cart();
    let mut ppu = crate::ppu::Ppu::new();

    ppu.v = 0x2000;
    ppu.write_register(0x2007, 0x55, Some(&mut cart));
    ppu.v = 0x2400;
    ppu.write_register(0x2007, 0x66, Some(&mut cart));
    ppu.v = 0x2800;
    ppu.write_register(0x2007, 0x77, Some(&mut cart));
    ppu.v = 0x2C00;
    ppu.write_register(0x2007, 0x88, Some(&mut cart));

    assert_eq!(cart.read_nametable_byte(0, 0, &ppu.nametable), 0x55);
    assert_eq!(cart.read_nametable_byte(1, 0, &ppu.nametable), 0x66);
    assert_eq!(cart.read_nametable_byte(2, 0, &ppu.nametable), 0x77);
    assert_eq!(cart.read_nametable_byte(3, 0, &ppu.nametable), 0x88);
    assert_eq!(ppu.nametable[0][0], 0x77);
    assert_eq!(ppu.nametable[1][0], 0x88);
}

#[test]
fn mapper_99_uses_cartridge_four_screen_nametables() {
    let mut cart = make_mapper99_cart();
    let mut ppu = crate::ppu::Ppu::new();

    ppu.v = 0x2000;
    ppu.write_register(0x2007, 0x11, Some(&mut cart));
    ppu.v = 0x2400;
    ppu.write_register(0x2007, 0x22, Some(&mut cart));
    ppu.v = 0x2800;
    ppu.write_register(0x2007, 0x33, Some(&mut cart));
    ppu.v = 0x2C00;
    ppu.write_register(0x2007, 0x44, Some(&mut cart));

    assert_eq!(cart.read_nametable_byte(0, 0, &ppu.nametable), 0x11);
    assert_eq!(cart.read_nametable_byte(1, 0, &ppu.nametable), 0x22);
    assert_eq!(cart.read_nametable_byte(2, 0, &ppu.nametable), 0x33);
    assert_eq!(cart.read_nametable_byte(3, 0, &ppu.nametable), 0x44);
    assert_eq!(ppu.nametable[0][0], 0);
    assert_eq!(ppu.nametable[1][0], 0);
}

#[test]
fn mapper_137_custom_mirroring_and_state_restore() {
    let mut cart = make_mapper137_cart();

    cart.write_prg(0x4100, 7);
    cart.write_prg(0x4101, 0x00);
    assert_eq!(cart.resolve_nametable(0), Some(0));
    assert_eq!(cart.resolve_nametable(1), Some(1));
    assert_eq!(cart.resolve_nametable(2), Some(1));
    assert_eq!(cart.resolve_nametable(3), Some(1));

    let snapshot = cart.snapshot_state();

    cart.write_prg(0x4100, 7);
    cart.write_prg(0x4101, 0x06);
    assert_eq!(cart.resolve_nametable(0), None);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);

    cart.restore_state(&snapshot);
    assert_eq!(cart.resolve_nametable(0), Some(0));
    assert_eq!(cart.resolve_nametable(3), Some(1));
    assert_eq!(cart.read_prg_low(0x4101), 0x00);
}

#[test]
fn mapper_185_disables_chr_for_initial_probe_reads_after_reset() {
    let mut cart = make_mapper185_cart();

    cart.write_prg(0x8000, 0x02);
    assert_eq!(cart.read_chr(0x0000), 0);

    let snapshot = cart.snapshot_state();
    assert_eq!(cart.read_chr(0x0000), 0);
    assert_eq!(cart.read_chr(0x0000), 0x62);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_chr(0x0000), 0);
    assert_eq!(cart.read_chr(0x0000), 0x62);

    cart.on_reset();
    assert_eq!(cart.read_chr(0x0000), 0);
    assert_eq!(cart.read_chr(0x0000), 0);
    assert_eq!(cart.read_chr(0x0000), 0x62);
}

#[test]
fn mapper_32_switches_prg_chr_and_prg_mode() {
    let mut cart = make_mapper32_cart();

    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0xA000, 0x04);
    for index in 0..8 {
        cart.write_prg(0xB000 + index as u16, 0x08 + index as u8);
    }

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_prg(0xE000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x38);
    assert_eq!(cart.read_chr(0x1C00), 0x3F);

    cart.write_prg(0x9000, 0x03);
    assert_eq!(cart.read_prg(0x8000), 6);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x9000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 6);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x38);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_103_switches_bank_ram_overlay_and_mirroring() {
    let mut cart = make_mapper103_cart();

    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0xF000, 0x10);
    assert_eq!(cart.read_prg_ram(0x6000), 5);
    assert_eq!(cart.read_prg(0x8000), 0xA1);
    assert_eq!(cart.read_prg(0xB800), 0xB2);
    assert_eq!(cart.read_prg(0xD800), 0xC3);

    cart.write_prg(0xE000, 0x08);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0xF000, 0x00);
    cart.write_prg_ram(0x6000, 0x5A);
    cart.write_prg(0xB800, 0xA5);
    assert_eq!(cart.read_prg_ram(0x6000), 0xA5);
    assert_eq!(cart.read_prg(0xB800), 0xA5);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0xF000, 0x10);
    cart.write_prg_ram(0x6000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg_ram(0x6000), 0xA5);
    assert_eq!(cart.read_prg(0xB800), 0xA5);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_153_uses_outer_prg_bank_chr_ram_and_wram_enable() {
    let mut cart = make_mapper153_cart();

    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8008, 0x03);
    cart.write_prg(0x8009, 0x03);
    cart.write_prg(0x800D, 0x40);

    assert_eq!(cart.read_prg(0x8000), 19);
    assert_eq!(cart.read_prg(0xC000), 31);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);

    cart.write_prg_ram(0x6000, 0x5A);
    assert_eq!(cart.read_prg_ram(0x6000), 0x5A);

    cart.write_chr(0x1234, 0x77);
    assert_eq!(cart.read_chr(0x1234), 0x77);

    cart.write_prg(0x800B, 0x01);
    cart.write_prg(0x800C, 0x00);
    cart.write_prg(0x800A, 0x01);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x800D, 0x00);
    assert_eq!(cart.read_prg_ram(0x6000), 0);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg_ram(0x6000), 0x5A);
    assert!(cart.irq_pending());
}
