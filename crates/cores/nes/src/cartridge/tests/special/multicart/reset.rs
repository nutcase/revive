use super::super::super::*;

#[test]
fn mapper_59_latches_address_modes_and_unlocks_on_reset() {
    let mut cart = make_mapper59_cart();

    cart.write_prg(0x80BD, 0x00);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x45);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8122, 0x00);
    assert_eq!(cart.read_prg(0x8000), 0);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x45);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0x8222, 0x00);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x42);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_prg(0x80BD, 0x00);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_chr(0x0000), 0x42);

    cart.on_reset();
    cart.write_prg(0x80BD, 0x00);
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x45);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_60_cycles_through_four_nrom_games_on_reset() {
    let mut cart = make_mapper60_cart();

    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xC000), 0);
    assert_eq!(cart.read_chr(0x0000), 0x50);

    cart.on_reset();
    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xC000), 1);
    assert_eq!(cart.read_chr(0x0000), 0x51);

    cart.on_reset();
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 2);
    assert_eq!(cart.read_chr(0x0000), 0x52);

    let snapshot = cart.snapshot_state();

    cart.on_reset();
    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x53);

    cart.on_reset();
    assert_eq!(cart.read_prg(0x8000), 0);
    assert_eq!(cart.read_prg(0xC000), 0);
    assert_eq!(cart.read_chr(0x0000), 0x50);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xC000), 2);
    assert_eq!(cart.read_chr(0x0000), 0x52);
}

#[test]
fn mapper_61_latches_prg_chr_and_mirroring_modes() {
    let mut cart = make_mapper61_cart();

    cart.write_prg(0x89B5, 0x00);
    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_prg(0xC000), 11);
    assert_eq!(cart.read_chr(0x0000), 0x49);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();

    cart.write_prg(0x83C2, 0x00);
    assert_eq!(cart.read_prg(0x8000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_chr(0x0000), 0x43);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_prg(0xC000), 11);
    assert_eq!(cart.read_chr(0x0000), 0x49);
}
