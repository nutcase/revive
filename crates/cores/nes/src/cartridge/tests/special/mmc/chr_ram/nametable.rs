use super::super::super::super::*;

#[test]
fn mapper_118_uses_chr_bank_bits_for_nametable_mapping() {
    let mut cart = make_mmc3_mixed_chr_cart(118, 8, 16, 0);
    let mut ppu = crate::ppu::Ppu::new();
    ppu.nametable[0][0] = 0x11;
    ppu.nametable[1][0] = 0x22;

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x80);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x00);

    ppu.v = 0x2000;
    let _ = ppu.read_register(0x2007, Some(&cart));
    ppu.v = 0x2000;
    let _ = ppu.read_register(0x2007, Some(&cart));
    assert_eq!(ppu.read_register(0x2007, Some(&cart)), 0x22);

    ppu.v = 0x2800;
    let _ = ppu.read_register(0x2007, Some(&cart));
    ppu.v = 0x2800;
    let _ = ppu.read_register(0x2007, Some(&cart));
    assert_eq!(ppu.read_register(0x2007, Some(&cart)), 0x11);

    ppu.v = 0x2400;
    ppu.write_register(0x2007, 0x77, Some(&mut cart));
    assert_eq!(ppu.nametable[1][0], 0x77);
    assert_eq!(ppu.nametable[0][0], 0x11);
}
