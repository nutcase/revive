use super::super::super::*;

#[test]
fn mapper_68_switches_prg_chr_nametables_and_prg_ram() {
    let mut cart = make_sunsoft4_cart();

    cart.write_prg(0x8000, 0x04);
    cart.write_prg(0x9000, 0x05);
    cart.write_prg(0xA000, 0x06);
    cart.write_prg(0xB000, 0x07);
    cart.write_prg(0xC000, 0x02);
    cart.write_prg(0xD000, 0x03);
    cart.write_prg(0xE000, 0x11);
    cart.write_prg(0xF000, 0x12);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_chr(0x0000), 8);
    assert_eq!(cart.read_chr(0x0800), 10);
    assert_eq!(cart.read_chr(0x1000), 12);
    assert_eq!(cart.read_chr(0x1800), 14);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    assert!(!cart.nametable_writes_to_internal_vram());

    assert_eq!(cart.read_nametable_byte(0, 0, &[[0; 1024]; 2]), 0x82);
    assert_eq!(cart.read_nametable_byte(1, 0, &[[0; 1024]; 2]), 0x83);

    assert_eq!(cart.read_prg_ram(0x6000), 0x00);
    cart.write_prg_ram(0x6000, 0x5A);
    assert_eq!(cart.read_prg_ram(0x6000), 0x5A);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0xC000, 0x00);
    cart.write_prg(0xE000, 0x02);
    cart.write_prg(0xF000, 0x00);

    cart.restore_state(&snapshot);
    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_chr(0x1000), 12);
    assert_eq!(cart.read_nametable_byte(0, 0, &[[0; 1024]; 2]), 0x82);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    assert_eq!(cart.read_prg_ram(0x6000), 0x5A);
}

#[test]
fn mapper_68_ppu_reads_chr_rom_nametables() {
    let mut cart = make_sunsoft4_cart();
    let mut ppu = crate::ppu::Ppu::new();
    ppu.nametable[0][0] = 0x11;
    ppu.nametable[1][0] = 0x22;

    cart.write_prg(0xC000, 0x02);
    cart.write_prg(0xD000, 0x03);
    cart.write_prg(0xE000, 0x10);

    ppu.v = 0x2000;
    let _ = ppu.read_register(0x2007, Some(&cart));
    ppu.v = 0x2000;
    let _ = ppu.read_register(0x2007, Some(&cart));
    let rom_nt0 = ppu.read_register(0x2007, Some(&cart));
    assert_eq!(rom_nt0, 0x82);

    ppu.v = 0x2400;
    let _ = ppu.read_register(0x2007, Some(&cart));
    ppu.v = 0x2400;
    let _ = ppu.read_register(0x2007, Some(&cart));
    let rom_nt1 = ppu.read_register(0x2007, Some(&cart));
    assert_eq!(rom_nt1, 0x83);

    ppu.v = 0x2000;
    ppu.write_register(0x2007, 0x99, Some(&mut cart));
    assert_eq!(ppu.nametable[0][0], 0x11);
}

#[test]
fn mapper_69_fme7_switches_prg_chr_prg_ram_and_restores_state() {
    let mut cart = make_fme7_cart();

    cart.write_prg(0x8000, 9);
    cart.write_prg(0xA000, 0x03);
    cart.write_prg(0x8000, 0x0A);
    cart.write_prg(0xA000, 0x04);
    cart.write_prg(0x8000, 0x0B);
    cart.write_prg(0xA000, 0x05);

    cart.write_prg(0x8000, 0);
    cart.write_prg(0xA000, 0x12);
    cart.write_prg(0x8000, 7);
    cart.write_prg(0xA000, 0x19);

    cart.write_prg(0x8000, 8);
    cart.write_prg(0xA000, 0x82);
    assert_eq!(cart.read_prg_ram(0x6000), 0x02);

    cart.write_prg(0xA000, 0xC0);
    cart.write_prg_ram(0x6003, 0x5A);

    cart.write_prg(0x8000, 0x0C);
    cart.write_prg(0xA000, 0x02);

    assert_eq!(cart.read_prg(0x8000), 0x03);
    assert_eq!(cart.read_prg(0xA000), 0x04);
    assert_eq!(cart.read_prg(0xC000), 0x05);
    assert_eq!(cart.read_prg(0xE000), 0x3F);
    assert_eq!(cart.read_chr(0x0000), 0x92);
    assert_eq!(cart.read_chr(0x1C00), 0x99);
    assert_eq!(cart.read_prg_ram(0x6003), 0x5A);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenLower);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 9);
    cart.write_prg(0xA000, 0x00);
    cart.write_prg(0x8000, 0);
    cart.write_prg(0xA000, 0x00);
    cart.write_prg(0x8000, 8);
    cart.write_prg(0xA000, 0x40);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 0x03);
    assert_eq!(cart.read_chr(0x0000), 0x92);
    assert_eq!(cart.read_prg_ram(0x6003), 0x5A);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenLower);
}
