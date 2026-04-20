use super::super::super::super::*;

#[test]
fn mapper_205_selects_outer_prg_chr_blocks_and_restores_state() {
    let mut cart = make_mapper205_cart();

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x13);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x14);
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x86);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x88);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x8A);
    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0x8001, 0x8B);
    cart.write_prg(0x8000, 0x04);
    cart.write_prg(0x8001, 0x8C);
    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0x8001, 0x8D);

    assert_eq!(cart.read_prg(0x8000), 19);
    assert_eq!(cart.read_prg(0xA000), 20);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x0000), 0x46);
    assert_eq!(cart.read_chr(0x0400), 0x47);
    assert_eq!(cart.read_chr(0x0800), 0x48);
    assert_eq!(cart.read_chr(0x1000), 0x4A);
    assert_eq!(cart.read_chr(0x1C00), 0x4D);

    cart.write_prg_ram(0x6000, 0x02);
    assert_eq!(cart.read_prg(0x8000), 35);
    assert_eq!(cart.read_prg(0xA000), 36);
    assert_eq!(cart.read_prg(0xC000), 46);
    assert_eq!(cart.read_prg(0xE000), 47);
    assert_eq!(cart.read_chr(0x0000), 0x86);
    assert_eq!(cart.read_chr(0x1000), 0x8A);
    assert_eq!(cart.read_chr(0x1C00), 0x8D);

    cart.write_prg_ram(0x6000, 0x03);
    assert_eq!(cart.read_prg(0x8000), 51);
    assert_eq!(cart.read_prg(0xA000), 52);
    assert_eq!(cart.read_prg(0xC000), 62);
    assert_eq!(cart.read_prg(0xE000), 63);
    assert_eq!(cart.read_chr(0x0000), 0xC6);
    assert_eq!(cart.read_chr(0x1000), 0xCA);
    assert_eq!(cart.read_chr(0x1C00), 0xCD);

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x6000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 51);
    assert_eq!(cart.read_chr(0x1000), 0xCA);
}

#[test]
fn mapper_12_uses_split_outer_chr_bits_with_mmc3_prg_layout() {
    let mut cart = make_mapper12_cart();

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x06);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x08);
    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0x8001, 0x09);
    cart.write_prg(0x8000, 0x04);
    cart.write_prg(0x8001, 0x0A);
    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0x8001, 0x0B);
    cart.write_prg(0xA001, 0x11);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x0000), 0x84);
    assert_eq!(cart.read_chr(0x0400), 0x85);
    assert_eq!(cart.read_chr(0x0800), 0x86);
    assert_eq!(cart.read_chr(0x0C00), 0x87);
    assert_eq!(cart.read_chr(0x1000), 0x88);
    assert_eq!(cart.read_chr(0x1C00), 0x8B);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0xA001, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_chr(0x0000), 0x84);
    assert_eq!(cart.read_chr(0x1000), 0x88);
}

#[test]
fn mapper_44_switches_outer_prg_chr_windows() {
    let mut cart = make_mapper44_cart();

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x06);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x08);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x0A);
    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0x8001, 0x0B);
    cart.write_prg(0x8000, 0x04);
    cart.write_prg(0x8001, 0x0C);
    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0x8001, 0x0D);
    cart.write_prg(0xA000, 0x01);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 14);
    assert_eq!(cart.read_prg(0xE000), 15);
    assert_eq!(cart.read_chr(0x0000), 0x06);
    assert_eq!(cart.read_chr(0x1000), 0x0A);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0xA001, 0x02);
    assert_eq!(cart.read_prg(0x8000), 35);
    assert_eq!(cart.read_prg(0xA000), 36);
    assert_eq!(cart.read_prg(0xC000), 46);
    assert_eq!(cart.read_prg(0xE000), 47);
    assert_eq!(cart.read_chr(0x0000), 0x46);
    assert_eq!(cart.read_chr(0x1000), 0x4A);

    cart.write_prg(0xA001, 0x07);
    assert_eq!(cart.read_prg(0x8000), 115);
    assert_eq!(cart.read_prg(0xA000), 116);
    assert_eq!(cart.read_prg(0xC000), 126);
    assert_eq!(cart.read_prg(0xE000), 127);
    assert_eq!(cart.read_chr(0x0000), 0xE6);
    assert_eq!(cart.read_chr(0x1000), 0xEA);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0xA001, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 115);
    assert_eq!(cart.read_chr(0x1000), 0xEA);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}
