use super::super::*;

#[test]
fn mapper_242_switches_prg_modes_and_write_protects_chr_ram() {
    let mut cart = make_mapper242_cart();

    cart.write_prg(0x824E, 0);
    assert_eq!(cart.read_prg(0x8000), 19);
    assert_eq!(cart.read_prg(0xC000), 23);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    cart.write_chr(0x0000, 0x5A);
    assert_eq!(cart.read_chr(0x0000), 0x5A);

    cart.write_prg(0x80B9, 0);
    assert_eq!(cart.read_prg(0x8000), 14);
    assert_eq!(cart.read_prg(0xC000), 15);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
    cart.write_chr(0x0000, 0x11);
    assert_eq!(cart.read_chr(0x0000), 0x5A);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 14);
    assert_eq!(cart.read_prg(0xC000), 15);
    assert_eq!(cart.read_chr(0x0000), 0x5A);
}
