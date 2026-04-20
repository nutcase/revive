use super::super::*;

#[test]
fn mapper_21_switches_prg_chr_and_supports_dual_vrc4_decode() {
    let mut cart = make_mapper21_cart();

    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0xA000, 0x04);
    cart.write_prg(0x9000, 0x03);
    cart.write_prg(0xB000, 0x05);
    cart.write_prg(0xB002, 0x01);
    cart.write_prg(0xB040, 0x02);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 62);
    assert_eq!(cart.read_prg(0xE000), 63);
    assert_eq!(cart.read_chr(0x0000), 0xA5);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);

    cart.write_prg(0x9004, 0x02);
    assert_eq!(cart.read_prg(0x8000), 62);
    assert_eq!(cart.read_prg(0xC000), 3);
}

#[test]
fn mapper_22_switches_prg_chr_and_vrc2_mirroring() {
    let mut cart = make_mapper22_cart();

    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0xA000, 0x04);
    cart.write_prg_ram(0x6000, 0x01);
    cart.write_prg(0x9000, 0x01);
    cart.write_prg(0xB000, 0x05);
    cart.write_prg(0xB002, 0x01);
    cart.write_prg(0xB001, 0x07);
    cart.write_prg(0xB003, 0x00);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_prg(0xE000), 31);
    assert_eq!(cart.read_chr(0x0000), 0x8A);
    assert_eq!(cart.read_chr(0x0400), 0x83);
    assert_eq!(cart.read_prg_ram(0x6000), 0x61);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x9002, 0x00);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}

#[test]
fn mapper_23_switches_prg_chr_and_wram() {
    let mut cart = make_mapper23_cart();

    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0xA000, 0x06);

    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xA000), 6);
    assert_eq!(cart.read_prg(0xC000), 62);
    assert_eq!(cart.read_prg(0xE000), 63);

    cart.write_prg(0x9008, 0x03);
    cart.write_prg_ram(0x6001, 0x5A);
    assert_eq!(cart.read_prg_ram(0x6001), 0x5A);
    assert_eq!(cart.read_prg(0x8000), 62);
    assert_eq!(cart.read_prg(0xC000), 5);

    cart.write_prg(0xB000, 0x0A);
    cart.write_prg(0xB004, 0x01);
    cart.write_prg(0xB008, 0x03);
    cart.write_prg(0xB00C, 0x02);

    assert_eq!(cart.read_chr(0x0000), 0x9A);
    assert_eq!(cart.read_chr(0x0400), 0xA3);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x9008, 0x00);
    cart.write_prg_ram(0x6001, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_prg_ram(0x6001), 0x5A);
    assert_eq!(cart.read_chr(0x0400), 0xA3);
}

#[test]
fn mapper_24_26_switch_prg_chr_and_wram() {
    fn reg_addr(mapper: u8, reg: u16) -> u16 {
        if mapper == 26 {
            (reg & !0x0003) | (((reg & 0x0001) << 1) | ((reg & 0x0002) >> 1))
        } else {
            reg
        }
    }

    for mapper in [24_u8, 26] {
        let mut cart = make_mapper24_26_cart(mapper);

        cart.write_prg(reg_addr(mapper, 0x8000), 0x03);
        cart.write_prg(reg_addr(mapper, 0xC000), 0x05);
        cart.write_prg_ram(0x6002, 0x11);
        assert_eq!(cart.read_prg_ram(0x6002), 0x00);

        cart.write_prg(reg_addr(mapper, 0xB003), 0x84);
        cart.write_prg(reg_addr(mapper, 0xD000), 0x05);
        cart.write_prg(reg_addr(mapper, 0xD001), 0x06);

        assert_eq!(cart.read_prg(0x8000), 0x06, "mapper {mapper}");
        assert_eq!(cart.read_prg(0xA000), 0x07, "mapper {mapper}");
        assert_eq!(cart.read_prg(0xC000), 0x05, "mapper {mapper}");
        assert_eq!(cart.read_prg(0xE000), 0x3F, "mapper {mapper}");
        assert_eq!(cart.read_chr(0x0000), 0x85, "mapper {mapper}");
        assert_eq!(cart.read_chr(0x0400), 0x86, "mapper {mapper}");
        assert_eq!(cart.mirroring(), Mirroring::Horizontal, "mapper {mapper}");

        cart.write_prg_ram(0x6002, 0x5A);
        assert_eq!(cart.read_prg_ram(0x6002), 0x5A, "mapper {mapper}");
    }
}

#[test]
fn mapper_25_switches_prg_chr_and_wram_with_vrc4d_decode() {
    let mut cart = make_mapper25_cart(false);

    cart.write_prg(0x8008, 0x05);
    cart.write_prg(0xA004, 0x06);

    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xA000), 6);
    assert_eq!(cart.read_prg(0xC000), 62);
    assert_eq!(cart.read_prg(0xE000), 63);

    cart.write_prg(0x9004, 0x03);
    cart.write_prg_ram(0x6001, 0x5A);
    assert_eq!(cart.read_prg_ram(0x6001), 0x5A);
    assert_eq!(cart.read_prg(0x8000), 62);
    assert_eq!(cart.read_prg(0xC000), 5);

    cart.write_prg(0xB000, 0x0A);
    cart.write_prg(0xB008, 0x01);
    cart.write_prg(0xB004, 0x03);
    cart.write_prg(0xB00C, 0x02);

    assert_eq!(cart.read_chr(0x0000), 0x9A);
    assert_eq!(cart.read_chr(0x0400), 0xA3);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x9004, 0x00);
    cart.write_prg_ram(0x6001, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_prg_ram(0x6001), 0x5A);
    assert_eq!(cart.read_chr(0x0400), 0xA3);
}

#[test]
fn mapper_151_aliases_vrc1_layout() {
    let mut cart = make_vrc1_cart(151);

    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0xA000, 0x04);
    cart.write_prg(0xC000, 0x05);
    cart.write_prg(0x9000, 0x01);
    cart.write_prg(0xE000, 0x06);
    cart.write_prg(0xF000, 0x07);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_prg(0xE000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x66);
    assert_eq!(cart.read_chr(0x1000), 0x67);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}
