use super::super::*;

#[test]
fn mapper_5_switches_prg_chr_wram_and_multiplier() {
    let mut cart = make_mmc5_cart();

    cart.write_prg(0x5100, 0x03);
    cart.write_prg(0x5114, 0x81);
    cart.write_prg(0x5115, 0x82);
    cart.write_prg(0x5116, 0x83);
    cart.write_prg(0x5117, 0x9F);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xA000), 2);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_prg(0xE000), 31);

    cart.write_prg(0x5102, 0x02);
    cart.write_prg(0x5103, 0x01);
    cart.write_prg(0x5113, 0x04);
    cart.write_prg_ram(0x6123, 0x5A);
    assert_eq!(cart.read_prg_ram(0x6123), 0x5A);

    cart.write_prg(0x5101, 0x03);
    cart.write_prg(0x5120, 0x03);
    cart.write_prg(0x5121, 0x04);
    assert_eq!(cart.read_chr(0x0000), 0x83);
    assert_eq!(cart.read_chr_sprite(0x0400, 0), 0x84);

    cart.write_prg(0x5205, 7);
    cart.write_prg(0x5206, 9);
    assert_eq!(cart.read_prg_low(0x5205), 63);
    assert_eq!(cart.read_prg_low(0x5206), 0);

    let snapshot = cart.snapshot_state();

    cart.write_prg(0x5114, 0x87);
    cart.write_prg(0x5113, 0x00);
    cart.write_prg_ram(0x6123, 0x00);
    cart.write_prg(0x5205, 3);
    cart.write_prg(0x5206, 4);

    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg_ram(0x6123), 0x5A);
    assert_eq!(cart.read_prg_low(0x5205), 63);
}

#[test]
fn mapper_5_tracks_ppudata_chr_source_and_audio_status() {
    let mut cart = make_mmc5_cart();

    cart.write_prg(0x5101, 0x03);
    cart.write_prg(0x5128, 0x11);
    assert_eq!(cart.read_chr(0x0000), 0x91);

    cart.write_prg(0x5120, 0x03);
    assert_eq!(cart.read_chr(0x0000), 0x83);

    cart.write_prg(0x5015, 0x03);
    cart.write_prg(0x5000, 0xDF);
    cart.write_prg(0x5002, 0x08);
    cart.write_prg(0x5003, 0x18);
    assert_eq!(cart.read_prg_low(0x5015) & 0x01, 0x01);

    let mut non_zero = false;
    for _ in 0..64 {
        if cart.clock_expansion_audio().abs() > f32::EPSILON {
            non_zero = true;
            break;
        }
    }
    assert!(non_zero);

    cart.write_prg(0x5015, 0x00);
    assert_eq!(cart.read_prg_low(0x5015) & 0x03, 0x00);
}

#[test]
fn mapper_64_switches_prg_chr_modes_and_restores_state() {
    let mut cart = make_mapper64_cart();

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x0F);
    cart.write_prg(0x8001, 0x05);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_prg(0xE000), 31);

    cart.write_prg(0x8000, 0x46);
    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xC000), 3);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x0A);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x0C);
    assert_eq!(cart.read_chr(0x0000), 0x8A);
    assert_eq!(cart.read_chr(0x0400), 0x8B);
    assert_eq!(cart.read_chr(0x0800), 0x8C);
    assert_eq!(cart.read_chr(0x0C00), 0x8D);

    cart.write_prg(0x8000, 0x20);
    cart.write_prg(0x8001, 0x10);
    cart.write_prg(0x8000, 0x28);
    cart.write_prg(0x8001, 0x11);
    cart.write_prg(0x8000, 0x21);
    cart.write_prg(0x8001, 0x12);
    cart.write_prg(0x8000, 0x29);
    cart.write_prg(0x8001, 0x13);
    cart.write_prg(0xA000, 0x01);

    assert_eq!(cart.read_chr(0x0000), 0x90);
    assert_eq!(cart.read_chr(0x0400), 0x91);
    assert_eq!(cart.read_chr(0x0800), 0x92);
    assert_eq!(cart.read_chr(0x0C00), 0x93);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x8000, 0xA2);
    cart.write_prg(0x8001, 0x20);
    cart.write_prg(0x8000, 0xA3);
    cart.write_prg(0x8001, 0x21);
    cart.write_prg(0x8000, 0xA4);
    cart.write_prg(0x8001, 0x22);
    cart.write_prg(0x8000, 0xA5);
    cart.write_prg(0x8001, 0x23);

    assert_eq!(cart.read_chr(0x0000), 0xA0);
    assert_eq!(cart.read_chr(0x0400), 0xA1);
    assert_eq!(cart.read_chr(0x0800), 0xA2);
    assert_eq!(cart.read_chr(0x0C00), 0xA3);
    assert_eq!(cart.read_chr(0x1000), 0x90);
    assert_eq!(cart.read_chr(0x1400), 0x91);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x00);
    cart.write_prg(0xA000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_chr(0x0000), 0xA0);
    assert_eq!(cart.read_chr(0x1000), 0x90);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_13_switches_upper_chr_ram_page_only() {
    let mut cart = make_split_chr_cart(13, 8, 3);
    cart.prg_rom[0] = 0xFF;

    assert_eq!(cart.read_chr(0x0000), 0x60);
    assert_eq!(cart.read_chr(0x1000), 0x63);

    cart.write_prg(0x8000, 0x01);

    assert_eq!(cart.read_chr(0x0000), 0x60);
    assert_eq!(cart.read_chr(0x1000), 0x61);
}
