use super::super::*;

#[test]
fn mapper_63_latches_prg_mode_mirroring_and_chr_write_protect() {
    let mut cart = make_mapper63_cart();

    cart.write_prg(0xFFF6, 0x00);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 2);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_chr(0x0123, 0x5A);
    assert_eq!(cart.read_chr(0x0123), 0x5A);

    let snapshot = cart.snapshot_state();

    cart.write_prg(0xFBE9, 0x00);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_chr(0x0123, 0x11);
    assert_eq!(cart.read_chr(0x0123), 0x5A);

    cart.restore_state(&snapshot);
    cart.write_chr(0x0123, 0x33);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 2);
    assert_eq!(cart.read_chr(0x0123), 0x33);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}
