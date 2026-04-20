use super::super::super::super::*;

#[test]
fn mapper_114_scrambles_registers_and_supports_override_modes() {
    let mut cart = make_mapper114_cart(114);

    cart.write_prg(0xA000, 0x04);
    cart.write_prg(0xC000, 0x03);
    cart.write_prg(0xA000, 0x05);
    cart.write_prg(0xC000, 0x04);
    cart.write_prg(0xA000, 0x06);
    cart.write_prg(0xC000, 0x05);
    cart.write_prg_ram(0x6001, 0x01);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x1000), 0x85);

    cart.write_prg_ram(0x6000, 0x83);
    assert_eq!(cart.read_prg(0x8000), 6);
    assert_eq!(cart.read_prg(0xC000), 6);

    cart.write_prg_ram(0x6000, 0xC2);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x6000, 0x00);
    cart.write_prg_ram(0x6001, 0x00);
    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_chr(0x1000), 0x85);
}

#[test]
fn mapper_123_uses_scrambled_bank_select_and_5800_override() {
    let mut cart = make_mapper123_cart();

    cart.write_prg(0x8000, 0x04);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x05);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x1000), 0x65);

    cart.write_prg(0x5800, 0x40);
    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xC000), 0);

    cart.write_prg(0x5800, 0x42);
    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xC000), 2);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x5800, 0x00);
    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xC000), 2);
}

#[test]
fn mapper_115_supports_outer_chr_and_nrom_override_modes() {
    let mut cart = make_mapper115_cart(115);

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x05);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);

    cart.write_prg_ram(0x6001, 0x01);
    assert_eq!(cart.read_chr(0x1000), 0x85);

    cart.write_prg_ram(0x6000, 0x83);
    assert_eq!(cart.read_prg(0x8000), 6);
    assert_eq!(cart.read_prg(0xC000), 6);

    cart.write_prg_ram(0x6000, 0xA2);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_prg_ram(0x6002), 0);

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x6000, 0x00);
    cart.write_prg_ram(0x6001, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_chr(0x1000), 0x85);
}

#[test]
fn mapper_248_aliases_115_behavior() {
    let mut cart = make_mapper115_cart(248);

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x01);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x02);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg_ram(0x6001, 0x01);
    cart.write_prg_ram(0x6000, 0x81);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 2);
    assert_eq!(cart.read_chr(0x1000), 0x84);
    assert_eq!(cart.read_prg_ram(0x6002), 0);
}
