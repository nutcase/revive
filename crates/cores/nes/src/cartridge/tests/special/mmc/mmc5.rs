use super::super::super::*;

#[test]
fn mapper_5_uses_fill_mode_exram_attributes_and_scanline_irq() {
    let mut cart = make_mmc5_cart();
    let mut ppu = crate::ppu::Ppu::new();

    ppu.nametable[0][0] = 0x21;
    ppu.nametable[1][0] = 0x42;

    cart.write_prg(0x5105, 0b11_10_01_00);
    cart.write_prg(0x5106, 0x66);
    cart.write_prg(0x5107, 0x03);
    cart.write_prg(0x5C00, 0x33);

    assert_eq!(cart.resolve_nametable(0), Some(0));
    assert_eq!(cart.resolve_nametable(3), Some(3));
    assert_eq!(cart.read_nametable_byte(0, 0, &ppu.nametable), 0x21);
    assert_eq!(cart.read_nametable_byte(1, 0, &ppu.nametable), 0x42);
    assert_eq!(cart.read_nametable_byte(2, 0, &ppu.nametable), 0x33);
    assert_eq!(cart.read_nametable_byte(3, 0, &ppu.nametable), 0x66);
    assert_eq!(cart.read_nametable_byte(3, 960, &ppu.nametable), 0xFF);

    cart.write_prg(0x5104, 0x01);
    cart.write_prg(0x5105, 0x00);
    cart.write_prg(0x5C00, 0b10_001011);
    cart.notify_ppumask_mmc5(0x18);
    ppu.nametable[0][0] = 0x04;

    assert_eq!(cart.read_nametable_byte(0, 0, &ppu.nametable), 0x04);
    assert_eq!(cart.read_nametable_byte(0, 960, &ppu.nametable), 0x02);
    assert_eq!(cart.read_chr(0x0040), 0xAC);

    cart.write_prg(0x5203, 0x02);
    cart.write_prg(0x5204, 0x80);
    cart.mmc5_scanline_tick();
    cart.mmc5_scanline_tick();
    assert!(!cart.irq_pending());
    cart.mmc5_scanline_tick();
    assert!(cart.irq_pending());
    assert_eq!(cart.read_prg_low(0x5204), 0xC0);
    assert_eq!(cart.read_prg_low(0x5204), 0x40);

    let snapshot = cart.snapshot_state();
    cart.write_prg(0x5C00, 0x00);
    cart.notify_ppumask_mmc5(0x00);
    cart.restore_state(&snapshot);

    assert_eq!(cart.read_prg_low(0x5204), 0x40);
    assert_eq!(cart.read_prg_low(0x5C00), 0x00);
    cart.write_prg(0x5104, 0x02);
    assert_eq!(cart.read_prg_low(0x5C00), 0b10_001011);
}

#[test]
fn mapper_5_uses_pcm_read_mode_and_split_fetch() {
    let mut cart = make_mmc5_cart();

    cart.prg_rom[0] = 0x40;
    cart.prg_rom[1] = 0x00;
    cart.write_prg(0x5100, 0x03);
    cart.write_prg(0x5114, 0x80);
    cart.write_prg(0x5010, 0x81);
    assert_eq!(cart.read_prg_cpu(0x8000), 0x40);

    let audio_sample = cart.clock_expansion_audio();
    assert!(audio_sample < 0.0);

    assert_eq!(cart.read_prg_cpu(0x8001), 0x00);
    assert!(cart.irq_pending());
    assert_eq!(cart.read_prg_low(0x5010), 0x81);
    assert!(!cart.irq_pending());

    cart.write_prg(0x5104, 0x00);
    cart.write_prg(0x5201, 0x08);
    cart.write_prg(0x5202, 0x05);
    cart.write_prg(0x5C00 + 32, 0x40);
    cart.write_prg(0x5C00 + 29, 0x41);
    cart.write_prg(0x5C00 + 960, 0b10);
    cart.write_prg(0x5C00 + 967, 0b10);

    cart.notify_ppumask_mmc5(0x18);
    cart.write_prg(0x5200, 0x80 | 0x02);
    assert_eq!(cart.mmc5_split_bg_fetch(0, 0, 0), Some((0x95, 0x95, 0x02)));
    assert!(cart.mmc5_split_bg_fetch(16, 0, 0).is_none());

    cart.write_prg(0x5201, 0x00);
    cart.write_prg(0x5200, 0x80 | 0x40 | 30);
    assert!(cart.mmc5_split_bg_fetch(236, 0, 3).is_none());
    assert_eq!(
        cart.mmc5_split_bg_fetch(237, 0, 3),
        Some((0x95, 0x95, 0x02))
    );
}
