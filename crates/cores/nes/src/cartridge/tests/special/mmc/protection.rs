use super::super::super::*;

#[test]
fn mapper_208_switches_prg_protection_and_chr_banks() {
    let mut cart = make_mapper208_cart();

    assert_eq!(cart.read_prg(0x8000), 3);
    cart.write_prg(0x4800, 0x20);
    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg_ram(0x6800, 0x11);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_prg(0x5000, 0x09);
    cart.write_prg(0x5800, 0xAA);
    cart.write_prg(0x5801, 0x55);
    assert_eq!(cart.read_prg_low(0x5800), 0xE3);
    assert_eq!(cart.read_prg_low(0x5801), 0x1C);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x06);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x05);
    assert_eq!(cart.read_chr(0x0000), 0x56);
    assert_eq!(cart.read_chr(0x0400), 0x57);
    assert_eq!(cart.read_chr(0x1000), 0x55);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x4800, 0x20);
    cart.write_prg(0x5000, 0x0A);
    cart.write_prg(0x5800, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg_low(0x5800), 0xE3);
    assert_eq!(cart.read_chr(0x1000), 0x55);
}

#[test]
fn mapper_250_uses_address_lines_for_register_select_and_data() {
    let mut cart = make_mmc3_mixed_chr_cart(250, 8, 16, 0);

    cart.write_prg(0x8006, 0xFF);
    cart.write_prg(0x8403, 0xAA);
    cart.write_prg(0x8007, 0x11);
    cart.write_prg(0x8404, 0x22);
    cart.write_prg(0x8000, 0xFE);
    cart.write_prg(0x8406, 0x55);
    cart.write_prg(0xA001, 0x00);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_prg(0xE000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x56);
    assert_eq!(cart.read_chr(0x0400), 0x57);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8006, 0x00);
    cart.write_prg(0x8401, 0x00);
    cart.write_prg(0xA000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_chr(0x0000), 0x56);
    assert_eq!(cart.read_chr(0x0400), 0x57);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}
