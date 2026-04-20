use super::super::super::*;

#[test]
fn mapper_137_register_file_controls_prg_and_chr() {
    let mut cart = make_mapper137_cart();

    cart.write_prg(0x4100, 5);
    cart.write_prg(0x4101, 3);
    cart.write_prg(0x4100, 0);
    cart.write_prg(0x4101, 1);
    cart.write_prg(0x4100, 1);
    cart.write_prg(0x4101, 2);
    cart.write_prg(0x4100, 2);
    cart.write_prg(0x4101, 3);
    cart.write_prg(0x4100, 3);
    cart.write_prg(0x4101, 4);
    cart.write_prg(0x4100, 4);
    cart.write_prg(0x4101, 0x05);
    cart.write_prg(0x4100, 6);
    cart.write_prg(0x4101, 0x01);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_chr(0x0000), 0xA1);
    assert_eq!(cart.read_chr(0x0400), 0xB2);
    assert_eq!(cart.read_chr(0x0800), 0xA3);
    assert_eq!(cart.read_chr(0x0C00), 0xBC);
    assert_eq!(cart.read_chr(0x1000), 0xBC);
    assert_eq!(cart.read_prg_low(0x4101), 0x01);
}

#[test]
fn mapper_150_register_file_controls_prg_chr_and_mirroring() {
    let mut cart = make_mapper150_cart();

    cart.write_prg(0x4100, 5);
    cart.write_prg(0x4101, 0x02);
    cart.write_prg(0x4100, 4);
    cart.write_prg(0x4101, 0x01);
    cart.write_prg(0x4100, 6);
    cart.write_prg(0x4101, 0x03);
    cart.write_prg(0x4100, 7);
    cart.write_prg(0x4101, 0x04);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xFFFF), 2);
    assert_eq!(cart.read_chr(0x0000), 0xB7);
    assert_eq!(cart.read_prg_low(0x4101), 0x04);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x4100, 5);
    cart.write_prg(0x4101, 0x01);
    assert_eq!(cart.read_prg(0x8000), 1);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_chr(0x0000), 0xB7);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}
