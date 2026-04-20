use super::super::super::*;

#[test]
fn mapper_19_uses_nametable_alias_irq_and_audio() {
    let mut cart = make_mapper19_cart();
    let ppu = crate::ppu::Ppu::new();

    cart.write_prg(0xC000, 0xE0);
    cart.write_prg(0xC800, 0xE1);
    cart.write_prg(0xD000, 0x07);
    cart.write_prg(0xD800, 0xE0);

    cart.write_nametable_byte(0, 0x012, &mut [[0; 1024]; 2], 0x44);
    cart.write_nametable_byte(1, 0x012, &mut [[0; 1024]; 2], 0x55);
    assert_eq!(cart.read_nametable_byte(0, 0x012, &ppu.nametable), 0x44);
    assert_eq!(cart.read_nametable_byte(1, 0x012, &ppu.nametable), 0x55);
    assert_eq!(cart.read_nametable_byte(2, 0x012, &ppu.nametable), 0x87);

    cart.write_prg(0x8000, 0xE0);
    cart.write_prg(0x8800, 0xE1);
    cart.write_chr(0x0012, 0x66);
    cart.write_chr(0x0412, 0x77);
    assert_eq!(cart.read_chr(0x0012), 0x66);
    assert_eq!(cart.read_chr(0x0412), 0x77);

    cart.write_prg_low(0x5000, 0xFD);
    cart.write_prg_low(0x5800, 0xFF);
    cart.clock_irq_counter_cycles(1);
    assert!(!cart.irq_pending());
    cart.clock_irq_counter_cycles(1);
    assert!(cart.irq_pending());
    assert_eq!(cart.read_prg_low(0x5800), 0xFF);
    cart.acknowledge_irq();
    assert!(!cart.irq_pending());

    cart.write_prg(0xE000, 0x03);
    cart.write_prg(0xF800, 0x40);
    for (addr, value) in [
        (0x00, 0x10),
        (0x01, 0x00),
        (0x02, 0x00),
        (0x03, 0x00),
        (0x04, 0xFC),
        (0x05, 0x00),
        (0x06, 0x00),
        (0x07, 0x0F),
        (0x7F, 0x70),
    ] {
        cart.write_prg(0xF800, 0x40 | addr);
        cart.write_prg_low(0x4800, value);
    }
    cart.write_prg(0xF800, 0x40);
    cart.write_prg_low(0x4800, 0x98);

    let mut non_zero = false;
    for _ in 0..64 {
        if cart.clock_expansion_audio().abs() > f32::EPSILON {
            non_zero = true;
            break;
        }
    }
    assert!(non_zero);
}

#[test]
fn mapper_210_uses_namco340_mirroring_control() {
    let mut cart = make_mapper210_cart(true);

    cart.write_prg(0xE000, 0x00);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenLower);
    cart.write_prg(0xE000, 0x40);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
    cart.write_prg(0xE000, 0x80);
    assert_eq!(cart.mirroring(), Mirroring::OneScreenUpper);
    cart.write_prg(0xE000, 0xC0);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    cart.write_prg(0xE800, 0x06);
    cart.write_prg(0xF000, 0x07);
    assert_eq!(cart.read_prg(0xA000), 6);
    assert_eq!(cart.read_prg(0xC000), 7);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0xE000, 0x01);
    cart.write_prg(0xE800, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    assert_eq!(cart.read_prg(0xA000), 6);
}

#[test]
fn mapper_206_uses_namco108_bank_layout() {
    let mut cart = make_namco108_cart(206, 8, 16);

    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0x8001, 0x03);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0x8001, 0x04);
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0x8001, 0x06);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0x8001, 0x08);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0x8001, 0x0A);
    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0x8001, 0x0B);
    cart.write_prg(0x8000, 0x04);
    cart.write_prg(0x8001, 0x0C);
    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0x8001, 0x0D);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_prg(0xE000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x26);
    assert_eq!(cart.read_chr(0x0400), 0x27);
    assert_eq!(cart.read_chr(0x0800), 0x28);
    assert_eq!(cart.read_chr(0x1000), 0x2A);
    assert_eq!(cart.read_chr(0x1C00), 0x2D);
}

#[test]
fn mapper_112_uses_hardwired_prg_layout_and_2k_chr_banks() {
    let mut cart = make_namco108_cart(112, 8, 32);

    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0xA000, 0x03);
    cart.write_prg(0x8000, 0x01);
    cart.write_prg(0xA000, 0x04);
    cart.write_prg(0x8000, 0x02);
    cart.write_prg(0xA000, 0x05);
    cart.write_prg(0x8000, 0x03);
    cart.write_prg(0xA000, 0x06);
    cart.write_prg(0x8000, 0x04);
    cart.write_prg(0xA000, 0x07);
    cart.write_prg(0x8000, 0x05);
    cart.write_prg(0xA000, 0x08);
    cart.write_prg(0x8000, 0x06);
    cart.write_prg(0xA000, 0x09);
    cart.write_prg(0x8000, 0x07);
    cart.write_prg(0xA000, 0x0A);
    cart.write_prg(0xE000, 0x01);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_prg(0xA000), 4);
    assert_eq!(cart.read_prg(0xC000), 6);
    assert_eq!(cart.read_prg(0xE000), 7);
    assert_eq!(cart.read_chr(0x0000), 0x2A);
    assert_eq!(cart.read_chr(0x0400), 0x2B);
    assert_eq!(cart.read_chr(0x0800), 0x2C);
    assert_eq!(cart.read_chr(0x0C00), 0x2D);
    assert_eq!(cart.read_chr(0x1000), 0x27);
    assert_eq!(cart.read_chr(0x1400), 0x28);
    assert_eq!(cart.read_chr(0x1800), 0x29);
    assert_eq!(cart.read_chr(0x1C00), 0x2A);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x8000, 0x00);
    cart.write_prg(0xA000, 0x00);
    cart.write_prg(0xE000, 0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg(0x8000), 3);
    assert_eq!(cart.read_chr(0x1000), 0x27);
    assert_eq!(cart.mirroring(), Mirroring::Horizontal);
}
