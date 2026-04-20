use super::super::*;

#[test]
fn mapper_19_switches_prg_chr_and_chip_ram_port() {
    let mut cart = make_mapper19_cart();

    cart.write_prg(0xE000, 0x03);
    cart.write_prg(0xE800, 0x04);
    cart.write_prg(0xF000, 0x05);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_prg(0xE000), 63);

    cart.write_prg(0x8000, 0x10);
    cart.write_prg(0x8800, 0x11);
    cart.write_prg(0x9000, 0x12);
    assert_eq!(cart.read_chr(0x0000), 0x90);
    assert_eq!(cart.read_chr(0x0400), 0x91);
    assert_eq!(cart.read_chr(0x0800), 0x92);

    cart.write_prg(0xF800, 0x40);
    cart.write_prg_low(0x4800, 0x5A);
    assert_eq!(cart.read_prg_low(0x4800), 0x5A);

    cart.write_prg(0xF800, 0xC0);
    cart.write_prg_low(0x4800, 0x11);
    assert_eq!(cart.read_prg_low(0x4800), 0x00);
    cart.write_prg(0xF800, 0xC0);
    assert_eq!(cart.read_prg_low(0x4800), 0x11);

    cart.write_prg_low(0x5000, 0x34);
    cart.write_prg_low(0x5800, 0x92);
    let snapshot = cart.snapshot_state();

    cart.write_prg_low(0x5000, 0x00);
    cart.write_prg_low(0x5800, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg_low(0x5000), 0x34);
    assert_eq!(cart.read_prg_low(0x5800), 0x92);
}

#[test]
fn mapper_210_switches_prg_chr_and_namco175_ram() {
    let mut cart = make_mapper210_cart(false);

    cart.write_prg(0xE000, 0x03);
    cart.write_prg(0xE800, 0x04);
    cart.write_prg(0xF000, 0x05);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_prg(0xE000), 63);

    cart.write_prg(0x8000, 0x10);
    cart.write_prg(0xB800, 0x17);
    assert_eq!(cart.read_chr(0x0000), 0x90);
    assert_eq!(cart.read_chr(0x1C00), 0x97);

    cart.write_prg_ram(0x6002, 0x55);
    assert_eq!(cart.read_prg_ram(0x6002), 0x00);

    cart.write_prg(0xC000, 0x01);
    cart.write_prg_ram(0x6002, 0x5A);
    assert_eq!(cart.read_prg_ram(0x6002), 0x5A);
    assert_eq!(cart.read_prg_ram(0x6802), 0x5A);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0xE000, 0x00);
    cart.write_prg_ram(0x6002, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg_ram(0x6002), 0x5A);
}
