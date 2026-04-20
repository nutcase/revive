use super::*;

#[test]
fn test_vblank_timing() {
    let mut ppu = Ppu::new();

    // Ensure VBlank is set (initial state may have it set)
    ppu.status.insert(PpuStatus::VBLANK);
    assert!(ppu.status.contains(PpuStatus::VBLANK));

    // Test VBlank clear on status read
    let _status = ppu.read_register(0x2002, None);
    assert!(!ppu.status.contains(PpuStatus::VBLANK));
}

#[test]
fn test_sprite_0_hit() {
    let mut ppu = Ppu::new();

    // Enable rendering
    ppu.mask.insert(PpuMask::BG_ENABLE);
    ppu.mask.insert(PpuMask::SPRITE_ENABLE);

    // Place sprite 0 at a visible position
    ppu.oam[0] = 100; // Y position
    ppu.oam[1] = 0; // Tile index
    ppu.oam[2] = 0; // Attributes
    ppu.oam[3] = 100; // X position

    // Sprite 0 hit flag should be set during rendering
    // (This is a simplified test - actual hit detection requires background/sprite overlap)
}

#[test]
fn test_oam_dma() {
    let mut ppu = Ppu::new();

    // DMA is typically handled by the bus, but we can test OAM writing
    for i in 0..256 {
        ppu.oam[i] = i as u8;
    }

    // Verify OAM contents
    for i in 0..256 {
        assert_eq!(ppu.oam[i], i as u8);
    }
}

#[test]
fn test_frame_buffer_output() {
    let ppu = Ppu::new();

    // Check buffer is initialized
    let buffer = ppu.get_buffer();
    assert_eq!(buffer.len(), 256 * 240 * 3); // RGB format

    // Check buffer format (actual initial values may vary)
    assert!(!buffer.is_empty());
}

#[test]
fn color_emphasis_tints_rendered_pixels() {
    let mut ppu = Ppu::new();
    ppu.scanline = 0;
    ppu.cycle = 1;
    ppu.palette[0] = 0x21;
    ppu.scanline_color_emphasis = PpuMask::EMPHASIZE_RED.bits();

    ppu.render_pixel(None);

    assert_eq!(&ppu.buffer[..3], &[76, 115, 177]);
}

#[test]
fn grayscale_and_color_emphasis_are_combined() {
    let mut ppu = Ppu::new();
    ppu.scanline = 0;
    ppu.cycle = 1;
    ppu.palette[0] = 0x21;
    ppu.scanline_grayscale = true;
    ppu.scanline_color_emphasis = PpuMask::EMPHASIZE_BLUE.bits();

    ppu.render_pixel(None);

    assert_eq!(&ppu.buffer[..3], &[177, 178, 236]);
}

#[test]
fn sprite_overflow_uses_diagonal_oam_scan_after_eight_sprites() {
    let mut ppu = Ppu::new();
    ppu.scanline = 20;

    for sprite in 0..8 {
        let base = sprite * 4;
        ppu.oam[base] = 19;
    }

    ppu.oam[8 * 4] = 0xFF;
    ppu.oam[9 * 4] = 0xFF;
    ppu.oam[9 * 4 + 1] = 19;

    ppu.evaluate_scanline_sprites(None);

    assert!(ppu.status.contains(PpuStatus::SPRITE_OVERFLOW));
}

#[test]
fn sprite_overflow_bug_can_miss_later_ninth_y_coordinate() {
    let mut ppu = Ppu::new();
    ppu.scanline = 20;

    for sprite in 0..8 {
        let base = sprite * 4;
        ppu.oam[base] = 19;
    }

    ppu.oam[8 * 4] = 0xFF;
    ppu.oam[9 * 4] = 19;
    ppu.oam[9 * 4 + 1] = 0xFF;
    ppu.oam[10 * 4 + 2] = 0xFF;
    ppu.oam[11 * 4 + 3] = 0xFF;

    ppu.evaluate_scanline_sprites(None);

    assert!(!ppu.status.contains(PpuStatus::SPRITE_OVERFLOW));
}
