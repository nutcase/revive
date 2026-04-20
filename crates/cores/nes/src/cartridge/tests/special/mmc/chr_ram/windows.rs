use super::super::super::super::*;

#[test]
fn mapper_191_switches_fixed_prg_banks_and_chr_mode() {
    let mut cart = make_mmc3_mixed_chr_cart(191, 32, 256, 0x0800);

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x05);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 24);
    assert_eq!(cart.read_prg(0xE000), 25);
    assert_eq!(cart.read_chr(0x1000), 0xD5);

    cart.write_prg(0x90AA, 0x03);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x80);

    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x1000), 0x00);
    cart.write_chr(0x1000, 0x5E);
    assert_eq!(cart.read_chr(0x1000), 0x5E);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x90AA, 0x00);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x05);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x1000), 0x5E);
}

#[test]
fn mapper_195_switches_chr_ram_windows_via_ppu_writes() {
    let mut cart = make_mmc3_mixed_chr_cart(195, 8, 256, 0x2000);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x28);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x2A);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x10);

    assert_eq!(cart.read_chr(0x0000), 0x00);
    assert_eq!(cart.read_chr(0x0400), 0x00);
    cart.write_chr(0x0000, 0x61);
    cart.write_chr(0x0400, 0x62);
    assert_eq!(cart.read_chr(0x0000), 0x61);
    assert_eq!(cart.read_chr(0x0400), 0x62);
    assert_eq!(cart.read_chr(0x1000), 0x60);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x0A);
    assert_eq!(cart.read_chr(0x0000), 0x5A);
    cart.write_chr(0x0000, 0x00);
    cart.write_chr(0x0000, 0x71);
    cart.write_chr(0x0400, 0x72);
    assert_eq!(cart.read_chr(0x0000), 0x71);
    assert_eq!(cart.read_chr(0x0400), 0x72);
    assert_eq!(cart.read_chr(0x0800), 0x7A);

    let snapshot = cart.snapshot_state();

    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0xCA);
    cart.write_chr(0x1000, 0x00);
    assert_eq!(cart.read_chr(0x0000), 0x5A);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_chr(0x0000), 0x71);
    assert_eq!(cart.read_chr(0x0400), 0x72);
}
