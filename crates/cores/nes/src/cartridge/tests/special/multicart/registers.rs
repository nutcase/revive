use super::super::super::*;

#[test]
fn mapper_221_switches_prg_modes_and_write_protects_chr_ram() {
    let mut cart = make_mapper221_cart();

    cart.write_prg(0x8005, 0);
    cart.write_prg(0xC003, 0);
    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_prg(0xC000), 11);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x8006, 0);
    cart.write_prg(0xC005, 0);
    assert_eq!(cart.read_prg(0x8000), 12);
    assert_eq!(cart.read_prg(0xC000), 13);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_prg(0x8107, 0);
    cart.write_prg(0xC002, 0);
    assert_eq!(cart.read_prg(0x8000), 10);
    assert_eq!(cart.read_prg(0xC000), 15);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_chr(0x0010, 0x5A);
    cart.write_prg(0xC00A, 0);
    cart.write_chr(0x0010, 0x00);
    assert_eq!(cart.read_chr(0x0010), 0x5A);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8006, 0);
    cart.write_prg(0xC001, 0);
    cart.write_chr(0x0010, 0x11);
    assert_eq!(cart.read_chr(0x0010), 0x11);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 10);
    assert_eq!(cart.read_prg(0xC000), 15);
    assert_eq!(cart.read_chr(0x0010), 0x5A);

    cart.write_chr(0x0010, 0x33);
    assert_eq!(cart.read_chr(0x0010), 0x5A);
}

#[test]
fn mapper_243_uses_indexed_register_file_for_prg_chr_and_mirroring() {
    let mut cart = make_simple_bank_cart(243, 4, 16);

    cart.write_prg(0x4100, 0x05);
    cart.write_prg(0x4101, 0x02);
    cart.write_prg(0x4100, 0x02);
    cart.write_prg(0x4101, 0x01);
    cart.write_prg(0x4100, 0x04);
    cart.write_prg(0x4101, 0x01);
    cart.write_prg(0x4100, 0x06);
    cart.write_prg(0x4101, 0x02);
    cart.write_prg(0x4100, 0x07);
    cart.write_prg(0x4101, 0x00);

    cart.write_prg(0x4100, 0x05);
    assert_eq!(cart.read_prg_low(0x4101), 0x02);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xFFFF), 2);
    assert_eq!(cart.read_chr(0x0000), 0x4B);
    assert_eq!(cart.mirroring(), Mirroring::ThreeScreenLower);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x4100, 0x07);
    cart.write_prg(0x4101, 0x06);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg_low(0x4101), 0x02);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_chr(0x0000), 0x4B);
    assert_eq!(cart.mirroring(), Mirroring::ThreeScreenLower);
}
