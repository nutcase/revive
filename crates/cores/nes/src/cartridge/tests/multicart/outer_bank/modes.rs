use super::super::super::*;

#[test]
fn mapper_226_handles_mode_mirroring_high_bit_and_restore() {
    let mut cart = make_uxrom_like_cart(226, 80, 1);

    cart.write_prg(0x8000, 0x03);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x8001, 0x01);
    assert_eq!(cart.read_prg(0x8000), 66);
    assert_eq!(cart.read_prg(0xC000), 67);

    cart.write_prg(0x8000, 0x63);
    assert_eq!(cart.read_prg(0x8000), 67);
    assert_eq!(cart.read_prg(0xC000), 67);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 67);
    assert_eq!(cart.read_prg(0xC000), 67);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}

#[test]
fn mapper_232_selects_inner_page_inside_64k_block() {
    let mut cart = make_uxrom_like_cart(232, 16, 1);

    cart.write_prg(0x8000, 0x10);
    cart.write_prg(0xC000, 0x01);

    assert_eq!(cart.read_prg(0x8000), 9);
    assert_eq!(cart.read_prg(0xC000), 11);
    assert_eq!(cart.read_chr(0x0000), 0x70);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0xC000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 9);
    assert_eq!(cart.read_prg(0xC000), 11);
}

#[test]
fn mapper_233_switches_prg_chr_and_mirroring_modes() {
    let mut cart = make_uxrom_like_cart(233, 32, 32);

    cart.write_prg(0x8000, 0x85);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_chr(0x0000), 0x75);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x8000, 0xE3);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x73);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x05);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_chr(0x0000), 0x75);
    assert_eq!(cart.mirroring(), Mirroring::ThreeScreenLower);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x73);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
}

#[test]
fn mapper_234_latches_outer_and_inner_banks_from_cpu_reads() {
    let mut cart = make_simple_bank_cart(234, 16, 64);
    cart.prg_rom[0x7F80] = 0xC3;
    cart.prg_rom[2 * 0x8000 + 0x7FE8] = 0x51;
    cart.prg_rom[3 * 0x8000 + 0x7F80] = 0x00;
    cart.prg_rom[3 * 0x8000 + 0x7FE8] = 0x20;

    assert_eq!(cart.read_prg_cpu(0xFF80), 0xC3);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    assert_eq!(cart.read_prg_cpu(0xFFE8), 0x51);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xFFFF), 3);
    assert_eq!(cart.read_chr(0x0000), 0x4D);

    let snapshot = cart.snapshot_state();

    assert_eq!(cart.read_prg_cpu(0xFF80), 0x00);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x4D);

    assert_eq!(cart.read_prg_cpu(0xFFE8), 0x20);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_chr(0x0000), 0x4A);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x4D);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_235_selects_chip_page_and_mirroring_modes() {
    let mut cart = make_mapper235_cart();

    cart.write_prg(0x8203, 0);
    assert_eq!(cart.read_prg(0x8000), 70);
    assert_eq!(cart.read_prg(0xC000), 71);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x9E02, 0);
    assert_eq!(cart.read_prg(0x8000), 69);
    assert_eq!(cart.read_prg(0xC000), 69);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenLower);

    cart.write_chr(0x0123, 0x5A);
    assert_eq!(cart.read_chr(0x0123), 0x5A);

    let snapshot = cart.snapshot_state();

    cart.write_prg(0x8100, 0);
    assert_eq!(cart.read_prg(0x8000), 0xFF);
    assert_eq!(cart.read_prg(0xC000), 0xFF);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 69);
    assert_eq!(cart.read_prg(0xC000), 69);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenLower);
}
