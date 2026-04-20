use super::super::super::*;

#[test]
fn mapper_230_toggles_between_multicart_and_contra_modes_on_reset() {
    let mut cart = make_mapper230_cart();

    cart.on_reset();
    cart.write_prg(0x8000, 0x23);
    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_prg(0xC000), 11);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();

    cart.on_reset();
    cart.write_prg(0x8000, 0x05);
    assert_eq!(cart.read_prg(0x8000), 5);
    assert_eq!(cart.read_prg(0xC000), 7);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_prg(0xC000), 11);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_227_switches_between_unrom_and_nrom_modes() {
    let mut cart = make_mapper227_cart();

    cart.write_prg(0x822E, 0);
    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_prg(0xC000), 15);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    cart.write_chr(0x0010, 0x33);
    assert_eq!(cart.read_chr(0x0010), 0x33);

    cart.write_prg(0x80F5, 0);
    assert_eq!(cart.read_prg(0x8000), 28);
    assert_eq!(cart.read_prg(0xC000), 29);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
    cart.write_chr(0x0010, 0x77);
    assert_eq!(cart.read_chr(0x0010), 0x33);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0);
    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 28);
    assert_eq!(cart.read_prg(0xC000), 29);
}
