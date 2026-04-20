use super::super::super::super::*;

#[test]
fn mapper_74_uses_chr_ram_for_banks_8_and_9() {
    let mut cart = make_mmc3_mixed_chr_cart(74, 8, 16, 0x0800);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x08);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x02);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x04);

    assert_eq!(cart.read_chr(0x0000), 0x00);
    assert_eq!(cart.read_chr(0x0400), 0x00);
    cart.write_chr(0x0000, 0xAA);
    cart.write_chr(0x0400, 0xBB);
    assert_eq!(cart.read_chr(0x0000), 0xAA);
    assert_eq!(cart.read_chr(0x0400), 0xBB);
    assert_eq!(cart.read_chr(0x1000), 0x54);

    cart.write_chr(0x1000, 0x11);
    assert_eq!(cart.read_chr(0x1000), 0x54);
}

#[test]
fn mapper_119_switches_between_chr_rom_and_chr_ram() {
    let mut cart = make_mmc3_mixed_chr_cart(119, 8, 32, 0x2000);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x40);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x05);

    assert_eq!(cart.read_chr(0x0000), 0x00);
    assert_eq!(cart.read_chr(0x0400), 0x00);
    cart.write_chr(0x0000, 0xC1);
    cart.write_chr(0x0400, 0xC2);
    assert_eq!(cart.read_chr(0x0000), 0xC1);
    assert_eq!(cart.read_chr(0x0400), 0xC2);

    assert_eq!(cart.read_chr(0x0800), 0x52);
    assert_eq!(cart.read_chr(0x0C00), 0x53);
    assert_eq!(cart.read_chr(0x1000), 0x55);
    cart.write_chr(0x1000, 0x99);
    assert_eq!(cart.read_chr(0x1000), 0x55);
}

#[test]
fn mapper_192_uses_chr_ram_for_banks_8_through_11() {
    let mut cart = make_mmc3_mixed_chr_cart(192, 8, 16, 0x1000);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x08);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x0A);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x04);

    assert_eq!(cart.read_chr(0x0000), 0x00);
    assert_eq!(cart.read_chr(0x0400), 0x00);
    assert_eq!(cart.read_chr(0x0800), 0x00);
    assert_eq!(cart.read_chr(0x0C00), 0x00);
    cart.write_chr(0x0000, 0xA1);
    cart.write_chr(0x0400, 0xA2);
    cart.write_chr(0x0800, 0xA3);
    cart.write_chr(0x0C00, 0xA4);
    assert_eq!(cart.read_chr(0x0000), 0xA1);
    assert_eq!(cart.read_chr(0x0400), 0xA2);
    assert_eq!(cart.read_chr(0x0800), 0xA3);
    assert_eq!(cart.read_chr(0x0C00), 0xA4);
    assert_eq!(cart.read_chr(0x1000), 0x54);

    cart.write_chr(0x1000, 0x11);
    assert_eq!(cart.read_chr(0x1000), 0x54);
}

#[test]
fn mapper_194_uses_chr_ram_for_banks_0_and_1() {
    let mut cart = make_mmc3_mixed_chr_cart(194, 8, 16, 0x0800);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x00);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x02);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x04);

    assert_eq!(cart.read_chr(0x0000), 0x00);
    assert_eq!(cart.read_chr(0x0400), 0x00);
    cart.write_chr(0x0000, 0xB1);
    cart.write_chr(0x0400, 0xB2);
    assert_eq!(cart.read_chr(0x0000), 0xB1);
    assert_eq!(cart.read_chr(0x0400), 0xB2);

    assert_eq!(cart.read_chr(0x0800), 0x52);
    assert_eq!(cart.read_chr(0x1000), 0x54);
    cart.write_chr(0x0800, 0xCC);
    assert_eq!(cart.read_chr(0x0800), 0x52);
}
