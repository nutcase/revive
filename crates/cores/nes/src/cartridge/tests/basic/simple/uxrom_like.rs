use super::super::super::*;

#[test]
fn mapper_78_switches_prg_chr_and_one_screen_mirroring() {
    let mut cart = make_mapper78_cart(false);
    cart.prg_rom[0] = 0xFF;

    cart.write_prg(0x8000, 0x9A);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x79);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
}

#[test]
fn mapper_77_switches_32k_prg_and_split_chr_ram() {
    let mut cart = make_mapper77_cart();
    cart.prg_rom[0] = 0xFF;

    cart.write_prg(0x8000, 0x21);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xFFFF), 1);
    assert_eq!(cart.read_chr(0x0000), 0x82);

    cart.write_chr(0x0800, 0x44);
    cart.write_chr(0x1FFF, 0x99);
    assert_eq!(cart.read_chr(0x0800), 0x44);
    assert_eq!(cart.read_chr(0x1FFF), 0x99);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_chr(0x0800, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_chr(0x0000), 0x82);
    assert_eq!(cart.read_chr(0x0800), 0x44);
}

#[test]
fn mapper_78_header_variant_uses_horizontal_vertical_mirroring() {
    let mut cart = make_mapper78_cart(true);
    cart.prg_rom[0] = 0xFF;

    cart.write_prg(0x8000, 0x08);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_prg(0x8000, 0x00);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_94_switches_prg_bank_from_upper_bits() {
    let mut cart = make_uxrom_like_cart(94, 8, 1);
    cart.prg_rom[0] = 0xFF;

    cart.write_prg(0x8000, 0x14);

    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x70);
}

#[test]
fn mapper_89_switches_prg_chr_and_one_screen_mirroring() {
    let mut cart = make_uxrom_like_cart(89, 8, 16);
    cart.prg_rom[0] = 0xFF;

    cart.write_prg(0x8000, 0x9D);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x7D);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
}

#[test]
fn mapper_93_switches_prg_and_restores_chr_ram_enable_state() {
    let mut cart = make_uxrom_like_cart(93, 8, 1);
    cart.prg_rom[0] = 0xFF;

    cart.write_chr(0x0010, 0x44);
    assert_eq!(cart.read_chr(0x0010), 0x44);

    cart.write_prg(0x8000, 0x20);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_chr(0x0010), 0xFF);

    let snapshot = cart.snapshot_state();

    cart.write_prg(0xC000, 0x21);
    assert_eq!(cart.read_chr(0x0010), 0x44);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_chr(0x0010), 0xFF);
}

#[test]
fn mapper_70_switches_prg_and_chr_banks() {
    let mut cart = make_uxrom_like_cart(70, 8, 16);
    cart.prg_rom[0] = 0xFF;

    cart.write_prg(0x8000, 0x21);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x71);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_152_switches_prg_chr_and_mirroring() {
    let mut cart = make_uxrom_like_cart(152, 8, 16);
    cart.prg_rom[0] = 0xFF;

    cart.write_prg(0x8000, 0xB2);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x72);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
}

#[test]
fn mapper_146_matches_mapper_79_latch_layout() {
    let mut cart = make_simple_bank_cart(146, 2, 8);

    cart.write_prg(0x5F00, 0x0C);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_chr(0x0000), 0x44);
}

#[test]
fn mapper_148_switches_32k_prg_and_chr_banks_with_bus_conflicts() {
    let mut cart = make_simple_bank_cart(148, 2, 8);
    cart.prg_rom[0] = 0x09;

    cart.write_prg(0x8000, 0x0B);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xFFFF), 1);
    assert_eq!(cart.read_chr(0x0000), 0x41);
}

#[test]
fn mapper_180_switches_upper_prg_bank_only() {
    let mut cart = make_uxrom_like_cart(180, 8, 1);
    cart.prg_rom[1] = 0xFF;

    cart.write_prg(0xC001, 0x03);

    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xBFFF), 0);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_prg(0xFFFF), 3);
}

#[test]
fn mapper_15_handles_all_prg_modes_and_prg_ram() {
    let mut cart = make_mapper15_cart();

    cart.write_prg(0x8000, 0x02);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_prg(0x8001, 0x03);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 7);

    cart.write_prg(0x8002, 0x01);
    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xE000), 1);

    cart.write_prg(0x8003, 0x44);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 4);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg_ram(0x6000, 0xA5);
    assert_eq!(cart.read_prg_ram(0x6000), 0xA5);
}
