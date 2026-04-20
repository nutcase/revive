use super::super::super::*;

#[test]
fn mapper_225_switches_prg_chr_and_exposes_low_nibble_ram() {
    let mut cart = make_mapper225_cart(225);

    cart.write_prg(0xC085, 0);
    assert_eq!(cart.read_prg(0x8000), 66);
    assert_eq!(cart.read_prg(0xC000), 67);
    assert_eq!(cart.read_chr(0x0000), 0xC5);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_prg(0x5802, 0x3C);
    assert_eq!(cart.read_prg_low(0x5802), 0x0C);

    cart.write_prg(0xF081, 0);
    assert_eq!(cart.read_prg(0x8000), 66);
    assert_eq!(cart.read_prg(0xC000), 66);
    assert_eq!(cart.read_chr(0x0000), 0xC1);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 66);
    assert_eq!(cart.read_prg(0xC000), 66);
    assert_eq!(cart.read_prg_low(0x5802), 0x0C);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_228_selects_prg_chip_and_chr_bank() {
    let mut cart = make_mapper228_cart();

    cart.write_prg(0xB885, 0x02);
    assert_eq!(cart.read_prg(0x8000), 66);
    assert_eq!(cart.read_prg(0xC000), 67);
    assert_eq!(cart.read_chr(0x0000), 0x36);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x9000, 0);
    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xC000), 0);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 66);
    assert_eq!(cart.read_prg(0xC000), 67);
    assert_eq!(cart.read_chr(0x0000), 0x36);
}

#[test]
fn mapper_255_matches_225_bank_switching_without_low_ram() {
    let mut cart = make_mapper225_cart(255);

    cart.write_prg(0xC085, 0);
    assert_eq!(cart.read_prg(0x8000), 66);
    assert_eq!(cart.read_prg(0xC000), 67);
    assert_eq!(cart.read_chr(0x0000), 0xC5);

    cart.write_prg(0xF081, 0);
    assert_eq!(cart.read_prg(0x8000), 66);
    assert_eq!(cart.read_prg(0xC000), 66);
    assert_eq!(cart.read_prg_low(0x5802), 0x00);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_231_latches_prg_and_preserves_chr_ram_in_save_state() {
    let mut cart = make_mapper231_cart();

    cart.write_prg(0x80A0, 0);
    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xC000), 1);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x801E, 0);
    assert_eq!(cart.read_prg(0x8000), 30);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_chr(0x0123, 0x5A);
    let snapshot = cart.snapshot_state();
    cart.write_prg(0x80A0, 0);
    cart.write_chr(0x0123, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 30);
    assert_eq!(cart.read_prg(0xC000), 30);
    assert_eq!(cart.read_chr(0x0123), 0x5A);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}
