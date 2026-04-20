use super::super::*;

#[test]
fn mapper_18_switches_prg_chr_and_prg_ram() {
    let mut cart = make_mapper18_cart();

    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0x8001, 0x01);
    cart.write_prg(0x8002, 0x0A);
    cart.write_prg(0x8003, 0x02);
    cart.write_prg(0x9000, 0x03);
    cart.write_prg(0x9001, 0x03);

    assert_eq!(cart.read_prg(0x8000), 0x15);
    assert_eq!(cart.read_prg(0xA000), 0x2A);
    assert_eq!(cart.read_prg(0xC000), 0x33);
    assert_eq!(cart.read_prg(0xE000), 0x3F);

    cart.write_prg(0xA000, 0x04);
    cart.write_prg(0xA001, 0x02);
    cart.write_prg(0xD002, 0x0F);
    cart.write_prg(0xD003, 0x00);

    assert_eq!(cart.read_chr(0x0000), 0xA4);
    assert_eq!(cart.read_chr(0x1C00), 0x8F);

    cart.write_prg_ram(0x6001, 0x99);
    assert_eq!(cart.read_prg_ram(0x6001), 0x00);

    cart.write_prg(0x9002, 0x03);
    cart.write_prg_ram(0x6001, 0x5A);
    assert_eq!(cart.read_prg_ram(0x6001), 0x5A);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x00);
    cart.write_prg_ram(0x6001, 0x00);

    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 0x15);
    assert_eq!(cart.read_prg_ram(0x6001), 0x5A);
    assert_eq!(cart.read_chr(0x0000), 0xA4);
}
