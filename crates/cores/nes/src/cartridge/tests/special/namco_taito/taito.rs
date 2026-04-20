use super::super::super::*;

#[test]
fn mapper_48_uses_taito_banking_and_delayed_irq() {
    let mut cart = make_mapper48_cart();

    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8002, 0x05);
    cart.write_prg(0x8003, 0x06);
    cart.write_prg(0xA000, 0x07);
    cart.write_prg(0xA001, 0x08);
    cart.write_prg(0xA002, 0x09);
    cart.write_prg(0xA003, 0x0A);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 14);
    assert_eq!(cart.read_prg(0xE000), 15);
    assert_eq!(cart.read_chr(0x0000), 0x8A);
    assert_eq!(cart.read_chr(0x0400), 0x8B);
    assert_eq!(cart.read_chr(0x0800), 0x8C);
    assert_eq!(cart.read_chr(0x0C00), 0x8D);
    assert_eq!(cart.read_chr(0x1000), 0x87);
    assert_eq!(cart.read_chr(0x1C00), 0x8A);

    cart.write_prg(0xE000, 0x40);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0xC000, 0xFE);
    cart.write_prg(0xC001, 0x00);
    cart.write_prg(0xC002, 0x00);
    cart.clock_irq_counter();
    assert!(!cart.irq_pending());
    cart.clock_irq_counter();
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(3);
    assert!(!cart.irq_pending());

    let snapshot = cart.snapshot_state();
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());

    cart.write_prg(0xC003, 0x00);
    assert!(!cart.irq_pending());

    cart.restore_state(&snapshot);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());
}

#[test]
fn mapper_33_switches_prg_chr_and_mirroring() {
    let mut cart = make_taito_tc0190_cart();

    cart.write_prg(0x8000, 0x43);
    cart.write_prg(0x8001, 0x05);
    cart.write_prg(0x8002, 0x04);
    cart.write_prg(0x8003, 0x06);
    cart.write_prg(0xA000, 0x0A);
    cart.write_prg(0xA001, 0x0B);
    cart.write_prg(0xA002, 0x0C);
    cart.write_prg(0xA003, 0x0D);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 5);
    assert_eq!(cart.read_prg(0xC000), 14);
    assert_eq!(cart.read_prg(0xE000), 15);
    assert_eq!(cart.read_chr(0x0000), 0x48);
    assert_eq!(cart.read_chr(0x0400), 0x49);
    assert_eq!(cart.read_chr(0x0800), 0x4C);
    assert_eq!(cart.read_chr(0x0C00), 0x4D);
    assert_eq!(cart.read_chr(0x1000), 0x4A);
    assert_eq!(cart.read_chr(0x1400), 0x4B);
    assert_eq!(cart.read_chr(0x1800), 0x4C);
    assert_eq!(cart.read_chr(0x1C00), 0x4D);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8002, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_chr(0x0000), 0x48);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}

#[test]
fn mapper_80_switches_prg_chr_mirroring_and_internal_ram() {
    let mut cart = make_taito_x1005_cart();

    cart.write_prg_ram(0x7EF0, 0x03);
    cart.write_prg_ram(0x7EF1, 0x04);
    cart.write_prg_ram(0x7EF2, 0x0A);
    cart.write_prg_ram(0x7EF3, 0x0B);
    cart.write_prg_ram(0x7EF4, 0x0C);
    cart.write_prg_ram(0x7EF5, 0x0D);
    cart.write_prg_ram(0x7EF6, 0x01);
    cart.write_prg_ram(0x7EF8, 0x02);
    cart.write_prg_ram(0x7EF9, 0x03);
    cart.write_prg_ram(0x7EFA, 0x0C);

    assert_eq!(cart.read_prg(0x8000), 2);
    assert_eq!(cart.read_prg(0xA000), 3);
    assert_eq!(cart.read_prg(0xC000), 12);
    assert_eq!(cart.read_prg(0xE000), 15);
    assert_eq!(cart.read_chr(0x0000), 0x86);
    assert_eq!(cart.read_chr(0x0400), 0x87);
    assert_eq!(cart.read_chr(0x0800), 0x88);
    assert_eq!(cart.read_chr(0x0C00), 0x89);
    assert_eq!(cart.read_chr(0x1000), 0x8A);
    assert_eq!(cart.read_chr(0x1400), 0x8B);
    assert_eq!(cart.read_chr(0x1800), 0x8C);
    assert_eq!(cart.read_chr(0x1C00), 0x8D);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    assert_eq!(cart.read_prg_ram(0x7F20), 0x00);
    cart.write_prg_ram(0x7F20, 0x5A);
    assert_eq!(cart.read_prg_ram(0x7F20), 0x5A);
    assert_eq!(cart.read_prg_ram(0x7FA0), 0x5A);

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x7EF8, 0x00);
    cart.write_prg_ram(0x7EFA, 0x00);
    cart.write_prg_ram(0x7F20, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0xC000), 12);
    assert_eq!(cart.read_prg_ram(0x7FA0), 0x5A);
    assert_eq!(cart.read_chr(0x1000), 0x8A);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}

#[test]
fn mapper_82_switches_prg_chr_and_segmented_ram() {
    let mut cart = make_taito_x1017_cart();

    assert_eq!(cart.read_prg_ram(0x6000), 0x00);
    assert_eq!(cart.read_prg_ram(0x6800), 0x00);
    assert_eq!(cart.read_prg_ram(0x7000), 0x00);

    cart.write_prg_ram(0x7EF0, 0x01);
    cart.write_prg_ram(0x7EF1, 0x02);
    cart.write_prg_ram(0x7EF2, 0x0A);
    cart.write_prg_ram(0x7EF3, 0x0B);
    cart.write_prg_ram(0x7EF4, 0x0C);
    cart.write_prg_ram(0x7EF5, 0x0D);
    cart.write_prg_ram(0x7EF6, 0x03);
    cart.write_prg_ram(0x7EF7, 0xCA);
    cart.write_prg_ram(0x7EF8, 0x69);
    cart.write_prg_ram(0x7EF9, 0x84);
    cart.write_prg_ram(0x7EFA, 0x0C);
    cart.write_prg_ram(0x7EFB, 0x10);
    cart.write_prg_ram(0x7EFC, 0x14);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 5);
    assert_eq!(cart.read_prg(0xE000), 15);
    assert_eq!(cart.read_chr(0x0000), 0x9A);
    assert_eq!(cart.read_chr(0x0400), 0x9B);
    assert_eq!(cart.read_chr(0x0800), 0x9C);
    assert_eq!(cart.read_chr(0x0C00), 0x9D);
    assert_eq!(cart.read_chr(0x1000), 0x92);
    assert_eq!(cart.read_chr(0x1400), 0x93);
    assert_eq!(cart.read_chr(0x1800), 0x94);
    assert_eq!(cart.read_chr(0x1C00), 0x95);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);

    cart.write_prg_ram(0x6000, 0xA1);
    cart.write_prg_ram(0x6800, 0xB2);
    cart.write_prg_ram(0x7000, 0xC3);
    assert_eq!(cart.read_prg_ram(0x6000), 0xA1);
    assert_eq!(cart.read_prg_ram(0x6800), 0xB2);
    assert_eq!(cart.read_prg_ram(0x7000), 0xC3);

    let snapshot = cart.snapshot_state();
    cart.write_prg_ram(0x7EF7, 0x00);
    cart.write_prg_ram(0x7EFA, 0x00);
    cart.write_prg_ram(0x6000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg_ram(0x6000), 0xA1);
    assert_eq!(cart.read_chr(0x0000), 0x9A);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
}

#[test]
fn mapper_207_uses_chr_bank_bits_for_nametable_mapping() {
    let mut cart = make_taito_x1005_cart();
    let mut ppu = crate::ppu::Ppu::new();
    cart.mapper = 207;
    ppu.nametable[0][0] = 0x11;
    ppu.nametable[1][0] = 0x22;

    cart.write_prg_ram(0x7EF0, 0x81);
    cart.write_prg_ram(0x7EF1, 0x02);

    assert_eq!(cart.read_chr(0x0000), 0x82);
    assert_eq!(cart.read_chr(0x0400), 0x83);
    assert_eq!(cart.read_chr(0x0800), 0x84);
    assert_eq!(cart.read_chr(0x0C00), 0x85);
    assert_eq!(cart.mirroring(), Mirroring::HorizontalSwapped);

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
