use super::super::super::super::*;

#[test]
fn mapper_189_uses_low_address_prg_bank_writes_with_mmc3_chr() {
    let mut cart = make_mapper189_cart();

    cart.write_prg(0x4100, 0xA4);
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x07);

    assert_eq!(cart.read_prg(0x8000), 14);
    assert_eq!(cart.read_prg(0xE000), 14);
    assert_eq!(cart.read_chr(0x0000), 0x64);
    assert_eq!(cart.read_chr(0x0400), 0x65);
    assert_eq!(cart.read_chr(0x1000), 0x67);

    let snapshot = cart.snapshot_state();

    cart.write_prg_ram(0x6000, 0x93);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x01);
    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_chr(0x1000), 0x61);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 14);
    assert_eq!(cart.read_chr(0x0000), 0x64);
    assert_eq!(cart.read_chr(0x0400), 0x65);
    assert_eq!(cart.read_chr(0x1000), 0x67);
}

#[test]
fn mapper_245_uses_chr_register_high_bit_for_prg_bank_group() {
    let mut cart = make_mapper245_cart();

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x02);
    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x05);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x06);

    assert_eq!(cart.read_prg(0x8000), 37);
    assert_eq!(cart.read_prg(0xA000), 38);
    assert_eq!(cart.read_prg(0xC000), 62);
    assert_eq!(cart.read_prg(0xE000), 63);

    cart.write_chr(0x0123, 0x5A);
    assert_eq!(cart.read_chr(0x0123), 0x5A);
    cart.write_prg_ram(0x6000, 0xA5);
    assert_eq!(cart.read_prg_ram(0x6000), 0xA5);
}
