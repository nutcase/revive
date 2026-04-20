use super::super::super::super::*;

#[test]
fn mapper_37_selects_prg_and_chr_windows_from_prg_ram_latch() {
    let mut cart = make_mmc3_mixed_chr_cart(37, 32, 256, 0);

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x05);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x05);

    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_prg(0xE000), 7);
    assert_eq!(cart.read_chr(0x1000), 0x55);

    cart.write_prg_ram(0x6000, 0x03);
    assert_eq!(cart.read_prg(0x8000), 13);
    assert_eq!(cart.read_prg(0xA000), 12);
    assert_eq!(cart.read_prg(0xC000), 14);
    assert_eq!(cart.read_prg(0xE000), 15);
    assert_eq!(cart.read_chr(0x1000), 0x55);

    cart.write_prg_ram(0x6000, 0x04);
    assert_eq!(cart.read_prg(0x8000), 21);
    assert_eq!(cart.read_prg(0xA000), 20);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x1000), 0xD5);

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x6000, 0x00);
    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 21);
    assert_eq!(cart.read_chr(0x1000), 0xD5);
}

#[test]
fn mapper_47_switches_128k_blocks_only_when_prg_ram_is_writable() {
    let mut cart = make_mmc3_mixed_chr_cart(47, 32, 256, 0);

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x05);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x05);

    cart.write_prg(0xA001, 0x00);
    cart.write_prg_ram(0x6000, 0x01);
    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_chr(0x1000), 0x55);

    cart.write_prg(0xA001, 0x80);
    cart.write_prg_ram(0x6000, 0x01);
    assert_eq!(cart.read_prg(0x8000), 21);
    assert_eq!(cart.read_prg(0xA000), 20);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x1000), 0xD5);

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x6000, 0x00);
    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 21);
    assert_eq!(cart.read_chr(0x1000), 0xD5);
}
