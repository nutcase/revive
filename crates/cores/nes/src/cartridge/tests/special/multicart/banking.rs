use super::super::super::*;

#[test]
fn mapper_246_switches_prg_chr_and_vector_reads() {
    let mut cart = make_mapper246_cart();

    cart.write_prg_ram(0x6000, 0x01);
    cart.write_prg_ram(0x6001, 0x02);
    cart.write_prg_ram(0x6002, 0x03);
    cart.write_prg_ram(0x6003, 0x04);
    cart.write_prg_ram(0x6004, 0x05);
    cart.write_prg_ram(0x6005, 0x06);
    cart.write_prg_ram(0x6006, 0x07);
    cart.write_prg_ram(0x6007, 0x08);

    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xA000), 2);
    assert_eq!(cart.read_prg(0xC000), 3);
    assert_eq!(cart.read_prg(0xE000), 4);
    assert_eq!(cart.read_prg(0xFFFC), 20);
    assert_eq!(cart.read_chr(0x0000), 0x45);
    assert_eq!(cart.read_chr(0x0800), 0x46);
    assert_eq!(cart.read_chr(0x1000), 0x47);
    assert_eq!(cart.read_chr(0x1800), 0x48);

    cart.write_prg_ram(0x6800, 0xA5);
    assert_eq!(cart.read_prg_ram(0x6800), 0xA5);

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x6003, 0x00);
    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0xE000), 4);
    assert_eq!(cart.read_prg(0xFFFC), 20);
}

#[test]
fn mapper_236_chr_rom_variant_switches_prg_chr_and_modes() {
    let mut cart = make_mapper236_cart(false);

    cart.write_prg(0x801A, 0);
    assert_eq!(cart.read_chr(0x0000), 0x8A);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0xC00B, 0);
    assert_eq!(cart.read_prg(0x8000), 11);
    assert_eq!(cart.read_prg(0xC000), 15);

    cart.write_prg(0xC02D, 0);
    assert_eq!(cart.read_prg(0x8000), 12);
    assert_eq!(cart.read_prg(0xC000), 13);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8001, 0);
    cart.write_prg(0xC003, 0);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 12);
    assert_eq!(cart.read_prg(0xC000), 13);
    assert_eq!(cart.read_chr(0x0000), 0x8A);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_236_chr_ram_variant_switches_outer_and_inner_prg_banks() {
    let mut cart = make_mapper236_cart(true);

    cart.write_prg(0x8015, 0);
    cart.write_prg(0xC002, 0);
    assert_eq!(cart.read_prg(0x8000), 42);
    assert_eq!(cart.read_prg(0xC000), 47);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_chr(0x0123, 0x5A);
    assert_eq!(cart.read_chr(0x0123), 0x5A);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0xC026, 0);
    assert_eq!(cart.read_prg(0x8000), 46);
    assert_eq!(cart.read_prg(0xC000), 47);

    cart.write_prg(0x8000, 0);
    cart.write_chr(0x0123, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 42);
    assert_eq!(cart.read_prg(0xC000), 47);
    assert_eq!(cart.read_chr(0x0123), 0x5A);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}
